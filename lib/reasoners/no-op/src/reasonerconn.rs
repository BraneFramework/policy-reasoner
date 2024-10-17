//  REASONERCONN.rs
//    by Lut99
//
//  Created:
//    10 Oct 2024, 16:21:09
//  Last edited:
//    17 Oct 2024, 13:56:10
//  Auto updated?
//    Yes
//
//  Description:
//!   <Todo>
//

use std::future::Future;
use std::marker::PhantomData;

use error_trace::{ErrorTrace as _, Trace};
use serde::Serialize;
use spec::auditlogger::SessionedAuditLogger;
use spec::reasonerconn::ReasonerResponse;
use spec::{AuditLogger, ReasonerConnector};
use thiserror::Error;
use tracing::{Level, debug, span};


/***** ERRORS *****/
#[derive(Debug, Error)]
pub enum Error {
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





/***** LIBRARY *****/
/// The minimal no-operation reasoner connector, that approves all validation requests by default (it does not check any
/// policy/permissions).
#[derive(Clone, Copy, Debug)]
pub struct NoOpReasonerConnector<Q> {
    /// The completely arbitrary question that can be asked.
    _question: PhantomData<Q>,
}
impl<Q> Default for NoOpReasonerConnector<Q> {
    #[inline]
    fn default() -> Self { Self::new() }
}
impl<Q> NoOpReasonerConnector<Q> {
    /// Constructor for the NoOpReasonerConnector.
    ///
    /// # Returns
    /// A new connector, ready to allow anything in sight.
    #[inline]
    pub fn new() -> Self { Self { _question: PhantomData } }
}
impl<Q> ReasonerConnector for NoOpReasonerConnector<Q>
where
    Q: Serialize,
{
    type Error = Error;
    type Question = Q;
    type Reason = ();
    type State = ();

    fn consult<L>(
        &self,
        state: Self::State,
        question: Self::Question,
        logger: &mut SessionedAuditLogger<L>,
    ) -> impl Future<Output = Result<ReasonerResponse<Self::Reason>, Self::Error>>
    where
        L: AuditLogger,
    {
        async move {
            // NOTE: Using `#[instrument]` adds some unnecessary trait bounds on `S` and such.
            let _span = span!(Level::INFO, "NoOpReasonerConnector::consult", reference = logger.reference()).entered();
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
    }
}
