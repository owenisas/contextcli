use crate::output;
use contextcli_core::AppContext;
use contextcli_core::error::Result;

pub fn run(ctx: &AppContext, app: &str, profile: &str) -> Result<()> {
    let adapter = ctx.registry.get(app)?;
    ctx.profile_manager.set_default(app, profile)?;
    output::success(&format!(
        "default profile for {} set to '{}'",
        adapter.display_name(),
        profile
    ));
    Ok(())
}
