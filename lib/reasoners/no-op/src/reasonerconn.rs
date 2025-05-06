//  REASONERCONN.rs
//    by Lut99
//
//  Created:
//    10 Oct 2024, 16:21:09
//  Last edited:
//    06 May 2025, 12:51:00
//  Auto updated?
//    Yes
//
//  Description:
//!   Implements a reasoner connector that doesn't do ANYTHING (lazy thing).
//

use std::borrow::Cow;
use std::future::Future;
use std::marker::PhantomData;

use error_trace::{ErrorTrace as _, Trace};
use serde::{Deserialize, Serialize};
use spec::auditlogger::SessionedAuditLogger;
use spec::reasonerconn::{ReasonerContext, ReasonerResponse};
use spec::{AuditLogger, ReasonerConnector};
use thiserror::Error;
use tracing::{Instrument as _, Level, debug, span};


/***** ERRORS *****/
#[derive(Debug, Error)]
pub enum Error {
    /// Failed to log the reasoner's context to the given logger.
    #[error("Failed to log the reasoner's context to {to}")]
    LogContext {
        to:  &'static str,
        #[source]
        err: Trace,
    },
    /// Failed to log the reasoner's response to the given logger.
    #[error("Failed to log the reasoner's response to {to}")]
    LogResponse {
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
}





/***** AUXILLARY *****/
/// The [`ReasonerContext`] returned by the [`NoOpReasonerConnector`].
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NoOpReasonerContext {
    /// The version of this reasoner.
    pub version: String,
    /// The language identifier of this reasoner.
    pub language: String,
    /// The language's version identifier of this reasoner.
    pub language_version: String,
}
impl Default for NoOpReasonerContext {
    #[inline]
    fn default() -> Self { Self { version: env!("CARGO_PKG_VERSION").into(), language: "no-op".into(), language_version: "v1".into() } }
}
impl ReasonerContext for NoOpReasonerContext {
    #[inline]
    fn version(&self) -> Cow<str> { Cow::Borrowed(&self.version) }

    #[inline]
    fn language(&self) -> Cow<str> { Cow::Borrowed(&self.language) }

    #[inline]
    fn language_version(&self) -> Cow<str> { Cow::Borrowed(&self.language_version) }
}





/***** LIBRARY *****/
/// The minimal no-operation reasoner connector, that approves all validation requests by default (it does not check any
/// policy/permissions).
#[derive(Clone, Copy, Debug)]
pub struct NoOpReasonerConnector<Q> {
    /// The completely arbitrary question that can be asked.
    _question: PhantomData<Q>,
}
impl<Q> NoOpReasonerConnector<Q> {
    /// Constructor for the NoOpReasonerConnector.
    ///
    /// This constructor logs asynchronously.
    ///
    /// # Arguments
    /// - `logger`: A logger to write this reasoner's context to.
    ///
    /// # Errors
    /// This function may error if it failed to log to the given `logger`.
    #[inline]
    pub async fn new_async<L: AuditLogger>(logger: &mut L) -> Result<Self, Error> {
        logger
            .log_context(&NoOpReasonerContext::default())
            .await
            .map_err(|err| Error::LogContext { to: std::any::type_name::<L>(), err: err.freeze() })?;
        Ok(Self { _question: PhantomData })
    }
}
impl<Q> ReasonerConnector for NoOpReasonerConnector<Q>
where
    Q: Send + Sync + Serialize,
{
    type Context = NoOpReasonerContext;
    type Error = Error;
    type Question = Q;
    type Reason = ();
    type State = ();

    #[inline]
    fn context(&self) -> Self::Context { NoOpReasonerContext::default() }

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
            debug!("NoOpReasonerConnector: request received");

            // Log that the question has been asked
            logger
                .log_question(&state, &question)
                .await
                .map_err(|err| Error::LogQuestion { to: std::any::type_name::<SessionedAuditLogger<L>>(), err: err.freeze() })?;

            // Log the reasoner has been called
            logger
                .log_response::<u8>(&ReasonerResponse::Success, None)
                .await
                .map_err(|err| Error::LogResponse { to: std::any::type_name::<SessionedAuditLogger<L>>(), err: err.freeze() })?;

            Ok(ReasonerResponse::Success)
        }
        .instrument(span!(Level::INFO, "NoOpReasonerConnector::consult", reference = logger.reference()))
    }
}
