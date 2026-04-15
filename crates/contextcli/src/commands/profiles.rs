use crate::output;
use contextcli_core::AppContext;
use contextcli_core::error::Result;
use contextcli_core::jwt;

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

        let auth_warn = if p.needs_keychain_auth { " ⚠ needs keychain auth" } else { "" };

        // Token expiry info
        let expiry_info = match p.token_expires_at {
            Some(exp) if jwt::is_expired(exp) => format!(" 🔴 {}", jwt::format_expiry(exp)),
            Some(exp) if jwt::expires_within_days(exp, 7) => format!(" 🟡 {}", jwt::format_expiry(exp)),
            Some(exp) => format!(" {}", jwt::format_expiry(exp)),
            None => String::new(),
        };

        eprintln!(
            "  {}{} — {}{}{}{}",
            p.profile_name, default_marker, status, user, auth_warn, expiry_info
        );

        if p.needs_keychain_auth {
            eprintln!(
                "    → run: contextcli --app {} --profile {} <any command>",
                app, p.profile_name
            );
            eprintln!("      then click \"Always Allow\" — never prompted again");
        }

        if let Some(ref validated) = p.last_validated_at {
            eprintln!("    last validated: {validated}");
        }
    }

    eprintln!();
    Ok(())
}
