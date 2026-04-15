# ContextCLI

Rust workspace: `contextcli-core` (shared lib), `contextcli` (CLI bin), `contextcli-gui` (Tauri v2 + React).

## Build
```
cargo build --workspace              # all
cargo test --workspace               # 17 tests
cd ui && pnpm build && cd ..         # frontend
cargo build --release -p contextcli-gui  # GUI release
```

## Architecture
- Adapters are data-driven via `~/.contextcli/adapters.toml` — `crates/contextcli-core/src/adapter/generic.rs`
- Router: `crates/contextcli-core/src/router/mod.rs` — resolve profile → policy check → env inject → spawn
- Project config: `crates/contextcli-core/src/project.rs` — `.contextcli.toml` parsing
- Vault: macOS Keychain (`vault/keychain.rs`) or file-based (`vault/file_vault.rs`) on Linux/Windows
- Tauri IPC: `src-tauri/src/lib.rs` — thin wrappers around core

## Key rules
- Never log tokens or secrets
- Use `secrecy::SecretString` for all in-memory credentials
- `security-framework` is macOS-only (conditional compilation)
- Adapters.toml is the source of truth for tool definitions — no hardcoded adapters
