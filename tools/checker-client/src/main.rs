//  MAIN.rs
//    by Lut99
//
//  Created:
//    15 Dec 2023, 15:08:35
//  Last edited:
//    07 Feb 2024, 11:56:13
//  Auto updated?
//    Yes
//
//  Description:
//!   Entrypoint to the `checker-client` binary.
//

use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};
use std::env;
use std::error::Error;
use std::fmt::{Display, Formatter, Result as FResult};
use std::fs::{self, File};
use std::io::{BufRead as _, BufReader};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::time::{self, Duration, SystemTime};

use audit_logger::LogStatement;
use brane_ast::ast::Edge;
use brane_ast::locations::Locations;
use brane_ast::{CompileResult, ParserOptions, Workflow};
use chrono::DateTime;
use clap::{Parser, Subcommand};
use console::style;
use deliberation::spec::{Verdict, WorkflowValidationRequest};
use eflint_json::DisplayEFlint;
use eflint_to_json::compile;
use enum_debug::EnumDebug;
use error_trace::{ErrorTrace as _, trace};
use hmac::{Hmac, Mac as _};
use humanlog::{DebugMode, HumanLogger};
use jwt::SignWithKey as _;
use log::{LevelFilter, debug, error, info, trace as trace_log, warn};
use policy::Policy;
use rand::Rng as _;
use rand::distributions::Alphanumeric;
use reqwest::blocking::{Client, Request, Response};
use reqwest::{Method, StatusCode};
use serde_json::value::RawValue;
use sha2::Sha256;
use specifications::data::DataIndex;
use specifications::package::PackageIndex;
use srv::models::{AddPolicyPostModel, PolicyContentPostModel, SetVersionPostModel};

/***** CONSTANTS *****/
/// The key to use to create JWTs (for testing purposes only).
const JWT_KEY: &[u8] = b"wL5hkXZpM929BXRCMgVt1GNdM3cSDovRZsU_mPaOPrNJ8x9TvOv9yb3Ps5GkIqdfCyXWM9HEzh0zNDvc_pA_BqAlLiCtlrSajDtCza42HQgWkE71ocWFB5yMkeVcDWaBwUcDm_lPiy-BdfGjmpdox8H7-mOQoieEMNt8hXQR5E7rA3PC9Ih8lma0pFtkRkuCDYyLmBH7geajvkTE77pB5YVUQ57Qm4uijpBus8083tN2UP-oCqBmpAfZ0BtyGY3oFlRk3sf_HwhSz2gFalYUuK8379hY4BOzuM80pIL18VHVzFgOwRI48RBCk21M5aoFiLMc5Gp9VTKKd9VxQNgExA";

/// The checker path to the policy API's policy list request path.
const POLICY_ADD_POLICY_PATH: (Method, &str) = (Method::POST, "v1/management/policies");
/// The checker path to the policy API's set-active-policy request path.
const POLICY_SET_ACTIVE_POLICY_PATH: (Method, &str) = (Method::PUT, "v1/management/policies/active");
/// The checker path to the policy API's get-active-policy request path.
const POLICY_GET_ACTIVE_POLICY_PATH: (Method, &str) = (Method::GET, "v1/management/policies/active");
/// The checker path to the deliberation API's workflow check request path.
const DELIB_WORKFLOW_VALIDATION_PATH: (Method, &str) = (Method::POST, "v1/deliberation/execute-workflow");

/***** ERRORS *****/
/// Defines errors that originate from parsing [`PolicyLanguage`]s.
#[derive(Debug)]
enum PolicyLanguageParseError {
    /// It's an unknown language.
    Unknown { raw: String },
}
impl Display for PolicyLanguageParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        use PolicyLanguageParseError::*;
        match self {
            Unknown { raw } => write!(f, "Failed to parse '{raw}' as a policy language (expected 'eflint', 'eflint_json' or 'eflint-json'"),
        }
    }
}
impl Error for PolicyLanguageParseError {}

/// Defines errors that originate from parsing [`PolicyLanguage`]s.
#[derive(Debug)]
enum WorkflowLanguageParseError {
    /// It's an unknown language.
    Unknown { raw: String },
}
impl Display for WorkflowLanguageParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        use WorkflowLanguageParseError::*;
        match self {
            Unknown { raw } => {
                write!(f, "Failed to parse '{raw}' as a workflow language (expected 'bs', 'bscript', 'branescript', 'wir' or 'checker'")
            },
        }
    }
}
impl Error for WorkflowLanguageParseError {}

/// Defines errors that originate from creating JSON Web Tokens.
#[derive(Debug)]
enum JwtError {
    /// Failed to create/sign a token
    Create { err: jwt::Error },
}
impl Display for JwtError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        use JwtError::*;
        match self {
            Create { .. } => write!(f, "Failed to create new JWT"),
        }
    }
}
impl Error for JwtError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        use JwtError::*;
        match self {
            Create { err } => Some(err),
        }
    }
}

/***** HELPERS *****/
/// Defines accepted policy input languages.
#[derive(Clone, Copy, Debug, EnumDebug, Eq, Hash, PartialEq)]
enum PolicyLanguage {
    /// It's normal eFLINT syntax.
    EFlint,
    /// It's eFLINT JSON syntax.
    EFlintJson,
}
impl FromStr for PolicyLanguage {
    type Err = PolicyLanguageParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "eflint" => Ok(Self::EFlint),
            "eflint_json" | "eflint-json" => Ok(Self::EFlintJson),
            raw => Err(PolicyLanguageParseError::Unknown { raw: raw.into() }),
        }
    }
}

