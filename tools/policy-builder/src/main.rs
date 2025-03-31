//  MAIN.rs
//    by Lut99
//
//  Created:
//    29 Nov 2023, 15:11:08
//  Last edited:
//    13 Dec 2023, 15:32:42
//  Auto updated?
//    Yes
//
//  Description:
//!   Entrypoint for the `policy-builder` tool.
//

use std::borrow::Cow;
use std::error;
use std::fmt::{Display, Formatter, Result as FResult};
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

use clap::Parser;
use console::Style;
use eflint_to_json::compile;
use error_trace::ErrorTrace as _;
use humanlog::{DebugMode, HumanLogger};
use log::{debug, error, info};

/***** ERRORS *****/
/// Defines errors originating in the binary itself.
#[derive(Debug)]
enum Error {
    /// Failed to create the output file.
    FileCreate { path: PathBuf, err: std::io::Error },
}
impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        use Error::*;
        match self {
            FileCreate { path, .. } => write!(f, "Failed to create output file '{}'", path.display()),
        }
    }
}
impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        use Error::*;
        match self {
            FileCreate { err, .. } => Some(err),
        }
    }
}

/***** ARGUMENTS *****/
/// The arguments for the tool.
#[derive(Debug, Parser)]
struct Arguments {
    /// Whether to do INFO- and DEBUG-level statements.
    #[clap(long, global = true, help = "If given, enables INFO- and DEBUG-level log statements.")]
    debug: bool,
    /// Whether to do TRACE-level statements.
    #[clap(long, global = true, help = "If given, enables TRACE-level log statements. Implies '--debug'.")]
    trace: bool,

    /// The eFLINT file to compile.
    #[clap(name = "PATH", help = "Path pointing to the file to compile.")]
    path:   PathBuf,
    /// The file to compile to.
    #[clap(
        short,
        long,
        help = "If given, writes the result to a file at the given location instead of stdout. Use '-' to explicitly redirect to stdout."
    )]
    output: Option<String>,

    /// Overrides downloading to default location.
    #[clap(
        short,
        long,
        help = "If given, uses an existing 'eflint-to-json' executable. Otherwise, attempts to download from GitHub and writes to \
                `/tmp/eflint-to-json'."
    )]
    compiler: Option<PathBuf>,
}

/***** ENTRYPOINT *****/
fn main() {
    // Parse the arguments
    let args: Arguments = Arguments::parse();

    // Setup the logger
    if let Err(err) = HumanLogger::terminal(DebugMode::from_flags(args.trace, args.debug)).init() {
        eprintln!("WARNING: Failed to setup logger: {err} (no logging for this session)");
    }
    info!("{} - v{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));

    // Resolve the input file
    debug!("Resolving output file...");
    let (output, output_dsc): (Box<dyn Write>, Cow<str>) = if let Some(output) = args.output {
        if output != "-" {
            let output_path: PathBuf = output.into();
            match File::create(&output_path) {
                Ok(handle) => (Box::new(handle), Cow::Owned(output_path.to_string_lossy().into())),
                Err(err) => {
                    error!("{}", Error::FileCreate { path: output_path, err }.trace());
                    std::process::exit(1);
                },
            }
        } else {
            (Box::new(std::io::stdout()), "<stdout>".into())
        }
    } else {
        (Box::new(std::io::stdout()), "<stdout>".into())
    };

    // Run the thing, then
    if let Err(err) = compile(&args.path, output, args.compiler.as_deref()) {
        error!("{}", err.trace());
        std::process::exit(1);
    }

    // Done
    println!(
        "Successfully compiled {} to {}",
        Style::new().bold().green().apply_to(args.path.display()),
        Style::new().bold().green().apply_to(output_dsc),
    );
}
