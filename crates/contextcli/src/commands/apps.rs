use crate::output;
use contextcli_core::AppContext;
use contextcli_core::error::Result;
use contextcli_core::jwt;

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

        // Count expired / expiring-soon tokens
        let expired_count = profiles.iter().filter(|p| {
            p.token_expires_at.is_some_and(|exp| jwt::is_expired(exp))
        }).count();
        let expiring_count = profiles.iter().filter(|p| {
            p.token_expires_at.is_some_and(|exp| !jwt::is_expired(exp) && jwt::expires_within_days(exp, 7))
        }).count();

        let mut expiry_warn = String::new();
        if expired_count > 0 {
            expiry_warn.push_str(&format!(" | 🔴 {} expired", expired_count));
        }
        if expiring_count > 0 {
            expiry_warn.push_str(&format!(" | 🟡 {} expiring soon", expiring_count));
        }

        eprintln!(
            "  {} — {} | {} profile(s), default: {}{}",
            app.display_name, binary_status, profile_count, default_name, expiry_warn
        );

        // Show keychain auth warning per profile that needs it
        for p in profiles.iter().filter(|p| p.needs_keychain_auth) {
            eprintln!(
                "    ⚠  {} — needs one-time keychain auth:",
                p.profile_name
            );
            eprintln!(
                "       contextcli --app {} --profile {} <any command>",
                app.id, p.profile_name
            );
            eprintln!("       then click \"Always Allow\" — never prompted again");
        }

        // Show per-profile expiry warnings
        for p in profiles.iter().filter(|p| p.token_expires_at.is_some()) {
            let exp = p.token_expires_at.unwrap();
            if jwt::is_expired(exp) || jwt::expires_within_days(exp, 7) {
                let icon = if jwt::is_expired(exp) { "🔴" } else { "🟡" };
                eprintln!(
                    "    {}  {} — {}",
                    icon, p.profile_name, jwt::format_expiry(exp)
                );
            }
        }
    }

    eprintln!();
    Ok(())
}
