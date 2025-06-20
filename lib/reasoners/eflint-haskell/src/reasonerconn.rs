//  REASONERCONN.rs
//    by Lut99
//
//  Created:
//    16 Apr 2025, 23:09:26
//  Last edited:
//    06 May 2025, 12:53:34
//  Auto updated?
//    Yes
//
//  Description:
//!   Defines the actual [`ReasonerConnector`] that wraps the [Haskell
//!   implementation](https://gitlab.com/eflint/haskell-implementation).
//

use std::borrow::Cow;
use std::marker::PhantomData;
use std::path::PathBuf;
use std::process::{ExitStatus, Stdio};
use std::str::FromStr as _;

use error_trace::ErrorTrace as _;
use serde::{Deserialize, Serialize};
use share::formatters::BlockFormatter;
use spec::auditlogger::SessionedAuditLogger;
use spec::reasonerconn::{ReasonerContext, ReasonerResponse};
use spec::{AuditLogger, ReasonerConnector};
use thiserror::Error;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tracing::{debug, warn};

use crate::hash::compute_policy_hash;
use crate::reasons::{Problem, ReasonHandler};
use crate::spec::{EFlintable, EFlintableExt as _};
use crate::trace::{Delta, Trace};

/***** ERRORS *****/
/// Defines errors originating from the [`EFlintHaskellReasonerConnector`].
#[derive(Debug, Error)]
pub enum Error {
    /// Failed to log the context of the reasoner.
    #[error("Failed to log the reasoner's context to {to}")]
    LogContext { to: &'static str, source: error_trace::Trace },
    /// Failed to log the question to the given logger.
    #[error("Failed to log the question to {to}")]
    LogQuestion { to: &'static str, source: error_trace::Trace },
    /// Failed to hash the input policy.
    #[error("Failed to hash the input policy {:}", path.display())]
    PolicyHash { path: PathBuf, source: crate::hash::Error },

    #[error("Empty REPL-command given")]
    EmptyReplCommand,

    #[error("Failed to spawn command {cmd:?}")]
    CommandSpawn { cmd: Command, source: std::io::Error },
    #[error("Failed to write to subprocess stdin")]
    CommandStdinWrite { source: std::io::Error },
    #[error("Failed to wait for command {cmd:?} to complete")]
    CommandJoin { cmd: Command, source: std::io::Error },
    #[error(
        "Command {cmd:?} failed with exit code {code}\n\n{stdout}\n\n{stderr}",
        code = status.code().unwrap_or(-1),
        stdout = BlockFormatter::new("stdout:", stdout),
        stderr = BlockFormatter::new("stderr:", stderr)
    )]
    CommandFailure { cmd: Command, status: ExitStatus, stdout: String, stderr: String },
    #[error("Failed to parse reasoner output\n{output}", output = BlockFormatter::new("stdout:", output))]
    IllegalReasonerResponse { output: String, source: crate::trace::Error },
}





/***** AUXILLARY *****/
/// Defines the public reasoner context for this reasoner.
///
/// This does not include private details; [`EFlintHaskellReasonerContextFull`] contains everything
/// needed for the reasoner to run.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EFlintHaskellReasonerContext {
    // Note: we keep these fields here to serialize/deserialize properly.
    /// The version number of the reasoner itself.
    pub version: String,
    /// The language identifier of this reasoner.
    pub language: String,
    /// The version identifier of the language targeted by this reasoner.
    pub language_version: String,
    /// A hash of the base policy calculated at construction time.
    pub base_policy_hash: [u8; 32],
}
impl ReasonerContext for EFlintHaskellReasonerContext {
    #[inline]
    fn version(&self) -> Cow<'_, str> { Cow::Borrowed(&self.version) }

    #[inline]
    fn language(&self) -> Cow<'_, str> { Cow::Borrowed(&self.language) }

    #[inline]
    fn language_version(&self) -> Cow<'_, str> { Cow::Borrowed(&self.language_version) }
}

/// Defines the full reasoner context for this reasoner.
///
/// This includes private details.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EFlintHaskellReasonerContextFull {
    /// The public part
    pub public: EFlintHaskellReasonerContext,

    // The private part
    /// A command to call the eFLINT reasoner.
    pub cmd: (String, Vec<String>),
    /// The base policy to provide to the eFLINT reasoner.
    pub base_policy: PathBuf,
}
impl ReasonerContext for EFlintHaskellReasonerContextFull {
    #[inline]
    fn version(&self) -> Cow<'_, str> { self.public.version() }

