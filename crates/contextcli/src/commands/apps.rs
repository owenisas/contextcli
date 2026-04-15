use crate::output;
use contextcli_core::AppContext;
use contextcli_core::error::Result;

pub fn run(ctx: &AppContext) -> Result<()> {
    let apps = ctx.profile_manager.list_apps()?;

    if apps.is_empty() {
        output::info("no apps registered");
        return Ok(());
    }

    output::header("registered apps:");
    eprintln!();

    for app in &apps {
        let binary_status = match &app.binary_path {
            Some(path) => format!("✓ {}", path),
            None => "✗ not found".to_string(),
        };

        let profiles = ctx.profile_manager.list_profiles(&app.id)?;
        let profile_count = profiles.len();
        let default_name = profiles
            .iter()
            .find(|p| p.is_default)
            .map(|p| p.profile_name.as_str())
            .unwrap_or("none");

        eprintln!(
            "  {} — {} | {} profile(s), default: {}",
            app.display_name, binary_status, profile_count, default_name
        );
    }

    eprintln!();
    Ok(())
}
