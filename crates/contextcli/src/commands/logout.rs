use crate::output;
use contextcli_core::AppContext;
use contextcli_core::error::Result;

pub fn run(ctx: &AppContext, app: &str, profile: Option<&str>) -> Result<()> {
    let adapter = ctx.registry.get(app)?;
    let router = ctx.router();
    router.logout(app, profile)?;

    let display = profile.unwrap_or("default");
    output::success(&format!(
        "logged out of {} profile '{}'",
        adapter.display_name(),
        display
    ));
    Ok(())
}
