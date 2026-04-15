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
    /// Log out of a profile
    Logout {
        #[arg(long)]
        app: String,
        #[arg(long)]
        profile: String,
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
    Apps,
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("contextcli=warn".parse().unwrap()),
        )
        .with_target(false)
        .init();

    let cli = Cli::parse();

    let ctx = match AppContext::init() {
        Ok(ctx) => ctx,
        Err(e) => {
            output::error(&format!("failed to initialize: {e}"));
            process::exit(1);
        }
    };

    let result = match cli.command {
        Some(Commands::Login { app, profile }) => commands::login::run(&ctx, &app, &profile),
        Some(Commands::Logout { app, profile }) => commands::logout::run(&ctx, &app, &profile),
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
        Some(Commands::Apps) => commands::apps::run(&ctx),
        None => {
            // Forwarding mode: --app required
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