/// Defines accepted workflow input languages.
#[derive(Clone, Copy, Debug, EnumDebug, Eq, Hash, PartialEq)]
enum WorkflowLanguage {
    /// It's BraneScript.
    BraneScript,
    /// It's the WIR.
    Wir,
}
impl FromStr for WorkflowLanguage {
    type Err = WorkflowLanguageParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "bs" | "bscript" | "branescript" => Ok(Self::BraneScript),
            "wir" => Ok(Self::Wir),
            raw => Err(WorkflowLanguageParseError::Unknown { raw: raw.into() }),
        }
    }
}

/***** ARGUMENTS *****/
/// Defines the arguments of the `checker-client` binary.
#[derive(Debug, Parser)]
struct Arguments {
    /// Whether to enable debug logs
    #[clap(long, global = true, help = "If given, enabled additional log statements (DEBUG, INFO) and adds more information per statement.")]
    debug: bool,
    /// Whether to enable debug + trace logs
    #[clap(
        long,
        global = true,
        help = "If given, enabled additional log statements (TRACE, DEBUG, INFO) and adds maximum information per statement (implies '--debug')."
    )]
    trace: bool,

    /// The address of the checker to connect to.
    #[clap(short, long, global = true, default_value = "localhost", help = "The address of the checker we're connecting to.")]
    address: String,
    /// The port of the checker to connect to.
    #[clap(short, long, global = true, default_value = "3030", help = "The port of the checker we're connecting to.")]
    port:    u16,
    /// The name of the person submitting policies.
    #[clap(short, long, global = true, help = "The name under which to submit policies. Chooses a random name if omitted.")]
    name:    Option<String>,
    /// A JWT that authenticates the user.
    #[clap(short, long, global = true, help = "A JWT that is used to authenticate with the checker. Ignores '--name' if given.")]
    jwt:     Option<String>,

    /// The toplevel subcommand that decides what to do
    #[clap(subcommand)]
    subcommand: Subcommands,
}

/// Defines the toplevel subcommands for the `checker-client` binary.
#[derive(Debug, Subcommand)]
enum Subcommands {
    /// Policy-related stuff
    #[clap(name = "policy", about = "Groups commands relating to policy management.")]
    Policy(PolicyArguments),
    /// Deliberation-related stuff
    #[clap(name = "check", about = "Groups commands relating to deliberating the checker.")]
    Check(CheckArguments),
    /// Audit log-related stuff
    #[clap(name = "log", about = "Groups commands for better understanding audit logs.")]
    Log(LogArguments),
}

/// Defines arguments for the `checker-client policy` subcommand.
#[derive(Debug, Parser)]
struct PolicyArguments {
    /// Subcommand further
    #[clap(subcommand)]
    action: PolicySubcommands,
}

/// Defines nested subcommands for the `checker-client policy` subcommand.
#[derive(Debug, Subcommand)]
enum PolicySubcommands {
    /// Pushes a new policy to the checker.
    #[clap(name = "push", about = "Pushes a new policy to the checker.")]
    Push(PolicyPushArguments),
    /// Returns the currently active policy as active in the checker.
    #[clap(name = "get", about = "Retrieves the currently active policy in the checker.")]
    Get(PolicyGetArguments),
    /// Sets a policy  as active in the checker.
    #[clap(name = "set", about = "Makes a policy with the given version ID active in the checker.")]
    Set(PolicySetArguments),
}

/// Defines arguments for the `checker-client policy push` subcommand.
#[derive(Debug, Parser)]
struct PolicyPushArguments {
    /// The path to the policy file to push.
    #[clap(name = "POLICY", help = "The path of the policy file to push.")]
    path: PathBuf,

    /// Determines the input language of the policy file.
    #[clap(
        short,
        long,
        default_value = "eflint",
        help = "The language of the input file. Can be 'eflint' for eFLINT; or 'eflint_json' or 'eflint-json' for eFLINT JSON."
    )]
    language: PolicyLanguage,
    /// Whether we're using an external `eflint-to-json` executable or not.
    #[clap(short, long, help = "If given, does not download the Linux x86-64 'eflint-to-json' executable but instead uses the provided one.")]
    eflint_to_json_path: Option<PathBuf>,
}

/// Defines arguments for the `checker-client policy get` subcommand.
#[derive(Debug, Parser)]
struct PolicyGetArguments {
    /// If given, attempts to parse the returned set of policy as eFLINT JSON and shows it as such.
    #[clap(short, long, help = "If given, attempts to parse the returned set of policy as eFLINT JSON and shows it as such.")]
    eflint: bool,
}

/// Defines arguments for the `checker-client policy set` subcommand.
#[derive(Debug, Parser)]
struct PolicySetArguments {
    /// The ID of the policy to set.
    #[clap(name = "VERSION", help = "The ID of the policy to set.")]
    version: i64,
}

/// Defines arguments for the `checker-client check` subcommand.
#[derive(Debug, Parser)]
struct CheckArguments {
    /// Subcommand further
    #[clap(subcommand)]
    action: CheckSubcommands,

    /// A use-case to perform the command under.
    #[clap(short, long, default_value = "default", global = true, help = "Determines the use-case as which to report to the checker.")]
    use_case:     String,
    /// A user to designate as receiver of results.
    #[clap(short, long, global = true, help = "Determines who will be reported as receiving the final result of the submitted workflow.")]
    result_owner: Option<String>,
}

