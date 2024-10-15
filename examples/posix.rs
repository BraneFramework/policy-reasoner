//  UNIX.rs
//    by Lut99
//
//  Created:
//    11 Oct 2024, 16:32:29
//  Last edited:
//    15 Oct 2024, 17:05:12
//  Auto updated?
//    Yes
//
//  Description:
//!   Showcases the reasoner with a backend that overlays Unix file
//!   persmissions.
//

use std::path::PathBuf;

use clap::Parser;
use console::style;
use error_trace::trace;
use file_logger::FileLogger;
use policy_reasoner::reasoners::posix::{PosixReasonerConnector, State};
use policy_reasoner::spec::ReasonerConnector as _;
use policy_reasoner::spec::auditlogger::SessionedAuditLogger;
use policy_reasoner::workflow::Workflow;
use posix_reasoner::config::Config;
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
///
/// Note that errorring is done by calling [`std::process::exit()`].
async fn load_workflow(input: String) -> Workflow {
    let workflow: String = if input == "-" {
        let mut raw: Vec<u8> = Vec::new();
        if let Err(err) = io::stdin().read_buf(&mut raw).await {
            error!("{}", trace!(("Failed to read from stdin"), err));
            std::process::exit(1);
        }
        match String::from_utf8(raw) {
            Ok(raw) => raw,
            Err(err) => {
                error!("{}", trace!(("Stdin is not valid UTF-8"), err));
                std::process::exit(1);
            },
        }
    } else {
        match fs::read_to_string(&input).await {
            Ok(raw) => raw,
            Err(err) => {
                error!("{}", trace!(("Failed to read the workflow file {input:?}"), err));
                std::process::exit(1);
            },
        }
    };
    match serde_json::from_str(&workflow) {
        Ok(config) => config,
        Err(err) => {
            error!("{}", trace!(("{} is not a valid workflow", if input == "-" { "Stdin".to_string() } else { format!("File {input:?}") }), err));
            std::process::exit(1);
        },
    }
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
///
/// Note that errorring is done by calling [`std::process::exit()`].
async fn load_config(path: PathBuf) -> Config {
    // Load the file and parse it
    let config: String = match fs::read_to_string(&path).await {
        Ok(raw) => raw,
        Err(err) => {
            error!("{}", trace!(("Failed to read the config file {:?}", path.display()), err));
            std::process::exit(1);
        },
    };
    let mut config: Config = match serde_json::from_str(&config) {
        Ok(config) => config,
        Err(err) => {
            error!("{}", trace!(("File {:?} is not a valid config file", path.display()), err));
            std::process::exit(1);
        },
    };

    // Resolve relative files to relative to the binary, for consistency of calling the example
    let prefix: PathBuf = match std::env::current_exe() {
        Ok(path) => {
            if let Some(parent) = path.parent() {
                parent.into()
            } else {
                path
            }
        },
        Err(err) => {
            error!("{}", trace!(("Failed to obtain the current executable's path"), err));
            std::process::exit(1);
        },
    };
    for path in config.data.values_mut().map(|data| &mut data.path) {
        if path.is_relative() {
            *path = prefix.join(&*path);
        }
    }
    debug!("Config after resolving relative paths: {config:?}");

    // Done
    config
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
    workflow: String,
    /// The file containing the config for the reasoner.
    #[clap(short, long, help = "The JSON configuration file to read that configures the policy.")]
    config:   PathBuf,
}





/***** ENTRYPOINT *****/
#[tokio::main(flavor = "current_thread")]
async fn main() {
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

    // Read the workflow & config
    let workflow: Workflow = load_workflow(args.workflow).await;
    let config: Config = load_config(args.config).await;

    // Create the logger
    let logger: SessionedAuditLogger<FileLogger> =
        SessionedAuditLogger::new("test", FileLogger::new(format!("{} - v{}", env!("CARGO_BIN_NAME"), env!("CARGO_PKG_VERSION")), "./test.log"));

    // Run the reasoner
    let conn: PosixReasonerConnector = PosixReasonerConnector::new();
    let verdict: ReasonerResponse<()> = match conn.consult(State { workflow, config }, (), &logger).await {
        Ok(res) => res,
        Err(err) => {
            error!("{}", trace!(("Failed to consult the POSIX reasoner"), err));
            std::process::exit(1);
        },
    };

    // OK, report
    match verdict {
        ReasonerResponse::Success => println!("{} {}", style("Reasoner says:").bold(), style("OK").bold().green()),
        ReasonerResponse::Violated(_) => {
            println!("{} {}", style("Reasoner says:").bold(), style("VIOLATION").bold().red());
        },
    }
}
