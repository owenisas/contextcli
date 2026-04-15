use crate::output;
use contextcli_core::AppContext;
use contextcli_core::error::Result;

pub fn run(ctx: &AppContext, app: &str, old_name: &str, new_name: &str) -> Result<()> {
    let adapter = ctx.registry.get(app)?;
    ctx.profile_manager.rename_profile(app, old_name, new_name)?;
    output::success(&format!(
        "renamed {} profile '{}' → '{}'",
        adapter.display_name(),
        old_name,
        new_name
    ));
    Ok(())
}