/// Defines nested subcommands for the `checker-client check` subcommand.
#[derive(Debug, Subcommand)]
enum CheckSubcommands {
    /// Sends a workflow to the checker for validation.
    #[clap(name = "workflow", alias = "wf", about = "Asks the checker to validate an entire workflow.")]
    Workflow(CheckWorkflowArguments),
}

/// Defines arguments for the `checker-client check workflow` subcommand.
#[derive(Debug, Parser)]
struct CheckWorkflowArguments {
    /// The path to the workflow file to check.
    #[clap(name = "WORKFLOW", help = "The path of the workflow file to check.")]
    path: PathBuf,

    /// Determines the input language of the policy file.
    #[clap(
        short,
        long,
        default_value = "branescript",
        help = "The language of the input file. Can be 'bs', 'bscript' or 'branescript' for BraneScript; or 'wir' for the Brane WIR."
    )]
    language: WorkflowLanguage,
    /// Determines the package index location.
    #[clap(short='P', long, default_value = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/packages"), help = "The location where the package index is read from. Note that this is read in test mode (i.e., `brane`'s default package index does not work)")]
    packages: PathBuf,
    /// Determines the data index location.
    #[clap(short='D', long, default_value = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/data"), help = "The location where the data index is read from. Note that this is read in test mode (i.e., `brane`'s default data index does not work)")]
    data:     PathBuf,
}

/// Defines arguments for the `checker-client log` subcommand.
#[derive(Debug, Parser)]
struct LogArguments {
    /// The audit log used
    #[clap(short, long, global = true, default_value = "./audit-log.log", help = "The path to the audit log to read.")]
    log: PathBuf,

    /// Subcommand further
    #[clap(subcommand)]
    action: LogSubcommands,
}

/// Defines nested subcommands for the `checker-client log` subcommand.
#[derive(Debug, Subcommand)]
enum LogSubcommands {
    /// Attempts to find the reasons why a policy was denied
    #[clap(name = "reason", about = "Reads the audit log to find reasons why the request with given reference ID is denied.")]
    Reason(LogReasonArguments),
}

/// Defines the arguments for the `checker-client log reason` subcommand.
#[derive(Debug, Parser)]
struct LogReasonArguments {
    /// The reference ID to search for.
    #[clap(name = "REFERENCE_ID", help = "The reference ID provided by the checker to find why the request failed.")]
    reference_id: String,
}

/***** HELPER FUNCTIONS *****/
/// Given a potentially given JWT, uses it or generates a new one.
///
/// # Arguments
/// - `name`: The name to embed in the JWT if we're generating one.
/// - `jwt`: The JWT given by the user, or [`None`] if they didn't.
///
/// # Returns
/// A new, already serialized (and encoded!) JSON web token.
fn resolve_jwt(name: impl Into<String>, jwt: Option<String>) -> Result<String, JwtError> {
    match jwt {
        Some(jwt) => {
            debug!("Using given JWT '{jwt}'");
            Ok(jwt)
        },
        None => {
            // Create a key from the internal one
            let key: Hmac<Sha256> = match Hmac::new_from_slice(JWT_KEY) {
                Ok(key) => key,
                Err(err) => {
                    error!("{}", trace!(("Failed to create HMAC key from private key"), err));
                    std::process::exit(1);
                },
            };

            // Generate the claims
            let mut claims: BTreeMap<&str, String> = BTreeMap::new();
            claims.insert("sub", "1234567890".into());
            claims.insert("username", name.into());
            claims.insert("iat", SystemTime::now().duration_since(time::UNIX_EPOCH).unwrap().as_secs().to_string());
            claims
                .insert("exp", (SystemTime::now() + Duration::from_secs(24 * 3600)).duration_since(time::UNIX_EPOCH).unwrap().as_secs().to_string());

            // Create a JWT with it
            match claims.sign_with_key(&key) {
                Ok(jwt) => {
                    debug!("Using generated JWT '{jwt}'");
                    Ok(jwt)
                },
                Err(err) => Err(JwtError::Create { err }),
            }
        },
    }
}

