use crate::output;
use contextcli_core::AppContext;
use contextcli_core::error::Result;
use std::process;

pub fn run(
    ctx: &AppContext,
    app: &str,
    profile: Option<&str>,
    forward_args: &[String],
) -> Result<()> {
    if forward_args.is_empty() {
        output::error("no command to forward");
        output::hint(&format!(
            "example: contextcli --app {} --profile work deploy",
            app
        ));
        process::exit(1);
    }

    let router = ctx.router();
    let status = router.forward(app, profile, forward_args)?;

    // Exit with the same code as the native CLI
    process::exit(status.code().unwrap_or(1));
}
