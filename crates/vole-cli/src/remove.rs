//! `vole remove` — 自卸载。

use std::io::{self, BufRead, IsTerminal, Write};
use std::path::PathBuf;

use serde_json::json;
use vole_core::ops::{
    default_config_dir, plan_remove, run_remove, BrewUninstaller, FakeBrewUninstaller,
    LiveBrewUninstaller, RemoveItemKind, RemoveOptions, RemoveOutcome, RemovePlan,
};

pub struct RemoveCliOptions {
    pub dry_run: bool,
    pub yes: bool,
    pub json: bool,
    pub purge_oplog: bool,
}

pub fn run_remove_cli(opts: RemoveCliOptions) -> i32 {
    match run_remove_inner(opts) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("vole remove: {e}");
            1
        }
    }
}

fn run_remove_inner(opts: RemoveCliOptions) -> io::Result<i32> {
    let binary_path = resolve_binary_path()?;
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"));
    let config_dir = std::env::var_os("VOLE_CONFIG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(default_config_dir);

    let mut remove_opts = RemoveOptions {
        dry_run: opts.dry_run,
        yes: opts.yes,
        purge_oplog: opts.purge_oplog,
        binary_path,
        home,
        config_dir,
    };

    if !remove_opts.dry_run && !remove_opts.yes {
        let plan = plan_remove(&remove_opts);
        if plan.items.is_empty() {
            emit_nothing(opts.json)?;
            return Ok(0);
        }
        emit_plan_preview(&plan, opts.json)?;
        if !opts.json {
            if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
                eprintln!("vole remove: non-interactive terminal requires --yes");
                return Ok(2);
            }
            if !prompt_confirm()? {
                return Ok(0);
            }
        } else if !io::stdin().is_terminal() {
            eprintln!("vole remove: non-interactive terminal requires --yes");
            return Ok(2);
        } else if !prompt_confirm()? {
            return Ok(0);
        }
        remove_opts.yes = true;
    }

    let brew = brew_uninstaller();
    let outcome = run_remove(&remove_opts, brew.as_ref()).map_err(io::Error::other)?;
    emit_outcome(&outcome, opts.json)?;
    Ok(outcome_exit(&outcome))
}

fn brew_uninstaller() -> Box<dyn BrewUninstaller> {
    if std::env::var_os("VOLE_REMOVE_FAKE_BREW").is_some_and(|v| v == "1") {
        Box::new(FakeBrewUninstaller::default())
    } else {
        Box::new(LiveBrewUninstaller)
    }
}

fn resolve_binary_path() -> io::Result<PathBuf> {
    if let Some(p) = std::env::var_os("VOLE_UPDATE_EXE") {
        return Ok(PathBuf::from(p));
    }
    std::env::current_exe()
}

fn prompt_confirm() -> io::Result<bool> {
    let mut stdout = io::stdout();
    write!(
        stdout,
        "Press Enter to confirm removal, or Ctrl-C / type n to cancel: "
    )?;
    stdout.flush()?;
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    let t = line.trim();
    Ok(t.is_empty() || t.eq_ignore_ascii_case("y") || t.eq_ignore_ascii_case("yes"))
}

fn emit_nothing(json: bool) -> io::Result<()> {
    if json {
        println!(
            "{}",
            json!({
                "status": "nothing_found",
                "homebrew": false,
                "items": []
            })
        );
    } else {
        println!("No Vole installation detected");
    }
    Ok(())
}

fn emit_plan_preview(plan: &RemovePlan, json: bool) -> io::Result<()> {
    if json {
        // preview before confirm; full emit after apply
        let _ = plan;
        return Ok(());
    }
    println!("Remove Vole, will delete / act on the following:");
    print_plan_human(plan);
    Ok(())
}

fn emit_outcome(outcome: &RemoveOutcome, json: bool) -> io::Result<()> {
    match outcome {
        RemoveOutcome::DryRun(plan) => {
            if json {
                println!("{}", plan_to_json(plan, "dry_run"));
            } else {
                println!("DRY RUN MODE, no files will be removed");
                println!();
                println!("Remove Vole, would delete / act on the following:");
                print_plan_human(plan);
                println!();
                println!("Dry run complete, no changes made");
            }
        }
        RemoveOutcome::NothingFound => emit_nothing(json)?,
        RemoveOutcome::NeedsConfirmation => {
            // CLI should have confirmed already
            if json {
                println!("{}", json!({ "status": "needs_confirmation" }));
            }
        }
        RemoveOutcome::Removed { plan, errors } => {
            if json {
                let mut value = plan_to_json(plan, "removed");
                if let Some(obj) = value.as_object_mut() {
                    obj.insert("errors".into(), json!(errors));
                }
                println!("{value}");
            } else if errors.is_empty() {
                println!("Vole uninstalled successfully, thank you for using Vole!");
            } else {
                for e in errors {
                    eprintln!("vole remove: {e}");
                }
                println!("Vole uninstalled with some errors, thank you for using Vole!");
            }
        }
    }
    Ok(())
}

fn outcome_exit(outcome: &RemoveOutcome) -> i32 {
    match outcome {
        RemoveOutcome::DryRun(_) | RemoveOutcome::NothingFound => 0,
        RemoveOutcome::NeedsConfirmation => 2,
        RemoveOutcome::Removed { errors, .. } => {
            if errors.is_empty() {
                0
            } else {
                1
            }
        }
    }
}

fn print_plan_human(plan: &RemovePlan) {
    for item in &plan.items {
        match item.kind {
            RemoveItemKind::BrewUninstall => {
                let note = item.note.as_deref().unwrap_or("brew uninstall vole");
                println!("  - Would run: {note}");
            }
            _ => {
                if let Some(path) = &item.path {
                    let label = kind_label(item.kind);
                    println!("  - [{label}] {}", path.display());
                }
            }
        }
    }
}

fn plan_to_json(plan: &RemovePlan, status: &str) -> serde_json::Value {
    let items: Vec<_> = plan
        .items
        .iter()
        .map(|i| {
            json!({
                "kind": kind_label(i.kind),
                "path": i.path.as_ref().map(|p| p.display().to_string()),
                "note": i.note,
            })
        })
        .collect();
    json!({
        "status": status,
        "homebrew": plan.homebrew,
        "items": items,
    })
}

fn kind_label(kind: RemoveItemKind) -> &'static str {
    match kind {
        RemoveItemKind::BrewUninstall => "brew",
        RemoveItemKind::ManualBinary => "manual_binary",
        RemoveItemKind::ShareTree => "share",
        RemoveItemKind::AliasOrCompletion => "completion",
        RemoveItemKind::Config => "config",
        RemoveItemKind::Cache => "cache",
        RemoveItemKind::ToolLogs => "tool_logs",
        RemoveItemKind::Oplog => "oplog",
    }
}
