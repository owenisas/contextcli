# ContextCLI

Universal CLI profile launcher. Run any developer CLI under a named auth profile.

```bash
contextcli --app vercel --profile work deploy --prod
contextcli --app gh --profile personal pr list
contextcli --app supabase --profile client-a db push
```

One machine, multiple accounts, zero friction. ContextCLI wraps your existing CLIs — it doesn't replace them.

## How It Works

ContextCLI sits between you and your CLI tools. It resolves which auth profile to use, injects the right credentials (via environment variables), and forwards your command unchanged to the real binary.

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

## Installation

### Desktop App (recommended)

Download `ContextCLI-v0.1.0-aarch64-apple-darwin.zip` from the [latest release](https://github.com/owenisas/contextcli/releases/latest).

```bash
unzip ContextCLI-*.zip
cp -r ContextCLI.app /Applications/
```

Open the app. Click **"Install CLI Tool"** in the sidebar to install the `contextcli` command to your PATH. One download, both GUI and CLI.

### CLI Only

```bash
# Download
curl -L https://github.com/owenisas/contextcli/releases/latest/download/contextcli-v0.1.0-aarch64-apple-darwin.tar.gz | tar xz

# Install
sudo cp contextcli /usr/local/bin/
```

### Homebrew

```bash
# CLI
brew install owenisas/contextcli/contextcli

# Desktop app
brew install --cask owenisas/contextcli/contextcli-gui
```

### From Source

Requires Rust toolchain and pnpm.

```bash
git clone https://github.com/owenisas/contextcli.git
cd contextcli

# CLI only
cargo install --path crates/contextcli

# GUI (optional)
cd ui && pnpm install && pnpm build && cd ..
cargo build --release -p contextcli-gui
```

## Quick Start

```bash
# First run — auto-detects existing credentials from native CLIs
contextcli apps

# That's it. Your Vercel, GitHub, Supabase, Firebase, Railway tokens
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

## CLI Reference

### Forwarding (primary use)

```bash
contextcli --app <tool> [--profile <name>] <command and args>
```

Everything after `--app` and `--profile` is forwarded verbatim to the native CLI. No `--profile` = uses default.

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

## Supported Tools (15 built-in)

All defined in `~/.contextcli/adapters.toml`. Edit to add any CLI.

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

**Auto-switch**: Commands in this directory auto-use the mapped profile.
**Policies**: Block dangerous command + profile combos before they reach the CLI.
**Explicit override**: `--profile` flag always wins over project config.

## Desktop App

Tauri v2 + React. Dark theme. Manage profiles visually.

```bash
# Build & run
cd ui && pnpm install && pnpm build && cd ..
cargo build --release -p contextcli-gui
codesign --force --sign - --identifier "com.contextcli.app" target/release/contextcli-gui
./target/release/contextcli-gui
```

Features: sidebar with all apps, profile cards, add/delete profiles, set default, test connection, project mappings with open-in-Finder/Terminal. Auto-refreshes on window focus.

**No localhost port in production** — frontend is embedded in the binary.

## Architecture

```
contextcli-core        ← Shared Rust library
    ↑           ↑
contextcli    contextcli-gui
(CLI)         (Tauri + React)
```

- **Adapters**: Data-driven via `adapters.toml` — generic adapter reads TOML, no per-tool Rust code
- **Secrets**: macOS Keychain via `security-framework`, wrapped in `secrecy::SecretString`
- **Database**: SQLite (`~/.contextcli/contextcli.db`) — apps, profiles, secret_refs, project_links
- **Auth flow**: env var injection (preferred) or config dir isolation
- **Project context**: `.contextcli.toml` walks up directories like `.git`

## Security

- Tokens stored in macOS Keychain, never in plain files
- In-memory secrets wrapped in `SecretString` (zeroized on drop)
- Env var injection over CLI flags (not visible in `ps`)
- No tokens in logs or activity records
- File permissions: dirs `0700`, files `0600`
- Policy rules block dangerous command combos

## License

MIT
