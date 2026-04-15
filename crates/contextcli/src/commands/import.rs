use crate::output;
use contextcli_core::AppContext;
use contextcli_core::error::Result;

pub fn run(ctx: &AppContext, app: &str, profile: &str) -> Result<()> {
    let adapter = ctx.registry.get(app)?;
    output::info(&format!(
        "looking for existing {} credentials...",
        adapter.display_name()
    ));

    let router = ctx.router();
    let imported = router.import(app, profile)?;

    if imported {
        let p = ctx.profile_manager.get_profile(app, profile)?;
        let user = p.auth_user.as_deref().unwrap_or("unknown");
        output::success(&format!(
            "imported {} credentials into profile '{}' ({})",
            adapter.display_name(),
            profile,
            user
        ));

        // Auto-set as default if only profile
        let profiles = ctx.profile_manager.list_profiles(app)?;
        if profiles.len() == 1 {
            ctx.profile_manager.set_default(app, profile)?;
            output::info(&format!("set '{}' as default profile", profile));
        }
    } else {
        output::info(&format!(
            "no existing {} credentials found",
            adapter.display_name()
        ));
        output::hint(&format!(
            "use: contextcli login --app {} --profile {} to log in fresh",
            app, profile
        ));
    }

    Ok(())
}
