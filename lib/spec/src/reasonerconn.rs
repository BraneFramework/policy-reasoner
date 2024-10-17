//  REASONERCONN.rs
//    by Lut99
//
//  Created:
//    09 Oct 2024, 13:35:41
//  Last edited:
//    17 Oct 2024, 09:50:17
//  Auto updated?
//    Yes
//
//  Description:
//!   Defines the interface to the backend reasoner.
//

use std::error::Error;
use std::future::Future;

use serde::{Deserialize, Serialize};

use crate::auditlogger::{AuditLogger, SessionedAuditLogger};


/***** AUXILLARY *****/
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





/***** LIBRARY *****/
/// Defines the interface with the backend reasoner.
pub trait ReasonerConnector {
    /// The type of state that this reasoner accepts.
    type State;
    /// The type of question that this reasoner accepts.
    type Question;
    /// Any reason(s) that are given by the reasoner that explain why something is violating.
    type Reason;
    /// The error returned by the reasoner.
    type Error: Error;


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
    fn consult<L>(
        &self,
        state: Self::State,
        question: Self::Question,
        logger: &mut SessionedAuditLogger<L>,
    ) -> impl Future<Output = Result<ReasonerResponse<Self::Reason>, Self::Error>>
    where
        L: AuditLogger;
}
