//  EFLINT JSON.rs
//    by Lut99
//
//  Created:
//    10 Oct 2024, 13:54:17
//  Last edited:
//    06 May 2025, 10:57:22
//  Auto updated?
//    Yes
//
//  Description:
//!   Entrypoint to the example `eflint` policy reasoner.
//

use std::fs::{self, File};
use std::io::{self, Read as _};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use console::style;
use eflint_json_reasoner::json::spec::Phrase;
use miette::{Context, IntoDiagnostic as _};
use policy_reasoner::loggers::file::FileLogger;
use policy_reasoner::reasoners::eflint_json::EFlintJsonReasonerConnector;
use policy_reasoner::reasoners::eflint_json::json::spec::RequestPhrases;
use policy_reasoner::reasoners::eflint_json::reasons::EFlintSilentReasonHandler;
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
    /// Whether to read the input as DSL.
    #[clap(
        short,
        long,
        conflicts_with = "json",
        help = "If given, assumes the input is standard eFLINT syntax. This is the default if no language flag is given. Mutually exclusive with \
                '--json'."
    )]
    dsl: bool,
    /// Whether to read the input as JSON.
    #[clap(short, long, conflicts_with = "dsl", help = "If given, assumes the input is eFLINT JSON syntax. Mutually exclusive with '--dsl'.")]
    json: bool,
    /// Which `eflint-to-json` to use.
    #[clap(
        short,
        long,
        help = "If '--json' is given, you can give this to use an existing 'eflint-to-json' binary instead of downloading one from the internet."
    )]
    eflint_path: Option<PathBuf>,

    /// The address where the reasoner lives.
    #[clap(short, long, default_value = "http://127.0.0.1:8080", help = "The address where the eFLINT reasoner lives.")]
    address: String,
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

    // Decide which eflint to run
    let dsl: bool = !args.json;
    let policy: RequestPhrases = if dsl {
        // First: resolve any stdin to a file
        let file: PathBuf = if args.file == "-" {
            let file: PathBuf = std::env::temp_dir().join(format!("{}-v{}-stdin", env!("CARGO_BIN_NAME"), env!("CARGO_PKG_VERSION")));
            let mut handle =
                File::create(&file).into_diagnostic().with_context(|| format!("Failed to open temporary stdin file '{}'", file.display()))?;

            io::copy(&mut io::stdin(), &mut handle)
                .into_diagnostic()
                .with_context(|| format!("Failed to write stdin to temporary file '{}'", file.display()))?;

            file
        } else {
            PathBuf::from(&args.file)
        };

        // Compile first
        let mut json: Vec<u8> = Vec::new();
        eflint_to_json::compile_async(&file, &mut json, args.eflint_path.as_deref())
            .await
            .into_diagnostic()
            .with_context(|| format!("Failed to compile input file '{}' to JSON", args.file))?;

        // Now parse the file contents as a request and done
        serde_json::from_slice(&json).into_diagnostic().with_context(|| {
            format!(
                "Failed to parse {} as an eFLINT JSON phrases request",
                if args.file == "-" { "stdin".to_string() } else { format!("file {:?}", args.file) }
            )
        })?
    } else {
        // Read the file
        let raw: Vec<u8> = if args.file == "-" {
            let mut raw: Vec<u8> = Vec::new();
            io::stdin().read_to_end(&mut raw).into_diagnostic().context("Failed to read stdin")?;
            raw
        } else {
            // Open the file
            fs::read(&args.file).into_diagnostic().with_context(|| format!("Failed to open & read file '{}'", args.file))?
        };

        // Now parse the file contents as a request and done
        serde_json::from_slice(&raw).into_diagnostic().with_context(|| {
            format!(
                "Failed to parse {} as an eFLINT JSON phrases request",
                if args.file == "-" { "stdin".to_string() } else { format!("file {:?}", args.file) }
            )
        })?
    };

    // Create the reasoner
    let conn =
        EFlintJsonReasonerConnector::<EFlintSilentReasonHandler, Vec<Phrase>, ()>::new_async(&args.address, EFlintSilentReasonHandler, &logger)
            .await
            .into_diagnostic()
            .context("Failed to create eFLINT reasoner")?;

    let verdict: ReasonerResponse<NoReason> = conn
        .consult(policy.phrases, (), &logger)
        .await
        .into_diagnostic()
        .with_context(|| format!("Failed to send message to reasoner at {:?}", args.address))?;

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
