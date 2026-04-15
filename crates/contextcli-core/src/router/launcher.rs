use crate::adapter::types::InvocationEnv;
use crate::error::Result;
use secrecy::ExposeSecret;
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};

/// Spawn the native CLI binary with the prepared invocation environment.
/// Inherits stdio for transparent pass-through.
pub fn spawn(
    binary: &Path,
    invocation: &InvocationEnv,
    user_args: &[String],
) -> Result<ExitStatus> {
    let mut cmd = Command::new(binary);

    // Prepend adapter's extra args (e.g., --scope=xxx)
    for arg in &invocation.extra_args {
        cmd.arg(arg);
    }

    // Forward user's args verbatim
    for arg in user_args {
        cmd.arg(arg);
    }

    // Inject env vars (secrets exposed only here, only for child process)
    for (key, secret) in &invocation.env_vars {
        cmd.env(key, secret.expose_secret());
    }

    // Transparent stdio pass-through
    cmd.stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    let status = cmd.status()?;
    Ok(status)
}
