//  REASONERCONN.rs
//    by Lut99
//
//  Created:
//    10 Oct 2024, 16:21:09
//  Last edited:
//    17 Oct 2024, 09:50:23
//  Auto updated?
//    Yes
//
//  Description:
//!   <Todo>
//

use std::error;
use std::fmt::{Display, Formatter, Result as FResult};
use std::future::Future;
use std::marker::PhantomData;

use error_trace::{ErrorTrace as _, Trace};
use spec::auditlogger::SessionedAuditLogger;
use spec::reasonerconn::ReasonerResponse;
use spec::{AuditLogger, ReasonerConnector};
use tracing::{Level, debug, span};


/***** ERRORS *****/
#[derive(Debug)]
pub enum Error {
    /// Failed to log the reasoner's response to the given logger.
    LogResponse { to: &'static str, err: Trace },
}
impl Display for Error {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        use Error::*;
        match self {
            LogResponse { to, .. } => write!(f, "Failed to log the reasoner's response to {to}"),
        }
    }
}
impl error::Error for Error {
    #[inline]
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        use Error::*;
        match self {
            LogResponse { err, .. } => Some(err),
        }
    }
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
impl<Q> ReasonerConnector for NoOpReasonerConnector<Q> {
    type Error = Error;
    type Question = Q;
    type Reason = ();
    type State = ();

    fn consult<L>(
        &self,
        _state: Self::State,
        _question: Self::Question,
        logger: &mut SessionedAuditLogger<L>,
    ) -> impl Future<Output = Result<ReasonerResponse<Self::Reason>, Self::Error>>
    where
        L: AuditLogger,
    {
        async move {
            // NOTE: Using `#[instrument]` adds some unnecessary trait bounds on `S` and such.
            let _span = span!(Level::INFO, "NoOpReasonerConnector::consult", reference = logger.reference()).entered();

            // Log the reasoner has been called
            debug!("NoOpReasonerConnector: request received");
            logger
                .log_response::<u8>(&ReasonerResponse::Success, None)
                .await
                .map_err(|err| Error::LogResponse { to: std::any::type_name::<SessionedAuditLogger<L>>(), err: err.freeze() })?;

            Ok(ReasonerResponse::Success)
        }
    }
}
