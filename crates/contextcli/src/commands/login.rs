use crate::output;
use contextcli_core::AppContext;
use contextcli_core::error::Result;

pub fn run(ctx: &AppContext, app: &str, profile: &str) -> Result<()> {
    // Verify adapter exists
    let adapter = ctx.registry.get(app)?;
    output::info(&format!(
        "logging in to {} as profile '{}'...",
        adapter.display_name(),
        profile
    ));

    let router = ctx.router();
    router.login(app, profile)?;

    output::success(&format!(
        "logged in to {} profile '{}'",
        adapter.display_name(),
        profile
    ));

    // Set as default if it's the only profile
    let profiles = ctx.profile_manager.list_profiles(app)?;
    if profiles.len() == 1 {
        ctx.profile_manager.set_default(app, profile)?;
        output::info(&format!("set '{}' as default profile", profile));
    }

    Ok(())
}
