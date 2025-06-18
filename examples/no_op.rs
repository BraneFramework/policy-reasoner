//  NO OP.rs
//    by Lut99
//
//  Created:
//    10 Oct 2024, 16:17:21
//  Last edited:
//    02 Dec 2024, 14:34:47
//  Auto updated?
//    Yes
//
//  Description:
//!   Showcases the reasoner with a super dummy backend reasoner that
//!   always accepts anything.
//

use std::process::ExitCode;

use clap::Parser;
use console::style;
use miette::{Context as _, IntoDiagnostic as _};
use policy_reasoner::loggers::file::FileLogger;
use policy_reasoner::reasoners::no_op::NoOpReasonerConnector;
use policy_reasoner::spec::ReasonerConnector as _;
use policy_reasoner::spec::auditlogger::SessionedAuditLogger;
use policy_reasoner::spec::reasonerconn::ReasonerResponse;
use tracing::{Level, error, info};


/***** ARGUMENTS *****/
/// The arguments for this binary.
#[derive(Parser)]
pub struct Arguments {
    /// Whether to make `info!()` and `debug!()` visible.
    #[clap(long, help = "If given, enables INFO- and DEBUG-level logging.")]
    debug: bool,
    /// Whether to make `trace!()` visible.
    #[clap(long, help = "If given, enables TRACE-level logging. Implies '--debug'.")]
    trace: bool,
}





/***** ENTRYPOINT *****/
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

async fn run(_args: Arguments) -> miette::Result<()> {
    // Create the logger
    let mut logger =
        SessionedAuditLogger::new("test", FileLogger::new(format!("{} - v{}", env!("CARGO_BIN_NAME"), env!("CARGO_PKG_VERSION")), "./test.log"));

    // Run the reasoner
    let conn = NoOpReasonerConnector::new_async(&mut logger).await.into_diagnostic().context("Failed to setup no-op reasoner")?;
    let verdict = conn.consult((), (), &logger).await.unwrap();

    // OK, report
    match verdict {
        ReasonerResponse::Success => println!("{} {}", style("Reasoner says:").bold(), style("OK").bold().green()),
        ReasonerResponse::Violated(_) => {
            println!("{} {}", style("Reasoner says:").bold(), style("VIOLATION").bold().red());
        },
    }

    Ok(())
}
