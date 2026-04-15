use crate::output;
use contextcli_core::AppContext;
use contextcli_core::error::Result;

pub fn run(ctx: &AppContext, app: &str, profile: Option<&str>) -> Result<()> {
    let adapter = ctx.registry.get(app)?;
    let profile_display = profile.unwrap_or("default");

    output::info(&format!(
        "opening shell with {} profile '{}'...",
        adapter.display_name(),
        profile_display
    ));
    output::hint("type 'exit' to return to your normal shell");

    let router = ctx.router();
    let status = router.shell(app, profile)?;

    if status.success() {
        output::success("shell session ended");
    }

    Ok(())
}
