//  UNIX.rs
//    by Lut99
//
//  Created:
//    11 Oct 2024, 16:32:29
//  Last edited:
//    17 Oct 2024, 12:00:53
//  Auto updated?
//    Yes
//
//  Description:
//!   Showcases the reasoner with a backend that overlays Unix file
//!   persmissions.
//

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use console::style;
use file_logger::FileLogger;
use miette::{Context as _, IntoDiagnostic as _};
use policy_reasoner::reasoners::posix::{PosixReasonerConnector, State};
use policy_reasoner::spec::ReasonerConnector as _;
use policy_reasoner::spec::auditlogger::SessionedAuditLogger;
use policy_reasoner::workflow::Workflow;
use posix_reasoner::config::Config;
use share::InputFile;
use spec::reasonerconn::ReasonerResponse;
use tokio::fs;
use tokio::io::{self, AsyncReadExt as _};
use tracing::{Level, debug, error, info};


/***** HELPER FUNCTIONS *****/
/// Reads a [`Workflow`] from either stdin or disk.
///
/// # Arguments
/// - `input`: Either '-' to read from stdin, or a path of the file to read from otherwise.
///
/// # Returns
/// A parsed [`Workflow`] file.
///
/// # Errors
/// This function errors if it failed to read stdin OR the file, or parse it as a valid Workflow.
async fn load_workflow(input: &InputFile) -> miette::Result<Workflow> {
    let workflow = match input {
        InputFile::Stdin => {
            debug!("Reading workflow from stdin");
            let mut raw: Vec<u8> = Vec::new();
            io::stdin().read_buf(&mut raw).await.into_diagnostic().context("Failed to read from stdin")?;
            String::from_utf8(raw).into_diagnostic().context("Stdin is not valid UTF-8")?
        },
        InputFile::File(path) => {
            debug!("Reading workflow from file");
            fs::read_to_string(path)
                .await
                .into_diagnostic()
                .with_context(|| format!("Failed to read the workflow from {input}", input = input.display()))?
        },
    };

    let workflow = serde_json::from_str(&workflow).into_diagnostic().with_context(|| format!("{input:?} is not a valid workflow"))?;

    Ok(workflow)
}

/// Reads a [`Config`] from disk.
///
/// # Arguments
/// - `path`: The path to the config file to load.
///
/// # Returns
/// A parsed [`Config`] file.
///
/// # Errors
/// This function errors if it failed to read the file, or it did not contain a valid config.
async fn load_config(path: PathBuf) -> miette::Result<Config> {
    // Load the file and parse it
    let config: String =
        fs::read_to_string(&path).await.into_diagnostic().with_context(|| format!("Failed to read the config file {path}", path = path.display()))?;

    let mut config: Config =
        serde_json::from_str(&config).into_diagnostic().with_context(|| format!("File {path} is not a valid config file", path = path.display()))?;

    // Resolve relative files to relative to the binary, for consistency of calling the example
    let path = std::env::current_exe().into_diagnostic().context("Failed to obtain the current executable's path")?;
    let prefix = if let Some(parent) = path.parent() { parent.into() } else { path };

    for path in config.data.values_mut().map(|data| &mut data.path) {
        if path.is_relative() {
            *path = prefix.join(&*path);
        }
    }
    debug!("Config after resolving relative paths: {config:?}");

    // Done
    Ok(config)
}



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

    /// The file containing the workflow to check.
    #[clap(name = "WORKFLOW", default_value = "-", help = "The JSON workflow to evaluate. Use '-' to read from stdin.")]
    workflow: InputFile,
    /// The file containing the config for the reasoner.
    #[clap(short, long, help = "The JSON configuration file to read that configures the policy.")]
    config:   PathBuf,
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
async fn run(args: Arguments) -> miette::Result<()> {
    // Read the workflow & config
    let workflow: Workflow = load_workflow(&args.workflow).await.context("Could not load workflow")?;
    let config: Config = load_config(args.config).await.context("Could not load config")?;

    // Create the logger
    let mut logger =
        SessionedAuditLogger::new("test", FileLogger::new(format!("{} - v{}", env!("CARGO_BIN_NAME"), env!("CARGO_PKG_VERSION")), "./test.log"));

    // Run the reasoner
    let conn = PosixReasonerConnector::new_async(&mut logger).await.into_diagnostic().context("Failed to create the POSIX reasoner")?;

    let verdict = conn.consult(State { workflow, config }, (), &logger).await.into_diagnostic().context("Failed to consult the POSIX reasoner")?;

    // OK, report
    match verdict {
        ReasonerResponse::Success => println!("{} {}", style("Reasoner says:").bold(), style("OK").bold().green()),
        ReasonerResponse::Violated(_) => {
            println!("{} {}", style("Reasoner says:").bold(), style("VIOLATION").bold().red());
        },
    }

    Ok(())
}
