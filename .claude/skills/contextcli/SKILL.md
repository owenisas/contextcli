---
name: contextcli
description: |
  Manage CLI tool auth profiles via ContextCLI. Use when the user wants to:
  switch between CLI accounts (Vercel, GitHub, Supabase, AWS, Firebase, etc.),
  run a CLI command under a specific auth profile, link a project directory to a profile,
  set up project policies, check auth status, or manage the contextcli app/config.
  Trigger on: "switch vercel account", "run as work profile", "link this project",
  "contextcli", "cli profile", "which account am I using", "add vercel profile".
---

# ContextCLI

Universal CLI profile launcher. Wraps any dev CLI with multi-profile auth.
Generic router + per-app TOML adapters.

## Quick Reference

```bash
# Forward commands through a profile
contextcli --app vercel --profile work deploy --prod
contextcli --app gh pr list
contextcli --app supabase --profile client-a db push

# Management
contextcli apps                                     # list all tools + status
contextcli profiles --app vercel                    # list profiles
contextcli login --app vercel --profile work        # interactive login
contextcli logout --app vercel --profile work
contextcli default --app vercel --profile work      # set default
contextcli import --app vercel --profile name        # import native credentials
contextcli doctor --app vercel                      # health check
contextcli shell --app vercel --profile work        # shell with auth injected

# Project context
contextcli link --app vercel --profile work         # link cwd → profile
contextcli unlink --app vercel                      # remove link
contextcli project                                  # show project config
```

## Adding a Profile for the User

When the user asks to add/connect a new account for a CLI tool:

### Method 1: Token paste (preferred for agents)

Get an API token from the tool's web dashboard, then store directly:

```bash
# 1. Store token in keychain (macOS) or vault
security add-generic-password -a "<app>/<profile_name>/token" -s "contextcli" -w "<TOKEN>" -U

# 2. Create profile in DB
PROFILE_ID=$(python3 -c "import uuid; print(uuid.uuid4())")
sqlite3 ~/.contextcli/contextcli.db "INSERT INTO profiles (id, app_id, profile_name, label, is_default, auth_state, auth_user, created_at, updated_at) VALUES ('$PROFILE_ID', '<app>', '<profile_name>', '<label>', 0, 'authenticated', '<identity>', datetime('now'), datetime('now'));"

# 3. Add secret reference
REF_ID=$(python3 -c "import uuid; print(uuid.uuid4())")
sqlite3 ~/.contextcli/contextcli.db "INSERT INTO secret_refs (id, profile_id, secret_key, vault_service, vault_account, created_at) VALUES ('$REF_ID', '$PROFILE_ID', 'token', 'contextcli', '<app>/<profile_name>/token', datetime('now'));"

# 4. Verify
contextcli --app <app> --profile <profile_name> whoami
```

### Method 2: Interactive login
```bash
contextcli login --app vercel --profile work
```

### Method 3: Import existing native credentials
```bash
contextcli import --app vercel --profile work
```

### Token dashboard URLs
- **Vercel**: https://vercel.com/account/settings/tokens
- **GitHub**: https://github.com/settings/tokens or `gh auth token`
- **Supabase**: https://supabase.com/dashboard/account/tokens
- **AWS**: IAM console → Security credentials → Access keys
- **Cloudflare**: https://dash.cloudflare.com/profile/api-tokens
- **Netlify**: https://app.netlify.com/user/applications#personal-access-tokens

## Adding a New CLI Tool

Edit `~/.contextcli/adapters.toml`:

```toml
[tool.mycli]
binary = "mycli"
display_name = "My CLI"
env_token = "MYCLI_TOKEN"
whoami_args = ["whoami"]
```

No code. No recompilation. Works immediately.

## Project Config (.contextcli.toml)

Place in project root. Auto-selects profiles + enforces policies:

```toml
[profiles]
vercel = "work"
gh = "work"

[[policies.deny]]
app = "vercel"
profile = "personal"
args_contain = ["deploy", "--prod"]
reason = "Cannot deploy to prod with personal account"
```

## Data Locations

```
~/.contextcli/
├── contextcli.db     # SQLite: apps, profiles, secret_refs, project_links
├── adapters.toml     # Tool definitions
└── configs/          # Isolated config dirs per profile
```

Secrets: macOS Keychain (service `contextcli`, account `<app>/<profile>/token`).
Linux/Windows: `~/.contextcli/vault/` file-based store.
