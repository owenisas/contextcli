ContextCLI is a universal CLI profile launcher. It wraps dev CLIs (Vercel, GitHub, Supabase, AWS, etc.) with multi-profile auth support.

Use `contextcli --app <tool> --profile <name> <command>` to run any CLI under a specific auth profile.
Use `contextcli apps` to see all registered tools and their status.
Use `contextcli profiles --app <tool>` to list profiles.

Config at `~/.contextcli/adapters.toml`. Project config at `.contextcli.toml`.
Secrets in macOS Keychain (service: "contextcli").

This is a Rust workspace with Tauri v2 + React GUI.
Core library: `crates/contextcli-core/`. CLI: `crates/contextcli/`. GUI: `src-tauri/` + `ui/`.
