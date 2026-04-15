use crate::output;
use contextcli_core::AppContext;
use contextcli_core::error::Result;
use contextcli_core::project::{self, Policies, ProjectConfig};
use std::collections::HashMap;

/// Link an app to a profile in the current directory's .contextcli.toml
pub fn run(ctx: &AppContext, app: &str, profile: &str) -> Result<()> {
    let cwd = std::env::current_dir()?;

    // Load existing config or create new
    let mut config = match project::find_project_config(&cwd) {
        Some((existing, _)) => existing,
        None => ProjectConfig {
            profiles: HashMap::new(),
            policies: Policies::default(),
        },
    };

    config.profiles.insert(app.to_string(), profile.to_string());
    project::write_project_config(&cwd, &config)?;

    // Register in DB for GUI visibility
    let _ = ctx.profile_manager.register_project_link(
        &cwd.to_string_lossy(),
        app,
        profile,
    );

    output::success(&format!(
        "linked {} → profile '{}' in {}",
        app,
        profile,
        cwd.join(".contextcli.toml").display()
    ));
    output::hint("commands in this directory will auto-use this profile");

    Ok(())
}

/// Unlink an app from the current directory's .contextcli.toml
pub fn unlink(ctx: &AppContext, app: &str) -> Result<()> {
    let cwd = std::env::current_dir()?;

    let mut config = match project::find_project_config(&cwd) {
        Some((existing, _)) => existing,
        None => {
            output::info("no .contextcli.toml found in this directory");
            return Ok(());
        }
    };

    if config.profiles.remove(app).is_some() {
        project::write_project_config(&cwd, &config)?;
        let _ = ctx.profile_manager.remove_project_link(&cwd.to_string_lossy(), app);
        output::success(&format!("unlinked {} from this project", app));
    } else {
        output::info(&format!("{} was not linked in this project", app));
    }

    Ok(())
}

/// Show current project config
pub fn show() -> Result<()> {
    let cwd = std::env::current_dir()?;

    match project::find_project_config(&cwd) {
        Some((config, path)) => {
            output::header(&format!("project config: {}", path.display()));
            eprintln!();

            if config.profiles.is_empty() {
                output::info("no profile mappings");
            } else {
                output::header("profile mappings:");
                for (app, profile) in &config.profiles {
                    eprintln!("  {} → {}", app, profile);
                }
            }

            if !config.policies.deny.is_empty() {
                eprintln!();
                output::header("policies:");
                for rule in &config.policies.deny {
                    let profile = rule.profile.as_deref().unwrap_or("*");
                    let reason = rule.reason.as_deref().unwrap_or("(no reason)");
                    eprintln!(
                        "  deny {} [{}] when args contain [{}] — {}",
                        rule.app,
                        profile,
                        rule.args_contain.join(", "),
                        reason
                    );
                }
            }

            eprintln!();
        }
        None => {
            output::info("no .contextcli.toml found (searched up from current directory)");
            output::hint("create one: contextcli link --app vercel --profile work");
        }
    }

    Ok(())
}