/// "Trivially" plans a workflow.
///
/// Means: will plan if possible and properly scoped by user, or else give up.
///
/// # Arguments
/// - `edges`: The list of edges to plan.
/// - `pc`: The current program counter that points to the edge we're currently planning.
/// - `breakpoint`: If [`Some`], then this points to an edge we should stop and return at.
fn plan_wir(edges: &mut [Edge], pc: (usize, usize), breakpoint: Option<(usize, usize)>) {
    // Break at the breakpoint
    if let Some(breakpoint) = breakpoint {
        if pc == breakpoint {
            return;
        }
    }

    // Get the edge
    let edge: &mut Edge = match edges.get_mut(pc.1) {
        Some(edge) => edge,
        None => return,
    };

    // Match on it
    use Edge::*;
    #[allow(clippy::needless_return)]
    match edge {
        Node { task: _, locs, at, input: _, result: _, metadata: _, next } => {
            let next: usize = *next;

            // If there is a single location possible, log it
            if let Locations::Restricted(list) = locs {
                if list.len() == 1 {
                    *at = Some(list.first().cloned().unwrap());
                } else {
                    warn!("Cannot plan edge ({},{}) because it does not have exactly one possible location (instead: {:?})", pc.0, pc.1, list);
                }
            } else {
                warn!("Cannot plan edge ({},{}) because its possible locations are not restricted", pc.0, pc.1);
            }

            // Continue
            plan_wir(edges, (pc.0, next), breakpoint)
        },
        Linear { instrs: _, next } => {
            let next: usize = *next;
            plan_wir(edges, (pc.0, next), breakpoint);
        },
        Stop {} => return,

        Branch { true_next, false_next, merge } => {
            let (true_next, false_next, merge): (usize, Option<usize>, Option<usize>) = (*true_next, *false_next, *merge);

            plan_wir(edges, (pc.0, true_next), merge.map(|m| (pc.0, m)));
            if let Some(false_next) = false_next {
                plan_wir(edges, (pc.0, false_next), merge.map(|m| (pc.0, m)));
            }
            if let Some(merge) = merge {
                plan_wir(edges, (pc.0, merge), breakpoint);
            }
        },
        Parallel { branches, merge } => {
            let (branches, merge): (Vec<usize>, usize) = (branches.clone(), *merge);

            for branch in branches {
                plan_wir(edges, (pc.0, branch), Some((pc.0, merge)));
            }
            plan_wir(edges, (pc.0, merge), breakpoint);
        },
        Join { merge: _, next } => {
            let next: usize = *next;
            plan_wir(edges, (pc.0, next), breakpoint);
        },
        Loop { cond, body, next } => {
            let (cond, body, next): (usize, usize, Option<usize>) = (*cond, *body, *next);

            plan_wir(edges, (pc.0, cond), Some((pc.0, body - 1)));
            plan_wir(edges, (pc.0, body), Some((pc.0, cond)));
            if let Some(next) = next {
                plan_wir(edges, (pc.0, next), breakpoint);
            }
        },

        Call { input: _, result: _, next } => {
            let next: usize = *next;
            plan_wir(edges, (pc.0, next), breakpoint);
        },
        Return { result: _ } => return,
    };
}

/// Analyses a line to see if it's the start of a logging line.
///
/// Specifically, checks if it starts with `[policy-reasoner <any>][<date_time>] `.
///
/// # Arguments
/// - `line`: The line to check.
///
/// # Returns
/// The position from where the real line begins, or else [`None`] if this wasn't the start of a log line.
fn line_is_log_line(line: &str) -> Option<usize> {
    let line: &str = line.trim();

    // Find twice the `]`
    let brack_pos: usize = match line.find(']') {
        Some(pos) => pos,
        None => {
            trace_log!("Line '{line}' is not a log line because it does not have a ']'");
            return None;
        },
    };
    let first: &str = &line[..brack_pos];
    let rem: &str = &line[brack_pos + 1..];
    let brack_pos2: usize = match rem.find(']') {
        Some(pos) => pos,
        None => {
            trace_log!("Line '{line}' is not a log line because it does not have a second ']'");
            return None;
        },
    };
    let second: &str = &rem[..brack_pos2];
    let rem: &str = &rem[brack_pos2 + 1..];

    /* FIRST PART */
    // Assert it begins with '[policy-reasoner v'
    if first.len() < 18 || &first[..18] != "[policy-reasoner v" {
        trace_log!("Line '{line}' is not a log line because the first part ('{first}')  does not begin with '[policy-reasoner v'");
        return None;
    }

    /* SECOND PART */
    // Assert it begins with '['
    if !matches!(second.chars().next(), Some('[')) {
        trace_log!("Line '{line}' is not a log line because the second part ('{second}') does not begin with '['");
        return None;
    }
    let second: &str = &second[1..];

    // Attempt to parse the middle part as a datetime
    if let Err(err) = DateTime::parse_from_str(&format!("{second}.000 +0000"), "%Y-%m-%d %H:%M:%S%.3f %z") {
        trace_log!("Line '{line}' is not a log line because the second part ('{second}.000 %z') does not parse as a datetime: {err}");
        return None;
    }

    /* REMAINDER */
    // Now all that remains is to check for the final space
    rem.chars().next().filter(|c| *c == ' ').map(|_| brack_pos + 1 + brack_pos2 + 2)
}