    #[inline]
    fn language(&self) -> Cow<'_, str> { self.public.language() }

    #[inline]
    fn language_version(&self) -> Cow<'_, str> { self.public.language_version() }
}





/***** LIBRARY *****/
/// Defines a connector to make the eFLINT Haskell implementation `policy-reasoner`-complaint.
#[derive(Clone, Debug)]
pub struct EFlintHaskellReasonerConnector<R, S, Q> {
    /// The context for interpreting.
    context: EFlintHaskellReasonerContextFull,
    /// A handler for determining what kind of reasons to give back to the user.
    handler: R,

    /// For us to remember the state we're configured for.
    _state:    PhantomData<S>,
    /// For us to remember the questions we're configured for.
    _question: PhantomData<Q>,
}
impl<R, S, Q> EFlintHaskellReasonerConnector<R, S, Q> {
    /// Constructor for the EFlintHaskellReasonerConnector.
    ///
    /// # Arguments
    /// - `cmd`: Some command that is used to call the eFLINT reasoner.
    /// - `base_policy_path`: A path to an eFLINT file containing the base policy to load. We load
    ///   this as a file instead of a string since that is MUCH more efficient than feeding large
    ///   files to eFLINT by pipe.
    /// - `handler`: Some [`ReasonHandler`] that can be used to determine what information to return to the user upon failure.
    /// - `logger`: An [`AuditLogger`] for logging the reasoning context with.
    ///
    /// # Returns
    /// A new EFlintHaskellReasonerConnector ready to reason.
    ///
    /// # Errors
    /// This function can error if it failed to log the initial context to the given `logger`.
    pub async fn new_async<L: AuditLogger>(
        cmd: impl IntoIterator<Item = String>,
        base_policy_path: impl Into<PathBuf>,
        handler: R,
        logger: &L,
    ) -> Result<Self, Error> {
        let base_policy: PathBuf = base_policy_path.into();

        // Get the command and split it in a program and arguments
        let mut cmd: Vec<String> = cmd.into_iter().collect();
        let exec: Option<String> = cmd.pop();
        let cmd: (String, Vec<String>) = (exec.ok_or(Error::EmptyReplCommand)?, cmd);

        // Compute the hash of the input policy
        let base_policy_hash: [u8; 32] =
            compute_policy_hash(&base_policy, &[]).await.map_err(|source| Error::PolicyHash { path: base_policy.clone(), source })?;

        // Build the context & log it
        let context: EFlintHaskellReasonerContextFull = EFlintHaskellReasonerContextFull {
            public: EFlintHaskellReasonerContext {
                version: env!("CARGO_PKG_VERSION").into(),
                language: "eflint".into(),
                // NOTE: This really needn't be accurate, but it's kind of hard to check the eFLINT version from the Haskell version atm.
                language_version: "4.0.0.1".into(),
                base_policy_hash,
            },
            cmd,
            base_policy,
        };
        logger.log_context(&context).await.map_err(|err| Error::LogContext { to: std::any::type_name::<L>(), source: err.freeze() })?;

        // OK, return ourselves
        Ok(Self { context, handler, _state: PhantomData, _question: PhantomData })
    }

    /// Returns the command used to call the `eflint-repl` binary.
    ///
    /// # Returns
    /// A pair of the executable and a list of arguments that represents the command.
    #[inline]
    pub const fn cmd(&self) -> &(String, Vec<String>) { &self.context.cmd }

