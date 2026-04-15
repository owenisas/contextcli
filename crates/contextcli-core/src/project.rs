//! Project-level config: .contextcli.toml
//!
//! Placed in any project directory. Maps apps to profiles and defines policies.
//! When running from that directory (or a subdirectory), the router auto-selects
//! the mapped profile and enforces policies.

use crate::error::{Error, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const CONFIG_FILENAME: &str = ".contextcli.toml";

/// Project-level configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct ProjectConfig {
    /// Map of app_id → profile_name.
    /// When running from this project, these profiles are auto-selected.
    #[serde(default)]
    pub profiles: HashMap<String, String>,

    /// Policy rules to enforce.
    #[serde(default)]
    pub policies: Policies,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Policies {
    /// Commands to deny. If a forwarded command matches, it's blocked.
    #[serde(default)]
    pub deny: Vec<DenyRule>,
}

/// A deny rule blocks a command if all conditions match.
#[derive(Debug, Clone, Deserialize)]
pub struct DenyRule {
    /// App this rule applies to. Required.
    pub app: String,

    /// Profile this rule applies to. If omitted, applies to all profiles.
    #[serde(default)]
    pub profile: Option<String>,

    /// Block if any of these strings appear in the forwarded args.
    #[serde(default)]
    pub args_contain: Vec<String>,

    /// Human-readable reason shown when blocked.
    #[serde(default)]
    pub reason: Option<String>,
}

impl ProjectConfig {
    /// Get the mapped profile for an app, if any.
    pub fn profile_for(&self, app_id: &str) -> Option<&str> {
        self.profiles.get(app_id).map(|s| s.as_str())
    }

    /// Check if a command is denied by policy.
    /// Returns the deny reason if blocked, None if allowed.
    pub fn check_policy(
        &self,
        app_id: &str,
        profile_name: &str,
        forward_args: &[String],
    ) -> Option<String> {
        for rule in &self.policies.deny {
            if rule.app != app_id {
                continue;
            }

            // Check profile match (if specified)
            if let Some(rule_profile) = &rule.profile {
                if rule_profile != profile_name {
                    continue;
                }
            }

            // Check args_contain — all patterns must match
            if rule.args_contain.is_empty() {
                continue; // No arg patterns = rule doesn't apply
            }

            let all_match = rule.args_contain.iter().all(|pattern| {
                forward_args.iter().any(|arg| arg.contains(pattern))
            });

            if all_match {
                let reason = rule.reason.clone().unwrap_or_else(|| {
                    format!(
                        "blocked by project policy: {} with profile '{}' cannot use [{}]",
                        app_id,
                        profile_name,
                        rule.args_contain.join(", ")
                    )
                });
                return Some(reason);
            }
        }

        None
    }
}

/// Search for .contextcli.toml starting from `start_dir` and walking up.
/// Returns (config, path_to_config_file) if found.
pub fn find_project_config(start_dir: &Path) -> Option<(ProjectConfig, PathBuf)> {
    let mut dir = start_dir.to_path_buf();
    loop {
        let config_path = dir.join(CONFIG_FILENAME);
        if config_path.is_file() {
            match load_config(&config_path) {
                Ok(config) => return Some((config, config_path)),
                Err(e) => {
                    tracing::warn!("invalid {}: {e}", config_path.display());
                    return None;
                }
            }
        }
        if !dir.pop() {
            break;
        }
    }
    None
}

fn load_config(path: &Path) -> Result<ProjectConfig> {
    let content = std::fs::read_to_string(path)?;
    toml::from_str(&content).map_err(|e| Error::Other(format!("invalid .contextcli.toml: {e}")))
}

/// Write a .contextcli.toml file to the given directory.
pub fn write_project_config(dir: &Path, config: &ProjectConfig) -> Result<()> {
    let path = dir.join(CONFIG_FILENAME);
    let mut content = String::new();

    // Write profiles section
    if !config.profiles.is_empty() {
        content.push_str("[profiles]\n");
        let mut sorted: Vec<_> = config.profiles.iter().collect();
        sorted.sort_by_key(|(k, _)| (*k).clone());
        for (app, profile) in sorted {
            content.push_str(&format!("{app} = \"{profile}\"\n"));
        }
    }

    // Write policies section
    if !config.policies.deny.is_empty() {
        content.push_str("\n[policies]\n");
        for rule in &config.policies.deny {
            content.push_str("[[policies.deny]]\n");
            content.push_str(&format!("app = \"{}\"\n", rule.app));
            if let Some(profile) = &rule.profile {
                content.push_str(&format!("profile = \"{profile}\"\n"));
            }
            if !rule.args_contain.is_empty() {
                let args: Vec<String> = rule.args_contain.iter().map(|a| format!("\"{a}\"")).collect();
                content.push_str(&format!("args_contain = [{}]\n", args.join(", ")));
            }
            if let Some(reason) = &rule.reason {
                content.push_str(&format!("reason = \"{reason}\"\n"));
            }
        }
    }

    std::fs::write(&path, content)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_config() {
        let toml = r#"
[profiles]
vercel = "work"
gh = "work"

[policies]
[[policies.deny]]
app = "vercel"
profile = "personal"
args_contain = ["--prod"]
reason = "Cannot deploy to prod with personal account"
"#;
        let config: ProjectConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.profile_for("vercel"), Some("work"));
        assert_eq!(config.profile_for("gh"), Some("work"));
        assert_eq!(config.profile_for("aws"), None);
        assert_eq!(config.policies.deny.len(), 1);
    }

    #[test]
    fn test_policy_blocks() {
        let toml = r#"
[profiles]
vercel = "work"

[[policies.deny]]
app = "vercel"
profile = "personal"
args_contain = ["deploy", "--prod"]
"#;
        let config: ProjectConfig = toml::from_str(toml).unwrap();

        // Should block: deploy --prod with personal
        let args = vec!["deploy".to_string(), "--prod".to_string()];
        assert!(config.check_policy("vercel", "personal", &args).is_some());

        // Should allow: deploy --prod with work
        assert!(config.check_policy("vercel", "work", &args).is_none());

        // Should allow: deploy without --prod with personal
        let args2 = vec!["deploy".to_string()];
        assert!(config.check_policy("vercel", "personal", &args2).is_none());

        // Should allow: different app
        assert!(config.check_policy("gh", "personal", &args).is_none());
    }

    #[test]
    fn test_policy_no_profile_filter() {
        let toml = r#"
[[policies.deny]]
app = "aws"
args_contain = ["s3", "rm", "--recursive"]
reason = "Recursive S3 delete blocked for all profiles"
"#;
        let config: ProjectConfig = toml::from_str(toml).unwrap();
        let args = vec!["s3".to_string(), "rm".to_string(), "--recursive".to_string(), "s3://bucket".to_string()];
        let result = config.check_policy("aws", "any-profile", &args);
        assert!(result.is_some());
        assert!(result.unwrap().contains("Recursive S3 delete"));
    }
}
