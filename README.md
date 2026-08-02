# ContextCLI

> **One machine. Multiple accounts. Zero friction.**

ContextCLI is a universal CLI profile launcher. Run any developer CLI under a named auth profile — switch between Vercel, GitHub, Supabase, AWS and a dozen more without re-logging in, sharing tokens, or littering your shell with env exports.

```bash
contextcli --app vercel --profile work deploy --prod
contextcli --app gh --profile personal pr list
contextcli --app supabase --profile client-a db push
```

Written in **Rust** (core + CLI) with a **Tauri v2 + React** desktop app. Adapters are data-driven — add a new tool by editing TOML, no Rust code, no recompilation. Tokens live in the **macOS Keychain**, never in plain files.

---

## How It Works

ContextCLI sits between you and your native CLIs. It resolves the named profile, pulls the token from macOS Keychain, injects it as an environment variable, and forwards your command — unchanged — to the real binary.

```
You → contextcli --app vercel --profile work deploy
                    ↓
        Resolve "work" profile for Vercel
        Retrieve token from macOS Keychain
        Set VERCEL_TOKEN=<token>
                    ↓
        Spawn: vercel deploy  (with injected env)
                    ↓
        Output passes through transparently
```

No shims. No proxy processes. No token files on disk. ContextCLI wraps your existing tooling — it doesn't replace it.

---

## Installation

