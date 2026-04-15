use crate::output;
use contextcli_core::AppContext;
use contextcli_core::error::Result;

pub fn run(ctx: &AppContext, app: &str) -> Result<()> {
    let adapter = ctx.registry.get(app)?;
    output::header(&format!("{} health check:", adapter.display_name()));
    eprintln!();

    // Check binary
    let binary_name = adapter.binary_name();
    match which::which(binary_name) {
        Ok(path) => {
            output::success(&format!("binary found: {}", path.display()));
        }
        Err(_) => {
            output::error(&format!("binary '{}' not found in PATH", binary_name));
            return Ok(());
        }
    }

    // Check profiles
    let profiles = ctx.profile_manager.list_profiles(app)?;
    if profiles.is_empty() {
        output::info("no profiles configured");
        output::hint(&format!(
            "create one: contextcli login --app {} --profile <name>",
            app
        ));
        return Ok(());
    }

    let router = ctx.router();
    for p in &profiles {
        let default_marker = if p.is_default { " (default)" } else { "" };
        output::info(&format!("validating profile '{}'{}...", p.profile_name, default_marker));

        match router.validate(app, &p.profile_name) {
            Ok(result) => {
                if result.valid {
                    let user = result.identity.as_deref().unwrap_or("unknown");
                    output::success(&format!("  {} — authenticated as {}", p.profile_name, user));
                } else {
                    let msg = result.message.as_deref().unwrap_or("unknown error");
                    output::error(&format!("  {} — {}", p.profile_name, msg));
                }
            }
            Err(e) => {
                output::error(&format!("  {} — {}", p.profile_name, e));
            }
        }
    }

    eprintln!();
    Ok(())
}
