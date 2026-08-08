//! `vole update` — 自更新通道。

use std::io::{self, BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};

use vole_core::ops::{
    default_config_dir, run_update, CurlUpdateTransport, ExecVersionProbe, FakeUpdateTransport,
    InstallOrigin, UpdateOptions, UpdateOutcome, UpdateTransport,
};

pub struct UpdateCliOptions {
    pub force: bool,
    pub nightly: bool,
    pub check: bool,
    pub yes: bool,
    pub json: bool,
}

pub fn run_update_cli(opts: UpdateCliOptions) -> i32 {
    match run_update_inner(opts) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("vole update: {e}");
            1
        }
    }
}

fn run_update_inner(opts: UpdateCliOptions) -> io::Result<i32> {
    let binary_path = resolve_binary_path()?;
    let config_dir = std::env::var_os("VOLE_CONFIG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(default_config_dir);
    let repo = std::env::var("VOLE_UPDATE_REPO").unwrap_or_else(|_| "wukongnotnull/vole".into());

    let mut update_opts = UpdateOptions {
        force: opts.force,
        nightly: opts.nightly,
        check_only: opts.check,
        yes: opts.yes,
        current_version: env!("CARGO_PKG_VERSION").to_string(),
        binary_path,
        config_dir,
        confirm_brew_self_update: false,
        repo: repo.clone(),
        arch_triple: std::env::var("VOLE_UPDATE_ARCH").ok(),
    };

    // Homebrew 默认提示；TTY 下可确认自更新（--force/--yes 已在 core 放行）。
    if !opts.check && !opts.force && !opts.yes && !opts.nightly {
        if let Ok(layout_origin) = peek_origin(&update_opts.binary_path, &update_opts.config_dir) {
            if layout_origin == InstallOrigin::Homebrew
                && io::stdin().is_terminal()
                && io::stdout().is_terminal()
            {
                update_opts.confirm_brew_self_update = prompt_brew_self_update()?;
            }
        }
    }

    let transport = transport_for(&repo);
    let outcome = run_update(&update_opts, transport.as_ref(), &ExecVersionProbe)
        .map_err(io::Error::other)?;
    emit_outcome(&outcome, opts.json)?;
    Ok(outcome_exit(&outcome))
}

fn peek_origin(binary: &Path, config: &Path) -> io::Result<InstallOrigin> {
    Ok(vole_core::ops::detect_install_layout(binary, Some(config)).origin)
}

fn resolve_binary_path() -> io::Result<PathBuf> {
    if let Some(p) = std::env::var_os("VOLE_UPDATE_EXE") {
        return Ok(PathBuf::from(p));
    }
    std::env::current_exe()
}

fn transport_for(repo: &str) -> Box<dyn UpdateTransport> {
    if let Ok(tag) = std::env::var("VOLE_UPDATE_FAKE") {
        let mut fake = FakeUpdateTransport::new(if tag.is_empty() { "0.0.0".into() } else { tag });
        if let Ok(commit) = std::env::var("VOLE_UPDATE_FAKE_COMMIT") {
            fake.latest_commit = commit;
        }
        // Optional in-memory files via VOLE_UPDATE_FAKE_DIR: <url-basename> files not needed for --check
        Box::new(fake)
    } else {
        Box::new(CurlUpdateTransport::new(repo.to_string()))
    }
}

fn prompt_brew_self_update() -> io::Result<bool> {
    let mut stdout = io::stdout();
    writeln!(
        stdout,
        "This vole appears to be Homebrew-managed. Prefer: brew upgrade vole"
    )?;
    write!(stdout, "Continue with self-update anyway? [y/N] ")?;
    stdout.flush()?;
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    let t = line.trim();
    Ok(t.eq_ignore_ascii_case("y") || t.eq_ignore_ascii_case("yes"))
}

fn emit_outcome(outcome: &UpdateOutcome, json: bool) -> io::Result<()> {
    if json || !io::stdout().is_terminal() {
        let body = match outcome {
            UpdateOutcome::AlreadyLatest { version } => serde_json::json!({
                "outcome": "already_latest",
                "version": version,
            }),
            UpdateOutcome::Check {
                current,
                latest,
                origin,
                channel,
            } => serde_json::json!({
                "outcome": "check",
                "current": current,
                "latest": latest,
                "origin": origin_str(*origin),
                "channel": channel,
            }),
            UpdateOutcome::Updated { version } => serde_json::json!({
                "outcome": "updated",
                "version": version,
            }),
            UpdateOutcome::BrewPreferred => serde_json::json!({
                "outcome": "brew_preferred",
                "hint": "brew upgrade vole",
            }),
            UpdateOutcome::NightlyBrewRejected => serde_json::json!({
                "outcome": "nightly_brew_rejected",
            }),
            UpdateOutcome::Failed(msg) => serde_json::json!({
                "outcome": "failed",
                "error": msg,
            }),
        };
        println!("{}", body);
        return Ok(());
    }

    match outcome {
        UpdateOutcome::AlreadyLatest { version } => {
            println!("Already on latest version, {version}");
        }
        UpdateOutcome::Check {
            current,
            latest,
            origin,
            channel,
        } => {
            println!("Current: {current}");
            println!("Latest:  {}", latest.as_deref().unwrap_or("unknown"));
            println!("Origin:  {}", origin_str(*origin));
            println!("Channel: {channel}");
        }
        UpdateOutcome::Updated { version } => {
            println!("Updated to {version}");
        }
        UpdateOutcome::BrewPreferred => {
            println!("Homebrew install detected. Prefer:");
            println!("  brew upgrade vole");
            println!("Or re-run with --force / --yes to self-update.");
        }
        UpdateOutcome::NightlyBrewRejected => {
            eprintln!(
                "Nightly update is only available for script installations. Homebrew installs follow stable releases."
            );
        }
        UpdateOutcome::Failed(msg) => eprintln!("update failed: {msg}"),
    }
    Ok(())
}

fn origin_str(o: InstallOrigin) -> &'static str {
    match o {
        InstallOrigin::Homebrew => "homebrew",
        InstallOrigin::Manual => "manual",
        InstallOrigin::Unknown => "unknown",
    }
}

fn outcome_exit(outcome: &UpdateOutcome) -> i32 {
    match outcome {
        UpdateOutcome::Failed(_) | UpdateOutcome::NightlyBrewRejected => 1,
        UpdateOutcome::BrewPreferred => 2,
        _ => 0,
    }
}
