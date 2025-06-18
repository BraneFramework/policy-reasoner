//  EFLINT HASKELL.rs
//    by Lut99
//
//  Created:
//    06 May 2025, 11:09:11
//  Last edited:
//    06 May 2025, 11:17:02
//  Auto updated?
//    Yes
//
//  Description:
//!   Showcases using the `eflint-haskell-reasoner` backend of the policy
//!   reasoner.
//

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use console::style;
use miette::{Context, IntoDiagnostic};
use policy_reasoner::loggers::file::FileLogger;
use policy_reasoner::reasoners::eflint_haskell::EFlintHaskellReasonerConnector;
use policy_reasoner::reasoners::eflint_haskell::reasons::SilentHandler;
use policy_reasoner::spec::auditlogger::SessionedAuditLogger;
use policy_reasoner::spec::reasonerconn::ReasonerConnector as _;
use policy_reasoner::spec::reasons::NoReason;
use spec::reasonerconn::ReasonerResponse;
use tracing::{Level, error, info};


/***** ARGUMENTS *****/
/// Defines the arguments for this binary.
#[derive(Parser)]
struct Arguments {
    /// Whether to make `info!()` and `debug!()` visible.
    #[clap(long, help = "If given, enables INFO- and DEBUG-level logging.")]
    debug: bool,
    /// Whether to make `trace!()` visible.
    #[clap(long, help = "If given, enables TRACE-level logging. Implies '--debug'.")]
    trace: bool,

    /// The file to use as input.
    #[clap(name = "FILE", default_value = "-", help = "The eFLINT (JSON) file to read. Use '-' to read from stdin.")]
    file: String,

    /// Which `eflint-repl` to use.
    #[clap(short, long, default_value = "eflint-repl", help = "The command used to launch the `eflint-repl` binary.")]
    eflint_cmd: String,
}





/***** LIBRARY *****/
#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    // Parse the arguments
    let args = Arguments::parse();

    // Setup the logger
    tracing_subscriber::fmt()
        .with_max_level(if args.trace {
            Level::TRACE
        } else if args.debug {
            Level::DEBUG
        } else {
            Level::WARN
        })
        .init();
    info!("{} - v{}", env!("CARGO_BIN_NAME"), env!("CARGO_PKG_VERSION"));

    match run(args).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            error!("{err:?}");
            ExitCode::FAILURE
        },
    }
}

async fn run(args: Arguments) -> miette::Result<()> {
    // Create the logger
    let logger: SessionedAuditLogger<FileLogger> =
        SessionedAuditLogger::new("test", FileLogger::new(format!("{} - v{}", env!("CARGO_BIN_NAME"), env!("CARGO_PKG_VERSION")), "./test.log"));

    // Ensure there is a file to input
    let policy: PathBuf = if args.file == "-" {
        let file: PathBuf = std::env::temp_dir().join(format!("{}-v{}-stdin.eflint", env!("CARGO_BIN_NAME"), env!("CARGO_PKG_VERSION")));
        let mut handle = tokio::fs::File::create(&file)
            .await
            .into_diagnostic()
            .with_context(|| format!("Failed to open temporary stdin file '{}'", file.display()))?;

        tokio::io::copy(&mut tokio::io::stdin(), &mut handle)
            .await
            .into_diagnostic()
            .with_context(|| format!("Failed to write stdin to temporary file '{}'", file.display()))?;

        file
    } else {
        PathBuf::from(args.file)
    };

    // Create the reasoner
    let conn = EFlintHaskellReasonerConnector::<SilentHandler, String, ()>::new_async(
        shlex::split(&args.eflint_cmd).into_iter().flatten(),
        &policy,
        SilentHandler,
        &logger,
    )
    .await
    .into_diagnostic()
    .context("Failed to create eFLINT reasoner")?;

    let verdict: ReasonerResponse<NoReason> = conn
        .consult("".into(), (), &logger)
        .await
        .into_diagnostic()
        .with_context(|| format!("Failed to send message to reasoner {:?}", args.eflint_cmd))?;

    // OK, report
    match verdict {
        ReasonerResponse::Success => println!("{} {}", style("Reasoner says:").bold(), style("OK").bold().green()),
        ReasonerResponse::Violated(reasons) => {
            println!("{} {}", style("Reasoner says:").bold(), style("VIOLATION").bold().red());
            println!("Reason:");
            println!("{reasons}");
            println!();
        },
    }

    Ok(())
}
