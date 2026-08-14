//! `vole touchid` — PAM Touch ID 引导开关。

use std::io::{self, BufRead, IsTerminal, Write};

use vole_core::ops::{
    disable_touchid, enable_touchid, is_touchid_configured, pam_install_for_runtime,
    pam_paths_injected, plan_touchid, resolve_touchid_paths, touchid_auth_blocked, FakePamInstall,
    PamInstall, TouchIdAction, TouchIdOutcome, TouchIdPaths, TouchIdPlan, PAM_TID_LINE,
};

pub struct TouchidOptions {
    pub action: Option<String>,
    pub plan: bool,
    pub json: bool,
}

pub fn run_touchid(opts: TouchidOptions) -> i32 {
    match run_touchid_inner(opts) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("vole touchid: {e}");
            1
        }
    }
}

fn run_touchid_inner(opts: TouchidOptions) -> io::Result<i32> {
    let paths = resolve_touchid_paths();
    let preview = opts.plan;

    match opts.action.as_deref() {
        None if preview => {
            let plan = plan_touchid(&paths, None);
            emit_plan(&plan, opts.json)?;
            Ok(0)
        }
        None => run_interactive(&paths, opts.json),
        Some("status") => {
            emit_status(&paths, opts.json)?;
            Ok(0)
        }
        Some("enable") => apply_toggle(&paths, TouchIdAction::Enable, preview, opts.json),
        Some("disable") => apply_toggle(&paths, TouchIdAction::Disable, preview, opts.json),
        Some(other) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unknown touchid action: {other} (use status|enable|disable)"),
        )),
    }
}

fn apply_toggle(
    paths: &TouchIdPaths,
    action: TouchIdAction,
    preview: bool,
    json: bool,
) -> io::Result<i32> {
    if preview {
        let plan = plan_touchid(paths, Some(action));
        let outcome = match action {
            TouchIdAction::Enable => enable_touchid(paths, &*installer_for(), true),
            TouchIdAction::Disable => disable_touchid(paths, &*installer_for(), true),
            TouchIdAction::None => unreachable!(),
        }
        .map_err(io::Error::other)?;
        emit_outcome(paths, &outcome, Some(&plan), json)?;
        return Ok(0);
    }
    if touchid_auth_blocked() && !pam_paths_injected() {
        emit_skipped(json)?;
        return Ok(0);
    }
    let outcome = match action {
        TouchIdAction::Enable => enable_touchid(paths, &*installer_for(), false),
        TouchIdAction::Disable => disable_touchid(paths, &*installer_for(), false),
        TouchIdAction::None => unreachable!(),
    }
    .map_err(io::Error::other)?;
    emit_outcome(paths, &outcome, None, json)?;
    Ok(outcome_exit(&outcome))
}

fn installer_for() -> Box<dyn PamInstall> {
    if pam_paths_injected() {
        Box::new(FakePamInstall)
    } else {
        pam_install_for_runtime()
    }
}

fn emit_status(paths: &TouchIdPaths, json: bool) -> io::Result<()> {
    let configured = is_touchid_configured(paths);
    let uses_sudo_local = std::fs::read_to_string(&paths.sudo)
        .map(|s| s.contains("sudo_local"))
        .unwrap_or(false);
    if json || !io::stdout().is_terminal() {
        let body = serde_json::json!({
            "configured": configured,
            "uses_sudo_local": uses_sudo_local,
            "pam_tid_line": PAM_TID_LINE,
        });
        println!("{}", body);
    } else if configured {
        println!("Touch ID is enabled for sudo");
    } else {
        println!("Touch ID is not configured for sudo");
    }
    Ok(())
}

fn emit_plan(plan: &TouchIdPlan, json: bool) -> io::Result<()> {
    if json || !io::stdout().is_terminal() {
        println!("{}", serde_json::to_string(plan).map_err(io::Error::other)?);
    } else {
        println!(
            "touchid plan: configured={} action={:?} targets={}",
            plan.configured,
            plan.action,
            plan.targets.len()
        );
        for t in &plan.targets {
            println!("  - {}", t.display());
        }
    }
    Ok(())
}

fn emit_outcome(
    paths: &TouchIdPaths,
    outcome: &TouchIdOutcome,
    plan: Option<&TouchIdPlan>,
    json: bool,
) -> io::Result<()> {
    let label = outcome_label(outcome);
    if json || !io::stdout().is_terminal() {
        let mut body = serde_json::json!({
            "outcome": label,
            "configured": is_touchid_configured(paths),
        });
        if let Some(p) = plan {
            body["plan"] = serde_json::to_value(p).map_err(io::Error::other)?;
        }
        println!("{}", body);
    } else {
        match outcome {
            TouchIdOutcome::AlreadyEnabled => println!("Touch ID is already enabled"),
            TouchIdOutcome::AlreadyDisabled => println!("Touch ID is not currently enabled"),
            TouchIdOutcome::Enabled => println!("Touch ID enabled, try: sudo ls"),
            TouchIdOutcome::Disabled => println!("Touch ID disabled"),
            TouchIdOutcome::DryRun => println!("[DRY RUN] would change Touch ID configuration"),
            TouchIdOutcome::SkippedNoAuth => {
                println!("skipped: VOLE_TEST_NO_AUTH (no pam writes)")
            }
            TouchIdOutcome::Failed(msg) => eprintln!("touchid failed: {msg}"),
        }
    }
    Ok(())
}

fn emit_skipped(json: bool) -> io::Result<()> {
    if json || !io::stdout().is_terminal() {
        println!(r#"{{"outcome":"skipped_no_auth"}}"#);
    } else {
        println!("skipped: VOLE_TEST_NO_AUTH (no pam writes)");
    }
    Ok(())
}

fn outcome_label(outcome: &TouchIdOutcome) -> &'static str {
    match outcome {
        TouchIdOutcome::AlreadyEnabled => "already_enabled",
        TouchIdOutcome::AlreadyDisabled => "already_disabled",
        TouchIdOutcome::Enabled => "enabled",
        TouchIdOutcome::Disabled => "disabled",
        TouchIdOutcome::DryRun => "dry_run",
        TouchIdOutcome::SkippedNoAuth => "skipped_no_auth",
        TouchIdOutcome::Failed(_) => "failed",
    }
}

fn outcome_exit(outcome: &TouchIdOutcome) -> i32 {
    match outcome {
        TouchIdOutcome::Failed(_) => 1,
        _ => 0,
    }
}

fn run_interactive(paths: &TouchIdPaths, json: bool) -> io::Result<i32> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        eprintln!("vole touchid: provide status|enable|disable, or run in a terminal");
        return Ok(2);
    }
    emit_status(paths, false)?;
    let configured = is_touchid_configured(paths);
    let prompt = if configured {
        "Press Enter to disable, Q to quit: "
    } else {
        "Press Enter to enable, Q to quit: "
    };
    print!("{prompt}");
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    let key = line.chars().next().unwrap_or('\n');
    match key {
        'q' | 'Q' | '\u{1b}' => Ok(0),
        '\n' | '\r' => {
            let action = if configured {
                TouchIdAction::Disable
            } else {
                TouchIdAction::Enable
            };
            apply_toggle(paths, action, false, json)
        }
        _ => {
            eprintln!("Invalid key");
            Ok(1)
        }
    }
}
