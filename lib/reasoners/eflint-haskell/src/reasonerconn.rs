//  REASONERCONN.rs
//    by Lut99
//
//  Created:
//    16 Apr 2025, 23:09:26
//  Last edited:
//    17 Apr 2025, 00:04:49
//  Auto updated?
//    Yes
//
//  Description:
//!   Defines the actual [`ReasonerConnector`] that wraps the [Haskell
//!   implementation](https://gitlab.com/eflint/haskell-implementation).
//

use std::borrow::Cow;
use std::future::Future;
use std::marker::PhantomData;
use std::process::{ExitStatus, Stdio};

use error_trace::{ErrorTrace as _, Trace};
use serde::{Deserialize, Serialize};
use spec::auditlogger::SessionedAuditLogger;
use spec::reasonerconn::{ReasonerContext, ReasonerResponse};
use spec::{AuditLogger, ReasonerConnector};
use thiserror::Error;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tracing::debug;

use crate::spec::{EFlintable, EFlintableExt as _};


/***** ERRORS *****/
/// Defines errors originating from the [`EFlintHaskellReasonerConnector`].
#[derive(Debug, Error)]
pub enum Error {
    /// Failed to log the context of the reasoner.
    #[error("Failed to log the reasoner's context to {to}")]
    LogContext {
        to:  &'static str,
        #[source]
        err: Trace,
    },
    /// Failed to log the question to the given logger.
    #[error("Failed to log the question to {to}")]
    LogQuestion {
        to:  &'static str,
        #[source]
        err: Trace,
    },

    #[error("Empty REPL-command given")]
    EmptyReplCommand,

    #[error("Failed to spawn command {cmd:?}")]
    CommandSpawn {
        cmd: Command,
        #[source]
        err: std::io::Error,
    },
    #[error("Failed to write to subprocess stdin")]
    CommandStdinWrite {
        #[source]
        err: std::io::Error,
    },
    #[error("Failed to wait for command {cmd:?} to complete")]
    CommandJoin {
        cmd: Command,
        #[source]
        err: std::io::Error,
    },
    #[error("Command {:?} failed with exit code {}\n\nstdout:\n{}\n{}\n{}\n\nstderr:\n{}\n{}\n{}\n", cmd, status.code().unwrap_or(-1), "-".repeat(80), stdout, "-".repeat(80), "-".repeat(80), stderr, "-".repeat(80))]
    CommandFailure { cmd: Command, status: ExitStatus, stdout: String, stderr: String },
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
}
impl Default for EFlintHaskellReasonerContext {
    #[inline]
    fn default() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").into(),
            language: "eflint".into(),
            // NOTE: This really needn't be accurate, but it's kind of hard to check the eFLINT version from the Haskell version atm.
            language_version: "4.0.0.1".into(),
        }
    }
}
impl ReasonerContext for EFlintHaskellReasonerContext {
    #[inline]
    fn version(&self) -> Cow<str> { Cow::Borrowed(&self.version) }

    #[inline]
    fn language(&self) -> Cow<str> { Cow::Borrowed(&self.language) }

    #[inline]
    fn language_version(&self) -> Cow<str> { Cow::Borrowed(&self.language_version) }
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
}
impl ReasonerContext for EFlintHaskellReasonerContextFull {
    #[inline]
    fn version(&self) -> Cow<str> { self.public.version() }

    #[inline]
    fn language(&self) -> Cow<str> { self.public.language() }

    #[inline]
    fn language_version(&self) -> Cow<str> { self.public.language_version() }
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
    /// - `handler`: Some [`ReasonHandler`] that can be used to determine what information to return to the user upon failure.
    /// - `logger`: An [`AuditLogger`] for logging the reasoning context with.
    ///
    /// # Returns
    /// A new EFlintHaskellReasonerConnector ready to reason.
    ///
    /// # Errors
    /// This function can error if it failed to log the initial context to the given `logger`.
    pub async fn new_async<'l, L: AuditLogger>(cmd: impl IntoIterator<Item = String>, handler: R, logger: &'l L) -> Result<Self, Error> {
        // Get the command and split it in a program and arguments
        let mut cmd: Vec<String> = cmd.into_iter().collect();
        let exec: Option<String> = cmd.pop();
        let cmd: (String, Vec<String>) = (exec.ok_or(Error::EmptyReplCommand)?, cmd);

        // Build the context & log it
        let context: EFlintHaskellReasonerContextFull = EFlintHaskellReasonerContextFull { public: EFlintHaskellReasonerContext::default(), cmd };
        logger.log_context(&context).await.map_err(|err| Error::LogContext { to: std::any::type_name::<L>(), err: err.freeze() })?;

        // OK, return ourselves
        Ok(Self { context, handler, _state: PhantomData, _question: PhantomData })
    }
}
impl<R, S, Q> ReasonerConnector for EFlintHaskellReasonerConnector<R, S, Q>
where
    R: Sync,
    S: Send + Sync + EFlintable + Serialize,
    Q: Send + Sync + EFlintable + Serialize,
{
    type Context = EFlintHaskellReasonerContext;
    type Error = Error;
    type Question = Q;
    type Reason = R;
    type State = S;

    #[inline]
    fn context(&self) -> Self::Context { self.context.public.clone() }

    #[inline]
    fn consult<'a, L>(
        &'a self,
        state: Self::State,
        question: Self::Question,
        logger: &'a SessionedAuditLogger<L>,
    ) -> impl 'a + Send + Future<Output = Result<ReasonerResponse<Self::Reason>, Self::Error>>
    where
        L: Sync + AuditLogger,
    {
        async move {
            logger
                .log_question(&state, &question)
                .await
                .map_err(|err| Error::LogQuestion { to: std::any::type_name::<SessionedAuditLogger<L>>(), err: err.freeze() })?;

            // Prepare the full file to send
            let spec: String = format!("{}{}", state.eflint(), question.eflint());
            debug!("Full spec to submit to reasoner:{}\n{}\n{}\n", "-".repeat(80), spec, "-".repeat(80));

            // Prepare the command to execute
            let mut cmd = Command::new(&self.context.cmd.0);
            cmd.args(&self.context.cmd.1);
            cmd.stdin(Stdio::piped());
            cmd.stdout(Stdio::piped());
            cmd.stderr(Stdio::piped());

            // Attempt to execute it, sending the full spec on the input
            // NOTE: Using match to avoid moving `cmd` a closure and having to clone it (which it can't)
            debug!("Calling reasoner with {cmd:?}...");
            let mut handle = match cmd.spawn() {
                Ok(handle) => handle,
                Err(err) => return Err(Error::CommandSpawn { cmd, err }),
            };
            handle
                .stdin
                .as_mut()
                .unwrap_or_else(|| panic!("No stdin on subprocess even though it's piped!"))
                .write_all(spec.as_bytes())
                .await
                .map_err(|err| Error::CommandStdinWrite { err })?;
            debug!("Inputs submitted, waiting for reasoner to complete...");
            let output = match handle.wait_with_output().await {
                Ok(handle) => handle,
                Err(err) => return Err(Error::CommandJoin { cmd, err }),
            };
            if !output.status.success() {
                return Err(Error::CommandFailure {
                    cmd,
                    status: output.status,
                    stdout: String::from_utf8_lossy(&output.stdout).into(),
                    stderr: String::from_utf8_lossy(&output.stderr).into(),
                });
            }

            // Attempt to parse the output
            let output: Cow<str> = String::from_utf8_lossy(&output.stdout);
            debug!("Reasoner output:\n{}\n{}\n{}\n", "-".repeat(80), output, "-".repeat(80));

            todo!()
        }
    }
}
