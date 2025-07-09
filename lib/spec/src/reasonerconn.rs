//  REASONERCONN.rs
//    by Lut99
//
//  Created:
//    09 Oct 2024, 13:35:41
//  Last edited:
//    01 May 2025, 16:48:24
//  Auto updated?
//    Yes
//
//  Description:
//!   Defines the interface to the backend reasoner.
//

use std::borrow::Cow;
use std::error::Error;
use std::fmt::{Display, Formatter, Result as FResult};
use std::future::Future;

use serde::{Deserialize, Serialize};

use crate::auditlogger::{AuditLogger, SessionedAuditLogger};


/***** AUXILLARY *****/
/// Defines the general information contained within a [`ReasonerConnector::Context`].
pub trait ReasonerContext: Serialize {
    /// Returns some identifier for the specific reasoner version.
    ///
    /// This is useful, because users may (want) to depend on specific non-language, but
    /// yes-reasoner, features.
    ///
    /// # Returns
    /// A string identifier denoting this reasoner's version.
    fn version(&self) -> Cow<'_, str>;

    /// Returns some identifier of the language that's being used as backend.
    ///
    /// # Returns
    /// A string identifier that tells users the language used.
    fn language(&self) -> Cow<'_, str>;

    /// Returns some identifier of the specific language version being used as backend.
    ///
    /// This would usually be a semantic version number. However, it may also denote specific
    /// dialects or additional extensions.
    ///
    /// # Returns
    /// A string identifier that tells users which version of the backend
    /// [language](ReasonerContext::language()) is being used.
    fn language_version(&self) -> Cow<'_, str>;
}



/// Defines the result of a reasoner.
///
/// # Generics
/// - `R`: A type that describes the reason(s) for the query being violating.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum ReasonerResponse<R> {
    /// The state is compliant to the policy w.r.t. the question.
    Success,
    /// The state is _not_ compliant to the policy w.r.t. the question.
    Violated(R),
}
impl<R: Display> Display for ReasonerResponse<R> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        match self {
            Self::Success => write!(f, "SUCCESS"),
            Self::Violated(r) => {
                write!(f, "VIOLATION({r})")
            },
        }
    }
}





/***** LIBRARY *****/
/// Defines the interface with the backend reasoner.
pub trait ReasonerConnector {
    /// Some context returned that describes this reasoner for policy writers.
    type Context: ReasonerContext;
    /// The type of state that this reasoner accepts.
    type State;
    /// The type of question that this reasoner accepts.
    type Question;
    /// Any reason(s) that are given by the reasoner that explain why something is violating.
    type Reason;
    /// The error returned by the reasoner.
    type Error: Error;


    /// Retrieves some context of the connector that is relevant for people writing policy.
    ///
    /// Ideally, the context allows policy reasoners to uniquely identify everything they need to
    /// know to write policy in the appropriate language, version and interface required by the
    /// reasoner.
    ///
    /// # Returns
    /// A [`Context`](ReasonerConnector::Context) that describes this context.
    fn context(&self) -> Self::Context;

    /// Sends a policy to the backend reasoner.
    ///
    /// # Arguments
    /// - `state`: The [`ReasonerConnector::State`] that describes the state to check in the reasoner.
    /// - `question`: The [`ReasonerConnector::Question`] that selects exactly what kind of compliance is being checked.
    /// - `logger`: A [`SessionedAuditLogger`] wrapping some [`AuditLogger`] that is used to write to the audit trail as the question's being asked.
    ///
    /// # Returns
    /// A [`ReasonerResponse`] that describes the answer to the `question` of compliance of the `state`.
    ///
    /// # Errors
    /// This function may error if the reasoner was unreachable or did not respond (correctly).
    fn consult<'a, L>(
        &'a self,
        state: Self::State,
        question: Self::Question,
        logger: &'a SessionedAuditLogger<L>,
    ) -> impl 'a + Send + Future<Output = Result<ReasonerResponse<Self::Reason>, Self::Error>>
    where
        L: Sync + AuditLogger;
}
