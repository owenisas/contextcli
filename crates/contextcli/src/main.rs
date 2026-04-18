mod commands;
mod output;

use clap::Parser;
use contextcli_core::AppContext;
use std::process;

#[derive(Parser)]
#[command(
    name = "contextcli",
    version,
    about = "Universal CLI profile launcher. Run any dev CLI under a named auth profile."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Target CLI app (e.g., vercel, gh, wrangler)
    #[arg(long, global = true)]
    app: Option<String>,

    /// Auth profile to use (uses default if omitted)
    #[arg(long, global = true)]
    profile: Option<String>,

    /// Args forwarded to the native CLI
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    forward_args: Vec<String>,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// Log in to an app with a named profile
    Login {
        #[arg(long)]
        app: String,
        #[arg(long)]
        profile: String,
    },
    /// Log out of a profile (uses default if --profile omitted)
    Logout {
        #[arg(long)]
        app: String,
        #[arg(long)]
        profile: Option<String>,
    },
    /// List profiles for an app
    Profiles {
        #[arg(long)]
        app: String,
    },
    /// Set the default profile for an app
    Default {
        #[arg(long)]
        app: String,
        #[arg(long)]
        profile: String,
    },
    /// Open a shell with a profile's auth context
    Shell {
        #[arg(long)]
        app: String,
        #[arg(long)]
        profile: Option<String>,
    },
    /// Check binary and profile health for an app
    Doctor {
        #[arg(long)]
        app: String,
    },
    /// Import existing credentials from native CLI config
    Import {
        #[arg(long)]
        app: String,
        #[arg(long, default_value = "default")]
        profile: String,
    },
    /// Link an app to a profile in the current project directory
    Link {
        #[arg(long)]
        app: String,
        #[arg(long)]
        profile: String,
    },
    /// Remove an app's profile mapping from the current project
    Unlink {
        #[arg(long)]
        app: String,
    },
    /// Show current project config (.contextcli.toml)
    Project,
    /// Rename a profile
    Rename {
        #[arg(long)]
        app: String,
        #[arg(long)]
        profile: String,
        #[arg(long)]
        to: String,
    },
    /// List all registered apps
    Apps {
        /// Show auth capability details per app
        #[arg(long)]
        auth: bool,
    },
}

/// Split raw argv into (contextcli_args, forward_args).
///
/// When `--app` is present and the remaining args don't start with a known
/// contextcli subcommand, everything that isn't `--app VALUE` or
/// `--profile VALUE` is collected as forward args *before* clap sees them.
/// This lets single-dash flags like `-help`, `-v`, `-e` pass through
/// unmodified instead of being shredded into short-flag bundles by clap.
fn preprocess_args(mut raw: Vec<String>) -> (Vec<String>, Vec<String>) {
    // Known contextcli subcommands — these stay on the clap side.
    const SUBCOMMANDS: &[&str] = &[
        "login", "logout", "profiles", "default", "shell", "doctor",
        "import", "link", "unlink", "project", "rename", "apps",
        "help", "--help", "-h", "--version", "-V",
    ];

    // Check if --app is present at all.
    let has_app = raw.iter().any(|a| a == "--app");
    if !has_app {
        return (raw, vec![]);
    }

    // Check if a known subcommand is present — if so, clap handles everything.
    let has_subcommand = raw
        .iter()
        .any(|a| SUBCOMMANDS.contains(&a.as_str()) && !a.starts_with('-'));
    if has_subcommand {
        return (raw, vec![]);
    }

    // Forwarding mode: extract --app, --profile (and their values) for clap;
    // everything else becomes forward_args that bypass clap entirely.
    let mut cli_args = vec![raw.remove(0)]; // keep argv[0] (program name)
    let mut forward_args: Vec<String> = vec![];
    let mut i = 0;

    while i < raw.len() {
        let arg = &raw[i];
        if arg == "--app" || arg == "--profile" {
            cli_args.push(raw[i].clone());
            i += 1;
            if i < raw.len() {
                cli_args.push(raw[i].clone());
            }
        } else if arg == "--" {
            // Explicit separator: everything after goes to forward_args.
            i += 1;
            forward_args.extend(raw[i..].iter().cloned());
            break;
        } else {
            forward_args.push(raw[i].clone());
        }
        i += 1;
    }

    (cli_args, forward_args)
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("contextcli=warn".parse().unwrap()),
        )
        .with_target(false)
        .init();

    let raw: Vec<String> = std::env::args().collect();
    let (cli_args, pre_forward) = preprocess_args(raw);

    let mut cli = Cli::parse_from(cli_args);

    // Merge pre-extracted forward args (they bypass clap's flag parser).
    if !pre_forward.is_empty() {
        cli.forward_args = pre_forward;
    }

    let ctx = match AppContext::init() {
        Ok(ctx) => ctx,
        Err(e) => {
            output::error(&format!("failed to initialize: {e}"));
            process::exit(1);
        }
    };

    let result = match cli.command {
        Some(Commands::Login { app, profile }) => commands::login::run(&ctx, &app, &profile),
        Some(Commands::Logout { app, profile }) => {
            commands::logout::run(&ctx, &app, profile.as_deref())
        }
        Some(Commands::Profiles { app }) => commands::profiles::run(&ctx, &app),
        Some(Commands::Default { app, profile }) => commands::default::run(&ctx, &app, &profile),
        Some(Commands::Shell { app, profile }) => {
            commands::shell::run(&ctx, &app, profile.as_deref())
        }
        Some(Commands::Doctor { app }) => commands::doctor::run(&ctx, &app),
        Some(Commands::Import { app, profile }) => commands::import::run(&ctx, &app, &profile),
        Some(Commands::Rename { app, profile, to }) => {
            commands::rename::run(&ctx, &app, &profile, &to)
        }
        Some(Commands::Link { app, profile }) => commands::link::run(&ctx, &app, &profile),
        Some(Commands::Unlink { app }) => commands::link::unlink(&ctx, &app),
        Some(Commands::Project) => commands::link::show(),
        Some(Commands::Apps { auth }) => commands::apps::run(&ctx, auth),
        None => {
            match cli.app {
                Some(app) => {
                    commands::forward::run(&ctx, &app, cli.profile.as_deref(), &cli.forward_args)
                }
                None => {
                    output::error("either provide a subcommand or --app with args to forward");
                    output::hint("examples:");
                    output::hint("  contextcli --app vercel --profile work deploy --prod");
                    output::hint("  contextcli login --app vercel --profile work");
                    output::hint("  contextcli apps");
                    process::exit(1);
                }
            }
        }
    };

    if let Err(e) = result {
        output::error(&e.to_string());
        process::exit(1);
    }
}
