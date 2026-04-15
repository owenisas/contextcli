use contextcli_core::adapter::types::ValidationResult;
use contextcli_core::profile::types::{App, Profile, ProjectLink};
use contextcli_core::AppContext;
use serde::Serialize;
use std::sync::Mutex;
use tauri::{Manager, State};

type CmdResult<T> = Result<T, String>;

#[derive(Serialize)]
pub struct AdapterInfo {
    pub id: String,
    pub display_name: String,
    pub binary_names: Vec<String>,
    pub support_level: String,
}

// ── App queries ──────────────────────────────────────────

#[tauri::command]
fn list_apps(ctx: State<'_, Mutex<AppContext>>) -> CmdResult<Vec<App>> {
    let ctx = ctx.lock().map_err(|e| e.to_string())?;
    ctx.profile_manager.list_apps().map_err(|e| e.to_string())
}

#[tauri::command]
fn get_adapter_info(ctx: State<'_, Mutex<AppContext>>, app_id: String) -> CmdResult<AdapterInfo> {
    let ctx = ctx.lock().map_err(|e| e.to_string())?;
    let adapter = ctx.registry.get(&app_id).map_err(|e| e.to_string())?;
    Ok(AdapterInfo {
        id: adapter.id().to_string(),
        display_name: adapter.display_name().to_string(),
        binary_names: vec![adapter.binary_name().to_string()],
        support_level: adapter.support_level().to_string(),
    })
}

// ── Profile CRUD ─────────────────────────────────────────