| Platform | Method | Command / Link |
|----------|--------|----------------|
| **macOS** | Homebrew (recommended) | `brew install owenisas/contextcli/contextcli` |
| **macOS GUI** | Homebrew cask | `brew install --cask owenisas/contextcli/contextcli-gui` |
| **macOS** | Binary | [Releases](https://github.com/owenisas/contextcli/releases/latest) — `.tar.gz`, ARM + Intel |
| **Linux** | Binary | [Releases](https://github.com/owenisas/contextcli/releases/latest) — `x86_64-unknown-linux-gnu.tar.gz` |
| **Windows** | Binary | [Releases](https://github.com/owenisas/contextcli/releases/latest) — `x86_64-pc-windows-msvc.zip` |
| **Any** | From source | `cargo install --path crates/contextcli` |

<details>
<summary><b>Manual macOS install</b></summary>

```bash
# Apple Silicon
curl -L https://github.com/owenisas/contextcli/releases/latest/download/contextcli-v0.1.0-aarch64-apple-darwin.tar.gz | tar xz
sudo cp contextcli /usr/local/bin/

# Intel
curl -L https://github.com/owenisas/contextcli/releases/latest/download/contextcli-v0.1.0-x86_64-apple-darwin.tar.gz | tar xz
sudo cp contextcli /usr/local/bin/
```

**Desktop app:** download the `.dmg` from the [latest release](https://github.com/owenisas/contextcli/releases/latest), drag ContextCLI to Applications, launch and click **"Install CLI Tool"** in the sidebar.

</details>

<details>
<summary><b>Linux install</b></summary>

```bash
curl -L https://github.com/owenisas/contextcli/releases/latest/download/contextcli-v0.1.0-x86_64-unknown-linux-gnu.tar.gz | tar xz
sudo cp contextcli /usr/local/bin/
```

The GUI is also available — download `ContextCLI-*-linux-gnu.tar.gz` from releases.

</details>

<details>
<summary><b>Windows install</b></summary>

Download `contextcli-*-x86_64-pc-windows-msvc.zip` from the [latest release](https://github.com/owenisas/contextcli/releases/latest). Extract and add to your PATH.

The GUI bundle (`ContextCLI-*-windows-msvc.zip`) includes both `contextcli.exe` and `contextcli-gui.exe`.

</details>

<details>
<summary><b>Build from source</b></summary>

Requires [Rust](https://rustup.rs/) and [pnpm](https://pnpm.io/).

```bash
git clone https://github.com/owenisas/contextcli.git
cd contextcli

# CLI only
cargo install --path crates/contextcli

# GUI (optional)
cd ui && pnpm install && pnpm build && cd ..
cargo build --release -p contextcli-gui
```

</details>

---

## Quick Start

```bash
# First run — auto-detects existing credentials from native CLIs
contextcli apps

# Your Vercel, GitHub, Supabase, Firebase, Railway tokens
# are already imported. Use them immediately:
contextcli --app vercel whoami
contextcli --app gh api user --jq .login

# Add a second account
# (create an API token in the web dashboard, then import)
contextcli login --app vercel --profile work

# Switch between accounts
contextcli --app vercel --profile work deploy
contextcli --app vercel --profile personal env pull

# Set a default
contextcli default --app vercel --profile work
```

---

## CLI Reference

### Forwarding (primary use)

```bash
contextcli --app <tool> [--profile <name>] <command and args>
```

Everything after `--app` and `--profile` is forwarded **verbatim** to the native CLI. Omit `--profile` to use the default.

### Management Commands

| Command | Description |
|---------|-------------|
| `contextcli apps` | List all registered tools + binary status |
| `contextcli profiles --app <tool>` | List profiles for a tool |
| `contextcli login --app <tool> --profile <name>` | Interactive login |
| `contextcli logout --app <tool> --profile <name>` | Clear credentials |
| `contextcli default --app <tool> --profile <name>` | Set default profile |
| `contextcli import --app <tool> --profile <name>` | Import native credentials |
| `contextcli doctor --app <tool>` | Health check (binary + profiles) |
| `contextcli shell --app <tool> --profile <name>` | Open shell with auth injected |
| `contextcli link --app <tool> --profile <name>` | Link current directory to profile |
| `contextcli unlink --app <tool>` | Remove directory link |
| `contextcli project` | Show current project config |

---

## Supported Tools (15 built-in)

All defined in `~/.contextcli/adapters.toml` — edit it to add any CLI.

| Tool | Env Var | Auto-Import |
|------|---------|-------------|
| Vercel | `VERCEL_TOKEN` | ✓ reads `auth.json` |
| GitHub CLI | `GH_TOKEN` | ✓ via `gh auth token` |
| Supabase | `SUPABASE_ACCESS_TOKEN` | ✓ macOS Keychain |
| AWS | `AWS_ACCESS_KEY_ID` + `SECRET` | config file |
| Firebase | `FIREBASE_TOKEN` | ✓ multi-account |
| Railway | `RAILWAY_TOKEN` | ✓ reads `config.json` |
| Cloudflare Wrangler | `CLOUDFLARE_API_TOKEN` | — |
| Netlify | `NETLIFY_AUTH_TOKEN` | — |
| Fly.io | `FLY_ACCESS_TOKEN` | config file |
| Heroku | `HEROKU_API_KEY` | — |
| DigitalOcean | `DIGITALOCEAN_ACCESS_TOKEN` | — |
| Terraform | `TF_TOKEN_app_terraform_io` | — |
| Docker | `DOCKER_CONFIG` | — |
| npm | `NPM_TOKEN` | — |
| kubectl | `KUBECONFIG` | — |

### Adding a Custom Tool

Edit `~/.contextcli/adapters.toml`:

```toml
[tool.mycli]
binary = "mycli"
display_name = "My CLI"
env_token = "MYCLI_TOKEN"
whoami_args = ["whoami"]
```

No Rust code. No recompilation. Works immediately.

---

## Project Config

Place `.contextcli.toml` in any project root:

```toml
[profiles]
vercel = "work"
gh = "work"
supabase = "client-a"

[[policies.deny]]
app = "vercel"
profile = "personal"
args_contain = ["deploy", "--prod"]
reason = "Cannot deploy to prod with personal account"
```

- **Auto-switch** — commands in this directory auto-use the mapped profile.
- **Policies** — block dangerous command + profile combos before they reach the CLI.
- **Explicit override** — the `--profile` flag always wins over project config.

---

## Desktop App

A native desktop companion — Tauri v2 + React, dark theme, profile management at a glance.

| Feature | |
|---------|-|
| Sidebar | All apps in one view |
| Profile cards | Add, delete, set default, test connection |
| Project mappings | Open-in-Finder / open-in-Terminal |
| Auth status | Auto-refreshes on window focus |
| Network | No localhost port in production — frontend is embedded in the binary |

```bash
# Build & run
cd ui && pnpm install && pnpm build && cd ..
cargo build --release -p contextcli-gui
codesign --force --sign - --identifier "com.contextcli.app" target/release/contextcli-gui
./target/release/contextcli-gui
```

---

## Architecture

```
contextcli-core        ← Shared Rust library
    ↑           ↑
contextcli    contextcli-gui
(CLI)         (Tauri + React)
```

| Layer | Detail |
|-------|--------|
| **Adapters** | Data-driven via `adapters.toml` — a generic adapter reads the TOML; no per-tool Rust code |
| **Secrets** | macOS Keychain via `security-framework`, wrapped in `secrecy::SecretString` |
| **Database** | SQLite (`~/.contextcli/contextcli.db`) — apps, profiles, secret_refs, project_links |
| **Auth flow** | Env var injection (preferred) or config-dir isolation |
| **Project context** | `.contextcli.toml` walks up directories like `.git` |

---

## AI Agent Integration

ContextCLI ships skills/instructions for AI coding agents. Clone the repo and your agent automatically learns how to manage CLI profiles.

| Agent | File | Auto-discovered |
|-------|------|-----------------|
| **Claude Code** | `.claude/skills/contextcli/SKILL.md` | Yes, on clone |
| **Windsurf** | `.windsurf/skills/contextcli/SKILL.md` | Yes, on clone |
| **Cursor** | `.cursorrules` | Yes, on clone |
| **GitHub Copilot** | `.github/copilot-instructions.md` | Yes, on clone |

### Use everywhere (not just this repo)

```bash
# Claude Code
cp -r .claude/skills/contextcli/ ~/.claude/skills/contextcli/

# Windsurf
cp -r .windsurf/skills/contextcli/ ~/.windsurf/skills/contextcli/
```

Once the skill is loaded, your agent can:

- Run CLI commands under specific profiles
- Add new profiles (stores token in Keychain + creates DB records)
- Link projects to profiles
- Set up deny policies
- Check auth status across all tools

---

## macOS Keychain Authorization

When a credential is first stored (or was stored by an older ContextCLI version), macOS may show a **"contextcli wants to use your keychain"** dialog once per profile.

**Why:** the legacy Keychain API (`SecKeychainAddGenericPassword`) ties each item to the binary's code-signature hash. Every `cargo build` produces a new hash → a new prompt.

**The fix — one click per profile, then never again.** ContextCLI automatically upgrades items to a **permissive ACL** on first access. You only need to authorize each profile **once**:

1. Run any command with the affected profile:
   ```bash
   contextcli --app vercel --profile work whoami
   ```
2. Click **Always Allow** in the dialog.
3. Done — that profile is permanently unlocked for any binary.

### Which profiles need auth?

**CLI:**

```bash
contextcli profiles --app vercel
```

Profiles needing action show a `⚠ needs keychain auth` warning with the exact command to run.

**GUI:** each affected profile card shows an amber banner:

> ⚠ **Needs one-time keychain authorization.** Run any command with this profile and click **Always Allow** — never prompted again.

The banner disappears automatically after you authorize.

---

## Security

- **Keychain-first** — tokens stored in macOS Keychain, never in plain files.
- **Zeroized secrets** — in-memory tokens wrapped in `SecretString`, erased on drop.
- **Env var injection over CLI flags** — credentials aren't visible in `ps`.
- **No tokens in logs or activity records.**
- **File permissions** — dirs `0700`, files `0600`.
- **Policy rules** — block dangerous command + profile combos before they run.

---

## License

MIT
