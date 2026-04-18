use contextcli_core::adapter::types::{AuthCapabilities, ValidationResult};
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
    pub auth: AuthCapabilities,
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
        auth: adapter.auth_capabilities(),
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
        .logout(&app_id, Some(&profile_name))
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
//
// Target: $HOME/.local/bin/contextcli (user-writable, no sudo, no password).
// PATH is configured via idempotent marker blocks in ~/.zshenv and ~/.bash_profile.

const LEGACY_INSTALL_PATH: &str = "/usr/local/bin/contextcli";
const PATH_MARKER_START: &str = "# >>> contextcli PATH >>>";
const PATH_MARKER_END: &str = "# <<< contextcli PATH <<<";

#[derive(Serialize)]
pub struct InstallResult {
    pub path: String,
    pub path_shells_updated: Vec<String>,
    pub needs_shell_restart: bool,
    pub legacy_install_at: Option<String>,
}

fn home_dir() -> CmdResult<std::path::PathBuf> {
    std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .ok_or_else(|| "cannot determine home directory".to_string())
}

fn user_bin_dir() -> CmdResult<std::path::PathBuf> {
    Ok(home_dir()?.join(".local").join("bin"))
}

fn user_bin_target() -> CmdResult<std::path::PathBuf> {
    Ok(user_bin_dir()?.join("contextcli"))
}

#[tauri::command]
fn check_cli_installed() -> CmdResult<bool> {
    // Trust the user-local target first; fall back to `which` (for legacy or custom installs).
    if let Ok(target) = user_bin_target()
        && target.exists()
    {
        return Ok(true);
    }
    Ok(which::which("contextcli").is_ok())
}

/// Report a legacy `/usr/local/bin/contextcli` install so the UI can prompt the
/// user to remove it manually (avoids requesting sudo from the app).
#[tauri::command]
fn detect_legacy_install() -> CmdResult<Option<String>> {
    let p = std::path::Path::new(LEGACY_INSTALL_PATH);
    Ok(if p.exists() {
        Some(LEGACY_INSTALL_PATH.to_string())
    } else {
        None
    })
}

/// Locate the embedded CLI binary inside the .app bundle.
///
/// Tauri v2's `externalBin` places sidecars at `Contents/MacOS/<name>-<triple>`
/// alongside the main executable. We also fall back to the historic
/// `Contents/Resources/contextcli` layout for local dev builds.
fn find_embedded_cli(app_handle: &tauri::AppHandle) -> CmdResult<std::path::PathBuf> {
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();

    // Preferred: sidecar next to the main executable.
    if let Ok(exe) = std::env::current_exe()
        && let Some(exe_dir) = exe.parent()
    {
        for triple in sidecar_triples() {
            candidates.push(exe_dir.join(format!("contextcli-{triple}")));
        }
        candidates.push(exe_dir.join("contextcli"));
        // Contents/Resources/contextcli (legacy layout).
        if let Some(contents) = exe_dir.parent() {
            candidates.push(contents.join("Resources").join("contextcli"));
        }
    }

    // Last resort: Tauri's resource_dir.
    if let Ok(resource_dir) = app_handle.path().resource_dir() {
        candidates.push(resource_dir.join("contextcli"));
    }

    for path in &candidates {
        if path.exists() {
            return Ok(path.clone());
        }
    }

    Err(format!(
        "CLI binary not found in app bundle. Looked at:\n  {}",
        candidates
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join("\n  ")
    ))
}

fn sidecar_triples() -> &'static [&'static str] {
    #[cfg(target_os = "macos")]
    {
        if cfg!(target_arch = "aarch64") {
            &["aarch64-apple-darwin", "universal-apple-darwin"]
        } else {
            &["x86_64-apple-darwin", "universal-apple-darwin"]
        }
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        &["x86_64-unknown-linux-gnu"]
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    {
        &["aarch64-unknown-linux-gnu"]
    }
    #[cfg(target_os = "windows")]
    {
        &["x86_64-pc-windows-msvc"]
    }
}

/// Append a marker-wrapped `export PATH=...` block to a shell rc file if missing.
/// Returns true if the file was modified, false if the block was already present.
fn ensure_path_in_rc(rc_path: &std::path::Path, export_line: &str) -> CmdResult<bool> {
    let existing = match std::fs::read_to_string(rc_path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("failed to read {}: {e}", rc_path.display())),
    };

    if existing.contains(PATH_MARKER_START) {
        return Ok(false);
    }

    // Ensure the existing content ends with a newline before we append.
    let needs_leading_newline = !existing.is_empty() && !existing.ends_with('\n');
    let mut new_content = existing;
    if needs_leading_newline {
        new_content.push('\n');
    }
    new_content.push_str(&format!(
        "\n{PATH_MARKER_START}\n{export_line}\n{PATH_MARKER_END}\n"
    ));

    std::fs::write(rc_path, new_content)
        .map_err(|e| format!("failed to write {}: {e}", rc_path.display()))?;
    Ok(true)
}

/// Configure `$HOME/.local/bin` on PATH via the user's shell rc files.
/// Returns the list of rc files that were modified (empty if all already had it).
fn ensure_path_configured() -> CmdResult<Vec<String>> {
    let home = home_dir()?;
    let export_line = r#"export PATH="$HOME/.local/bin:$PATH""#;

    let targets = [
        ("~/.zshenv", home.join(".zshenv")),
        ("~/.bash_profile", home.join(".bash_profile")),
    ];

    let mut updated = Vec::new();
    for (label, path) in targets {
        if ensure_path_in_rc(&path, export_line)? {
            updated.push(label.to_string());
        }
    }
    Ok(updated)
}

#[tauri::command]
fn install_cli(app_handle: tauri::AppHandle) -> CmdResult<InstallResult> {
    let embedded_cli = find_embedded_cli(&app_handle)?;
    let bin_dir = user_bin_dir()?;
    let target = user_bin_target()?;

    std::fs::create_dir_all(&bin_dir)
        .map_err(|e| format!("failed to create {}: {e}", bin_dir.display()))?;

    std::fs::copy(&embedded_cli, &target)
        .map_err(|e| format!("failed to install CLI to {}: {e}", target.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("failed to set permissions: {e}"))?;
    }

    // Ad-hoc re-sign so macOS accepts the relocated binary.
    // If the sidecar was already signed with a Developer ID, this replaces that
    // signature with an ad-hoc one — acceptable for user-local copy. The app
    // bundle itself retains its Developer ID signature.
    #[cfg(target_os = "macos")]
    {
        let out = std::process::Command::new("codesign")
            .args([
                "--force",
                "--sign",
                "-",
                "--identifier",
                "com.contextcli.cli",
            ])
            .arg(&target)
            .output()
            .map_err(|e| format!("codesign failed to spawn: {e}"))?;
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            tracing::warn!("codesign non-zero exit: {stderr}");
            // Don't fail the install — ad-hoc sign is best-effort.
        }
    }

    let path_shells_updated = ensure_path_configured()?;

    // If PATH already contains ~/.local/bin at runtime, no shell restart needed.
    let home = home_dir()?;
    let target_dir_str = home.join(".local").join("bin");
    let path_has_target = std::env::var("PATH")
        .ok()
        .map(|p| {
            p.split(':')
                .any(|e| std::path::Path::new(e) == target_dir_str)
        })
        .unwrap_or(false);
    let needs_shell_restart = !path_has_target;

    let legacy_install_at = detect_legacy_install()?;

    Ok(InstallResult {
        path: target.display().to_string(),
        path_shells_updated,
        needs_shell_restart,
        legacy_install_at,
    })
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
            detect_legacy_install,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