#[tauri::command]
fn list_profiles(ctx: State<'_, Mutex<AppContext>>, app_id: String) -> CmdResult<Vec<Profile>> {
    let ctx = ctx.lock().map_err(|e| e.to_string())?;
    ctx.profile_manager
        .list_profiles(&app_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn create_profile(
    ctx: State<'_, Mutex<AppContext>>,
    app_id: String,
    profile_name: String,
    label: Option<String>,
) -> CmdResult<Profile> {
    let ctx = ctx.lock().map_err(|e| e.to_string())?;
    ctx.profile_manager
        .create_profile(&app_id, &profile_name, label.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn set_default(
    ctx: State<'_, Mutex<AppContext>>,
    app_id: String,
    profile_name: String,
) -> CmdResult<()> {
    let ctx = ctx.lock().map_err(|e| e.to_string())?;
    ctx.profile_manager
        .set_default(&app_id, &profile_name)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_profile(
    ctx: State<'_, Mutex<AppContext>>,
    app_id: String,
    profile_name: String,
) -> CmdResult<()> {
    let ctx = ctx.lock().map_err(|e| e.to_string())?;
    ctx.profile_manager
        .delete_profile(&app_id, &profile_name)
        .map_err(|e| e.to_string())
}

// ── Operations ───────────────────────────────────────────

#[tauri::command]
fn validate_profile(
    ctx: State<'_, Mutex<AppContext>>,
    app_id: String,
    profile_name: String,
) -> CmdResult<ValidationResult> {
    let ctx = ctx.lock().map_err(|e| e.to_string())?;
    let router = ctx.router();
    router
        .validate(&app_id, &profile_name)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn trigger_logout(
    ctx: State<'_, Mutex<AppContext>>,
    app_id: String,
    profile_name: String,
) -> CmdResult<()> {
    let ctx = ctx.lock().map_err(|e| e.to_string())?;
    let router = ctx.router();
    router
        .logout(&app_id, &profile_name)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn list_project_links(
    ctx: State<'_, Mutex<AppContext>>,
    app_id: String,
) -> CmdResult<Vec<ProjectLink>> {
    let ctx = ctx.lock().map_err(|e| e.to_string())?;
    ctx.profile_manager
        .list_project_links(&app_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn import_profile(
    ctx: State<'_, Mutex<AppContext>>,
    app_id: String,
    profile_name: String,
) -> CmdResult<bool> {
    let ctx = ctx.lock().map_err(|e| e.to_string())?;
    let router = ctx.router();
    router
        .import(&app_id, &profile_name)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn rename_profile(
    ctx: State<'_, Mutex<AppContext>>,
    app_id: String,
    old_name: String,
    new_name: String,
) -> CmdResult<Profile> {
    let ctx = ctx.lock().map_err(|e| e.to_string())?;
    ctx.profile_manager
        .rename_profile(&app_id, &old_name, &new_name)
        .map_err(|e| e.to_string())
}

/// Validate that a path is safe to open: absolute, exists, within home directory.
fn validate_open_path(path: &str) -> CmdResult<()> {
    let p = std::path::Path::new(path);
    if !p.is_absolute() {
        return Err("path must be absolute".to_string());
    }
    if !p.exists() {
        return Err(format!("path does not exist: {}", path));
    }
    let canonical = std::fs::canonicalize(p).map_err(|e| e.to_string())?;
    let home = std::env::var("HOME")
        .map(std::path::PathBuf::from)
        .map_err(|_| "cannot determine home directory".to_string())?;
    if !canonical.starts_with(&home) {
        return Err("path is outside home directory".to_string());
    }
    Ok(())
}

#[tauri::command]
fn open_directory(path: String) -> CmdResult<()> {
    validate_open_path(&path)?;
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn open_terminal_at(path: String) -> CmdResult<()> {
    validate_open_path(&path)?;
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .args(["-a", "Terminal", &path])
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ── CLI Installation ─────────────────────────────────────

const CLI_INSTALL_PATH: &str = "/usr/local/bin/contextcli";

#[tauri::command]
fn check_cli_installed() -> CmdResult<bool> {
    // Check if contextcli is anywhere in PATH
    Ok(which::which("contextcli").is_ok())
}

#[tauri::command]
fn install_cli(app_handle: tauri::AppHandle) -> CmdResult<String> {
    // Find the embedded CLI binary inside the .app bundle
    let resource_dir = app_handle
        .path()
        .resource_dir()
        .map_err(|e| format!("cannot find resource dir: {e}"))?;

    let embedded_cli = resource_dir.join("contextcli");

    // Also check Contents/Resources/ directly (for manual .app builds)
    let embedded_cli = if embedded_cli.exists() {
        embedded_cli
    } else {
        // Resolve from the executable path
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let app_contents = exe
            .parent() // MacOS/
            .and_then(|p| p.parent()) // Contents/
            .ok_or("cannot resolve .app bundle path")?;
        let alt = app_contents.join("Resources/contextcli");
        if alt.exists() {
            alt
        } else {
            return Err(format!(
                "CLI binary not found in app bundle. Looked at:\n  {}\n  {}",
                embedded_cli.display(),
                alt.display()
            ));
        }
    };

    // Ensure /usr/local/bin exists
    let install_dir = std::path::Path::new("/usr/local/bin");
    if !install_dir.exists() {
        std::fs::create_dir_all(install_dir).map_err(|e| {
            format!("cannot create /usr/local/bin (may need sudo): {e}")
        })?;
    }

    // Copy the binary
    std::fs::copy(&embedded_cli, CLI_INSTALL_PATH)
        .map_err(|e| format!("failed to install CLI to {CLI_INSTALL_PATH}: {e}"))?;

    // Make executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(CLI_INSTALL_PATH, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("failed to set permissions: {e}"))?;
    }

    // Codesign
    let _ = std::process::Command::new("codesign")
        .args(["--force", "--sign", "-", "--identifier", "com.contextcli.cli", CLI_INSTALL_PATH])
        .output();

    Ok(CLI_INSTALL_PATH.to_string())
}

// ── Tauri entry point ────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("contextcli=info".parse().unwrap()),
        )
        .with_target(false)
        .init();

    let ctx = AppContext::init().expect("failed to initialize AppContext");

    tauri::Builder::default()
        .manage(Mutex::new(ctx))
        .invoke_handler(tauri::generate_handler![
            list_apps,
            get_adapter_info,
            list_profiles,
            create_profile,
            set_default,
            delete_profile,
            validate_profile,
            trigger_logout,
            import_profile,
            list_project_links,
            rename_profile,
            open_directory,
            open_terminal_at,
            check_cli_installed,
            install_cli,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