    /// Returns the path of the base policy provided to every reasoner call.
    ///
    /// Note that the given file may depend on other eFLINT files. If you want to find all files,
    /// then call [`find_deps()`](crate::hash::find_deps()) on the resulting file.
    ///
    /// # Returns
    /// A [`PathBuf`] representing this file.
    #[inline]
    pub const fn base_policy(&self) -> &PathBuf { &self.context.base_policy }
}
impl<R, S, Q> ReasonerConnector for EFlintHaskellReasonerConnector<R, S, Q>
where
    R: Sync + ReasonHandler,
    S: Send + Sync + EFlintable + Serialize,
    Q: Send + Sync + EFlintable + Serialize,
{
    type Context = EFlintHaskellReasonerContext;
    type Error = Error;
    type Question = Q;
    type Reason = R::Reason;
    type State = S;

    #[inline]
    fn context(&self) -> Self::Context { self.context.public.clone() }

    #[inline]
    async fn consult<'a, L>(
        &'a self,
        state: Self::State,
        question: Self::Question,
        logger: &'a SessionedAuditLogger<L>,
    ) -> Result<ReasonerResponse<Self::Reason>, Self::Error>
    where
        L: Sync + AuditLogger,
    {
        logger
            .log_question(&state, &question)
            .await
            .map_err(|err| Error::LogQuestion { to: std::any::type_name::<SessionedAuditLogger<L>>(), source: err.freeze() })?;

        // Prepare the full file to send
        let spec: String = format!("{}{}", state.eflint(), question.eflint());
        debug!("{}", BlockFormatter::new("Full spec to submit to reasoner:", &spec));

        // Prepare the command to execute
        let mut cmd = Command::new(&self.context.cmd.0);
        cmd.args(&self.context.cmd.1);
        cmd.arg(&self.context.base_policy);
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        // Attempt to execute it, sending the full spec on the input
        // NOTE: Using match to avoid moving `cmd` a closure and having to clone it (which it can't)
        debug!("Calling reasoner with {cmd:?}...");
        let mut handle = match cmd.spawn() {
            Ok(handle) => handle,
            Err(source) => return Err(Error::CommandSpawn { cmd, source }),
        };
        handle
            .stdin
            .as_mut()
            .expect("No stdin on subprocess even though it's piped!")
            .write_all(spec.as_bytes())
            .await
            .map_err(|source| Error::CommandStdinWrite { source })?;
        debug!("Inputs submitted, waiting for reasoner to complete...");
        let output = match handle.wait_with_output().await {
            Ok(handle) => handle,
            Err(source) => return Err(Error::CommandJoin { cmd, source }),
        };
        if !output.status.success() {
            return Err(Error::CommandFailure {
                cmd,
                status: output.status,
                stdout: String::from_utf8_lossy(&output.stdout).into(),
                stderr: String::from_utf8_lossy(&output.stderr).into(),
            });
        }

        // Stript the prompts from the eFLINT output
        let output: Cow<str> = String::from_utf8_lossy(&output.stdout);
        let mut clean_output: String = String::with_capacity(output.len());
        let mut buf: String = String::new();
        let mut state: usize = 0;
        for c in output.chars() {
            // Loop exists to be able to examine some chars again
            loop {
                match state {
                    // Finding pounds
                    0 if c == '#' => {
                        buf.push('#');
                        state = 1;
                        break;
                    },
                    0 => {
                        clean_output.push(c);
                        break;
                    },

                    // Parsing numbers & whitespace
                    1 if c.is_ascii_digit() || c.is_whitespace() => {
                        buf.push(c);
                        break;
                    },
                    1 if c == '>' => {
                        state = 0;
                        break;
                    },
                    1 => {
                        clean_output.push_str(&buf);
                        buf.clear();
                        state = 0;
                        // Don't break, re-try this character
                    },

                    _ => unreachable!(),
                }
            }
        }

        // Attempt to parse the output
        debug!("{}", BlockFormatter::new("Reasoner output:", &clean_output));
        let trace: Trace = match Trace::from_str(clean_output.as_ref()) {
            Ok(trace) => trace,
            Err(source) => return Err(Error::IllegalReasonerResponse { output: clean_output, source }),
        };
        debug!("{}", BlockFormatter::new("Reasoner trace:", &trace));

        // Analyze the output to find violations
        // The rule is:
        // 1. Check the last delta
        //    a. If it's a query, then it must succeed; or
        //    b. If it's not a query, it must not be a violation.
        // 2. If there is no last delta, then we default to **success**.
        let problems: Vec<Problem> = trace
            .deltas
            .iter()
            .filter_map(|delta| match delta {
                Delta::Query(query) if query.is_success() => Some(Problem::QueryFailed),
                Delta::Violation(viol) => Some(Problem::Violation(viol.clone())),
                _ => None,
            })
            .collect();
        let res: ReasonerResponse<R::Reason> = trace
            .deltas
            .into_iter()
            .next_back()
            .map(|delta| match delta {
                Delta::Query(query) if query.is_success() => ReasonerResponse::Success,
                Delta::Query(_) => ReasonerResponse::Violated(self.handler.handle(problems)),
                Delta::Violation(_) => ReasonerResponse::Violated(self.handler.handle(problems)),
                delta => {
                    warn!("Got non-query, non-violation delta as last delta ({delta:?}); assuming OK");
                    ReasonerResponse::Success
                },
            })
            .unwrap_or(ReasonerResponse::Success);

        Ok(res)
    }
}