/***** ENTRYPOINT *****/
fn main() {
    // Parse the args
    let args = Arguments::parse();

    // Setup the logger
    if let Err(err) = HumanLogger::terminal(DebugMode::from_flags(args.trace, args.debug)).init() {
        eprintln!("WARNING: Failed to setup logger: {err} (logging disabled for this session)");
    }
    info!("{} v{}", env!("CARGO_BIN_NAME"), env!("CARGO_PKG_VERSION"));

    // Resolve the name
    let name: Cow<str> = match args.name {
        Some(name) => Cow::Owned(name),
        None => Cow::Borrowed(names::three::usualcase::rand()),
    };
    debug!("Working as '{name}'");

    // Match on the given subcommand
    match args.subcommand {
        Subcommands::Policy(policy) => match policy.action {
            PolicySubcommands::Push(push) => {
                info!("Handling `policy push` subcommand");

                // Resolve the JWT
                let jwt: String = match resolve_jwt(name, args.jwt) {
                    Ok(jwt) => jwt,
                    Err(err) => {
                        error!("{}", err.trace());
                        std::process::exit(1);
                    },
                };

                // Match on the input language
                let json_path: Cow<Path> = match push.language {
                    PolicyLanguage::EFlint => {
                        let json_path: PathBuf = env::temp_dir().join(format!(
                            "policy-{}.json",
                            rand::thread_rng().sample_iter(Alphanumeric).take(8).map(char::from).collect::<String>()
                        ));
                        debug!("Compiling input file '{}' to eFLINT JSON file '{}'...", push.path.display(), json_path.display());

                        // Open the output file
                        debug!("Creating output file '{}'...", json_path.display());
                        let handle: File = match File::create(&json_path) {
                            Ok(handle) => handle,
                            Err(err) => {
                                error!("{}", trace!(("Failed to create output file '{}'", json_path.display()), err));
                                std::process::exit(1);
                            },
                        };

                        // Run the compiler
                        debug!("Running eflint-to-json compiler on '{}'...", push.path.display());
                        if let Err(err) = compile(&push.path, handle, push.eflint_to_json_path.as_deref()) {
                            error!("{}", trace!(("Failed to compile input file '{}'", push.path.display()), err));
                            std::process::exit(1);
                        };
                        Cow::Owned(json_path)
                    },
                    PolicyLanguage::EFlintJson => Cow::Borrowed(&push.path),
                };

                // Open that file to send it
                debug!("Opening policy file '{}'...", json_path.display());
                let body: Vec<u8> = {
                    // // Open the file
                    // let handle: File = match File::open(&json_path) {
                    //     Ok(handle) => handle,
                    //     Err(err) => {
                    //         error!("{}", Error::PolicyOpen { path: push.path, err }.trace());
                    //         std::process::exit(1);
                    //     },
                    // };

                    // // Read its metadata
                    // let metadata: Metadata = match handle.metadata() {
                    //     Ok(metadata) => metadata,
                    //     Err(err) => {
                    //         error!("{}", Error::PolicyMetadata { path: push.path, err }.trace());
                    //         std::process::exit(1);
                    //     },
                    // };

                    // // Put it in a request body
                    // (Body::new(handle), metadata.len())

                    // First, read the file in its entirety
                    let policy: String = match fs::read_to_string(&json_path) {
                        Ok(policy) => policy,
                        Err(err) => {
                            error!("{}", trace!(("Failed to read eFLINT JSON file '{}'", json_path.display()), err));
                            std::process::exit(1);
                        },
                    };
                    // Deserialize it to a raw JSON value
                    let policy: Box<RawValue> = match serde_json::from_str(&policy) {
                        Ok(policy) => policy,
                        Err(err) => {
                            error!("{}", trace!(("Failed to parse eFLINT JSON file as JSON '{}'", json_path.display()), err));
                            std::process::exit(1);
                        },
                    };

                    // Wrap it in the request
                    let request: AddPolicyPostModel = AddPolicyPostModel {
                        description: None,
                        version_description: "A test version of policy uploaded using the checker-client tool".into(),
                        content: vec![PolicyContentPostModel { reasoner: "eflint".into(), reasoner_version: "0.1.0".into(), content: policy }],
                    };
                    // Re-serialize
                    match serde_json::to_string(&request) {
                        Ok(req) => req.into_bytes(),
                        Err(err) => {
                            error!("{}", trace!(("Failed to serialize checker add-policy request to JSON"), err));
                            std::process::exit(1);
                        },
                    }
                };

                // Build a request to the checker
                let addr: String = format!("http://{}:{}/{}", args.address, args.port, POLICY_ADD_POLICY_PATH.1);
                debug!("Building request to checker '{addr}'...");
                let client: Client = Client::new();
                let req: Request = match client
                    .request(POLICY_ADD_POLICY_PATH.0, &addr)
                    .header(reqwest::header::AUTHORIZATION, format!("Bearer {jwt}"))
                    .header(reqwest::header::CONTENT_LENGTH, body.len())
                    .body(body)
                    .build()
                {
                    Ok(req) => req,
                    Err(err) => {
                        error!("{}", trace!(("Failed to build request to '{}:{}'", args.address, args.port), err));
                        std::process::exit(1);
                    },
                };

                // Send it
                debug!("Sending request to checker '{addr}'...");
                let res: Response = match client.execute(req) {
                    Ok(res) => res,
                    Err(err) => {
                        error!("{}", trace!(("Failed to execute request to '{}:{}'", args.address, args.port), err));
                        std::process::exit(1);
                    },
                };
                let status: StatusCode = res.status();
                if !status.is_success() {
                    error!(
                        "Request to '{}' failed with {} ({}){}",
                        addr,
                        status.as_u16(),
                        status.canonical_reason().unwrap_or("???"),
                        if let Ok(err) = res.text() {
                            format!(
                                "\n\nResponse:\n{}\n{}\n{}\n",
                                (0..80).map(|_| '-').collect::<String>(),
                                err,
                                (0..80).map(|_| '-').collect::<String>()
                            )
                        } else {
                            String::new()
                        }
                    );
                    std::process::exit(1);
                }

                // Show the response to the user
                println!("{}", style("Checker replied with:").bold());
                println!("{}", res.text().unwrap_or("<failed to get response body>".into()));
                println!();
            },

            PolicySubcommands::Get(get) => {
                info!("Handling `policy get` subcommand");

                // Resolve the JWT
                let jwt: String = match resolve_jwt(name, args.jwt) {
                    Ok(jwt) => jwt,
                    Err(err) => {
                        error!("{}", err.trace());
                        std::process::exit(1);
                    },
                };

                // Build a request to the checker
                let addr: String = format!("http://{}:{}/{}", args.address, args.port, POLICY_GET_ACTIVE_POLICY_PATH.1);
                debug!("Building request to checker '{addr}'...");
                let client: Client = Client::new();
                let req: Request = match client
                    .request(POLICY_GET_ACTIVE_POLICY_PATH.0, &addr)
                    .header(reqwest::header::AUTHORIZATION, format!("Bearer {jwt}"))
                    .build()
                {
                    Ok(req) => req,
                    Err(err) => {
                        error!("{}", trace!(("Failed to build request to '{}:{}'", args.address, args.port), err));
                        std::process::exit(1);
                    },
                };

                // Send it
                debug!("Sending request to checker '{addr}'...");
                let res: Response = match client.execute(req) {
                    Ok(res) => res,
                    Err(err) => {
                        error!("{}", trace!(("Failed to execute request to '{}:{}'", args.address, args.port), err));
                        std::process::exit(1);
                    },
                };
                let status: StatusCode = res.status();
                if !status.is_success() {
                    error!(
                        "Request to '{}' failed with {} ({}){}",
                        addr,
                        status.as_u16(),
                        status.canonical_reason().unwrap_or("???"),
                        if let Ok(err) = res.text() {
                            format!(
                                "\n\nResponse:\n{}\n{}\n{}\n",
                                (0..80).map(|_| '-').collect::<String>(),
                                err,
                                (0..80).map(|_| '-').collect::<String>()
                            )
                        } else {
                            String::new()
                        }
                    );
                    std::process::exit(1);
                }

                // EITHER: Show the raw response or the parsed one
                let text: Result<String, reqwest::Error> = res.text();
                if get.eflint {
                    // Parse the incoming request
                    debug!("Parsing checker response...");
                    let policy: Policy = match text {
                        Ok(response) => match serde_json::from_str(&response) {
                            Ok(policy) => policy,
                            Err(err) => {
                                error!(
                                    "Failed to parse response text as Policy: {}\n\nResponse:\n{}\n{}\n{}\n",
                                    err,
                                    (0..80).map(|_| '-').collect::<String>(),
                                    response,
                                    (0..80).map(|_| '-').collect::<String>()
                                );
                                std::process::exit(1);
                            },
                        },
                        Err(err) => {
                            error!("{}", trace!(("Failed to get response"), err));
                            std::process::exit(1);
                        },
                    };

                    // Next, parse the policies
                    for (i, policy) in policy.content.into_iter().enumerate() {
                        // Attempt to parse the embedded eFLINT
                        debug!("Deserializing policy {i}...");
                        let policy: eflint_json::spec::Request = match serde_json::from_str(policy.content.get()) {
                            Ok(policy) => policy,
                            Err(err) => {
                                error!("{}", trace!(("Failed to parse policy {i} in request as valid eFLINT JSON"), err));
                                std::process::exit(1);
                            },
                        };

                        // Show it to the user
                        println!("{}", style(format!("Active checker policy {i}")).bold());
                        println!("{:#}", policy.display_syntax());
                        println!();
                    }
                } else {
                    println!("{}", style("Checker replied with:").bold());
                    println!("{}", text.unwrap_or("<failed to get response body>".into()));
                    println!();
                }
            },

            PolicySubcommands::Set(set) => {
                info!("Handling `policy set` subcommand");

                // Resolve the JWT
                let jwt: String = match resolve_jwt(name, args.jwt) {
                    Ok(jwt) => jwt,
                    Err(err) => {
                        error!("{}", err.trace());
                        std::process::exit(1);
                    },
                };

                // Create the request body to send
                debug!("Generating policy request...");
                let body: SetVersionPostModel = SetVersionPostModel { version: set.version };
                let body: Vec<u8> = match serde_json::to_string(&body) {
                    Ok(body) => body.into_bytes(),
                    Err(err) => {
                        error!("{}", trace!(("Failed to serialize checker set-policy request to JSON"), err));
                        std::process::exit(1);
                    },
                };

                // Build a request to the checker
                let addr: String = format!("http://{}:{}/{}", args.address, args.port, POLICY_SET_ACTIVE_POLICY_PATH.1);
                debug!("Building request to checker '{addr}'...");
                let client: Client = Client::new();
                let req: Request = match client
                    .request(POLICY_SET_ACTIVE_POLICY_PATH.0, &addr)
                    .header(reqwest::header::AUTHORIZATION, format!("Bearer {jwt}"))
                    .header(reqwest::header::CONTENT_LENGTH, body.len())
                    .body(body)
                    .build()
                {
                    Ok(req) => req,
                    Err(err) => {
                        error!("{}", trace!(("Failed to build request to '{}:{}'", args.address, args.port), err));
                        std::process::exit(1);
                    },
                };

                // Send it
                debug!("Sending request to checker '{addr}'...");
                let res: Response = match client.execute(req) {
                    Ok(res) => res,
                    Err(err) => {
                        error!("{}", trace!(("Failed to execute request to '{}:{}'", args.address, args.port), err));
                        std::process::exit(1);
                    },
                };
                let status: StatusCode = res.status();
                if !status.is_success() {
                    error!(
                        "Request to '{}' failed with {} ({}){}",
                        addr,
                        status.as_u16(),
                        status.canonical_reason().unwrap_or("???"),
                        if let Ok(err) = res.text() {
                            format!(
                                "\n\nResponse:\n{}\n{}\n{}\n",
                                (0..80).map(|_| '-').collect::<String>(),
                                err,
                                (0..80).map(|_| '-').collect::<String>()
                            )
                        } else {
                            String::new()
                        }
                    );
                    std::process::exit(1);
                }

                // Show the response to the user
                println!("{}", style("Checker replied with:").bold());
                println!("{}", res.text().unwrap_or("<failed to get response body>".into()));
                println!();
            },
        },

        Subcommands::Check(check) => match check.action {
            CheckSubcommands::Workflow(wf) => {
                info!("Handling `check workflow` subcommand");

                // Resolve the JWT
                let jwt: String = match resolve_jwt(name, args.jwt) {
                    Ok(jwt) => jwt,
                    Err(err) => {
                        error!("{}", err.trace());
                        std::process::exit(1);
                    },
                };

                // Read the data index (we'll need it for planning)
                let dindex: DataIndex = brane_shr::utilities::create_data_index_from(&wf.data);

                // Match on the input language
                let mut wir: Workflow = match wf.language {
                    WorkflowLanguage::BraneScript => {
                        debug!("Compiling input file '{}' to a Brane WIR...", wf.path.display());

                        // Open the input file
                        debug!("Reading input file '{}' as BraneScript", wf.path.display());
                        let input: String = match fs::read_to_string(&wf.path) {
                            Ok(input) => input,
                            Err(err) => {
                                error!("{}", trace!(("Failed to read input file '{}'", wf.path.display()), err));
                                std::process::exit(1);
                            },
                        };

                        // Open the package index, too
                        let pindex: PackageIndex = brane_shr::utilities::create_package_index_from(&wf.packages);

                        // Run the compiler
                        debug!("Running eflint-to-json compiler on '{}'...", wf.path.display());
                        match brane_ast::compile_program(input.as_bytes(), &pindex, &dindex, &ParserOptions::bscript()) {
                            CompileResult::Workflow(workflow, warns) => {
                                // Print warnings (if any)
                                let spath: String = wf.path.display().to_string();
                                for warn in warns {
                                    warn.prettyprint(&spath, &input)
                                }
                                workflow
                            },
                            CompileResult::Err(errs) => {
                                // Print the errors that occurred
                                let spath: String = wf.path.display().to_string();
                                for err in errs {
                                    err.prettyprint(&spath, &input);
                                }
                                error!("Failed to compile input file '{spath}' (see output above)");
                                std::process::exit(1);
                            },
                            CompileResult::Eof(err) => {
                                // Print the errors that occurred
                                let spath: String = wf.path.display().to_string();
                                err.prettyprint(&spath, input);
                                error!("Failed to compile input file '{spath}' (see output above)");
                                std::process::exit(1);
                            },

                            CompileResult::Program(_, _) | CompileResult::Unresolved(_, _) => unreachable!(),
                        }
                    },
                    WorkflowLanguage::Wir => {
                        // Open the input file
                        debug!("Reading input file '{}' as Brane WIR...", wf.path.display());
                        let input: String = match fs::read_to_string(&wf.path) {
                            Ok(input) => input,
                            Err(err) => {
                                error!("{}", trace!(("Failed to read input file '{}'", wf.path.display()), err));
                                std::process::exit(1);
                            },
                        };

                        // Deserialize
                        match serde_json::from_str(&input) {
                            Ok(wir) => wir,
                            Err(err) => {
                                error!("{}", trace!(("Failed to parse input file '{}' as Brane WIR", wf.path.display()), err));
                                std::process::exit(1);
                            },
                        }
                    },
                };

                // Trivially plan the workflow
                {
                    // Plan the main workflow
                    let mut graph: Arc<Vec<Edge>> = Arc::new(vec![]);
                    std::mem::swap(&mut graph, &mut wir.graph);
                    let mut graph: Vec<Edge> = Arc::into_inner(graph).unwrap();
                    plan_wir(&mut graph, (usize::MAX, 0), None);
                    let mut graph: Arc<Vec<Edge>> = Arc::new(graph);
                    std::mem::swap(&mut wir.graph, &mut graph);

                    // Plan the functions in the workflow
                    let mut funcs: Arc<HashMap<usize, Vec<Edge>>> = Arc::new(HashMap::new());
                    std::mem::swap(&mut funcs, &mut wir.funcs);
                    let mut funcs: HashMap<usize, Vec<Edge>> = Arc::into_inner(funcs).unwrap();
                    for edges in funcs.values_mut() {
                        plan_wir(edges, (usize::MAX, 0), None);
                    }
                    let mut funcs: Arc<HashMap<usize, Vec<Edge>>> = Arc::new(funcs);
                    std::mem::swap(&mut wir.funcs, &mut funcs);
                }
                if log::max_level() >= LevelFilter::Debug {
                    let mut buf: Vec<u8> = Vec::new();
                    brane_ast::traversals::print::ast::do_traversal(&wir, &mut buf).unwrap();
                    debug!("Workflow after planning:\n\n{}\n", String::from_utf8_lossy(&buf));
                }

                // Also add a user
                wir.user = Arc::new(Some(check.result_owner.unwrap_or_else(|| names::three::usualcase::rand().into())));

                // Now put the workflow in a request and serialize it
                let body: Vec<u8> = match serde_json::to_string(&WorkflowValidationRequest { use_case: check.use_case, workflow: wir }) {
                    Ok(body) => body.into_bytes(),
                    Err(err) => {
                        error!("{}", trace!(("Failed to serialize given Brane WIR in a WorkflowValidationRequest to JSON"), err));
                        std::process::exit(1);
                    },
                };

                // Build a request to the checker
                let addr: String = format!("http://{}:{}/{}", args.address, args.port, DELIB_WORKFLOW_VALIDATION_PATH.1);
                debug!("Building request to checker '{addr}'...");
                let client: Client = Client::new();
                let req: Request = match client
                    .request(DELIB_WORKFLOW_VALIDATION_PATH.0, &addr)
                    .header(reqwest::header::AUTHORIZATION, format!("Bearer {jwt}"))
                    .header(reqwest::header::CONTENT_LENGTH, body.len())
                    .body(body)
                    .build()
                {
                    Ok(req) => req,
                    Err(err) => {
                        error!("{}", trace!(("Failed to build request to '{}:{}'", args.address, args.port), err));
                        std::process::exit(1);
                    },
                };

                // Send it
                debug!("Sending request to checker '{addr}'...");
                let res: Response = match client.execute(req) {
                    Ok(res) => res,
                    Err(err) => {
                        error!("{}", trace!(("Failed to execute request to '{}:{}'", args.address, args.port), err));
                        std::process::exit(1);
                    },
                };
                let status: StatusCode = res.status();
                if !status.is_success() {
                    error!(
                        "Request to '{}' failed with {} ({}){}",
                        addr,
                        status.as_u16(),
                        status.canonical_reason().unwrap_or("???"),
                        if let Ok(err) = res.text() {
                            format!(
                                "\n\nResponse:\n{}\n{}\n{}\n",
                                (0..80).map(|_| '-').collect::<String>(),
                                err,
                                (0..80).map(|_| '-').collect::<String>()
                            )
                        } else {
                            String::new()
                        }
                    );
                    std::process::exit(1);
                }

                // Show the response to the user
                println!("{}", style("Checker replied with:").bold());
                println!("{}", res.text().unwrap_or("<failed to get response body>".into()));
                println!();
            },
        },

        Subcommands::Log(log) => {
            // Open the log file
            debug!("Opening log file '{}'...", log.log.display());
            let handle: BufReader<File> = match File::open(&log.log) {
                Ok(handle) => BufReader::new(handle),
                Err(err) => {
                    error!("{}", trace!(("Failed to open log file '{}'", log.log.display()), err));
                    std::process::exit(1);
                },
            };

            // Separate the log into statements
            debug!("Finding log statements...");
            let mut buf: String = String::new();
            let mut statements: Vec<LogStatement> = Vec::new();
            for line in handle.lines() {
                // Unwrap the line
                let line: String = match line {
                    Ok(line) => line,
                    Err(err) => {
                        error!("{}", trace!(("Failed to read line from '{}'", log.log.display()), err));
                        std::process::exit(1);
                    },
                };

                // See if the line begins with what we want
                if let Some(start_pos) = line_is_log_line(&line) {
                    // Flush the buffer if we have any to flush
                    if !buf.is_empty() {
                        // Attempt to parse the non-intro part as a LogStatement
                        match serde_json::from_str(&buf) {
                            Ok(stmt) => {
                                statements.push(stmt);
                            },
                            Err(err) => {
                                error!(
                                    "Failed to parse audit log line(s) as a log statement: {}\n\nLine(s):\n{}\n{}\n{}\n",
                                    err,
                                    (0..80).map(|_| '-').collect::<String>(),
                                    buf,
                                    (0..80).map(|_| '-').collect::<String>()
                                );
                                std::process::exit(1);
                            },
                        };
                        // Clean the buffer to continue
                        buf.clear();
                    }

                    // Add the new line to the buffer
                    buf.push_str(&line[start_pos..]);
                } else {
                    // Add to the buffer
                    buf.push('\n');
                    buf.push_str(&line);
                }
            }

            // Parse the remainder of the buffer, too
            if !buf.is_empty() {
                // Attempt to parse the non-intro part as a LogStatement
                match serde_json::from_str(&buf) {
                    Ok(stmt) => {
                        statements.push(stmt);
                    },
                    Err(err) => {
                        error!(
                            "Failed to parse audit log line(s) as a log statement: {}\n\nLine(s):\n{}\n{}\n{}\n",
                            err,
                            (0..80).map(|_| '-').collect::<String>(),
                            buf,
                            (0..80).map(|_| '-').collect::<String>()
                        );
                        std::process::exit(1);
                    },
                };
            }

            // Now continue with the subcommand to parse the statements
            match log.action {
                LogSubcommands::Reason(reason) => {
                    info!("Handling `log reason` subcommand");

                    // Search statements for reasoner outputs
                    let mut found: bool = false;
                    for stmt in statements {
                        if let LogStatement::ReasonerVerdict { reference, verdict } = stmt {
                            if reason.reference_id != reference {
                                continue;
                            }

                            // Show the verdict
                            let verdict: &Verdict = verdict.as_ref();
                            println!(
                                "Request '{}' was {}",
                                style(reference).bold(),
                                if let Verdict::Allow(_) = verdict { style("AUTHORIZED").bold().green() } else { style("DENIED").bold().red() }
                            );

                            // Mark as found
                            found = true;
                        }
                    }

                    // Show special case if not found
                    if !found {
                        println!("Request '{}' was {} in the audit log", style(&reason.reference_id).bold(), style("not found").bold().yellow());
                    }
                },
            }
        },
    }
}
