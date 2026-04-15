use crate::output;
use contextcli_core::AppContext;
use contextcli_core::error::Result;

pub fn run(ctx: &AppContext, app: &str) -> Result<()> {
    let adapter = ctx.registry.get(app)?;
    let profiles = ctx.profile_manager.list_profiles(app)?;

    if profiles.is_empty() {
        output::info(&format!("no profiles for {}", adapter.display_name()));
        output::hint(&format!("create one: contextcli login --app {} --profile <name>", app));
        return Ok(());
    }

    output::header(&format!("{} profiles:", adapter.display_name()));
    eprintln!();

    for p in &profiles {
        let default_marker = if p.is_default { " (default)" } else { "" };
        let status = output::status_badge(p.auth_state.as_str());
        let user = p
            .auth_user
            .as_deref()
            .map(|u| format!(" [{u}]"))
            .unwrap_or_default();

        eprintln!(
            "  {}{} — {}{}",
            p.profile_name, default_marker, status, user
        );
    }

    eprintln!();
    Ok(())
}
