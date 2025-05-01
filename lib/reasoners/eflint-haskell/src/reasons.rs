//  REASONS.rs
//    by Lut99
//
//  Created:
//    25 Apr 2025, 16:36:41
//  Last edited:
//    01 May 2025, 16:46:45
//  Auto updated?
//    Yes
//
//  Description:
//!   Defines reason handlers for the haskell interpreter.
//

use std::borrow::Cow;
use std::fmt::{Display, Formatter, Result as FResult};

use serde::{Deserialize, Serialize};

use crate::trace::Violation;


/***** AUXILLARY *****/
/// Defines an empty reason.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct NoReason;
impl Display for NoReason {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult { write!(f, "<REDACTED>") }
}

/// Defines an optional reason.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct OptReason<R>(pub Option<R>);
impl<R: Display> Display for OptReason<R> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        match &self.0 {
            Some(r) => r.fmt(f),
            None => write!(f, "<REDACTED>"),
        }
    }
}



/// Defines either a [`Query`] or a [`Violation`].
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Problem {
    QueryFailed,
    Violation(Violation),
}
impl Display for Problem {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        match self {
            Self::QueryFailed => write!(f, "Query failed"),
            Self::Violation(v) => v.fmt(f),
        }
    }
}





/***** INTERFACES *****/
/// Converts a failed query/violation into a Reason.
pub trait ReasonHandler {
    /// The reasons returned by this handler.
    type Reason: Display;

    /// Maps a query/violation to a reason.
    ///
    /// # Arguments
    /// - `problem`: A [`Problem::QueryFailed`] or a [`Problem::Violation`] that describes the
    ///   reason.
    ///
    /// # Returns
    /// A [`Self::Reason`](ReasonHandler::Reason) that represents the outputted reason.
    fn handle(&self, problem: Problem) -> Self::Reason;
}





/***** LIBRARY *****/
/// Reason handler that doesn't report anything.
#[derive(Clone, Debug)]
pub struct SilentHandler;
impl ReasonHandler for SilentHandler {
    type Reason = NoReason;

    #[inline]
    fn handle(&self, _problem: Problem) -> Self::Reason { NoReason }
}



/// Reason handler reports only violations with a specific prefix.
#[derive(Clone, Debug)]
pub struct PrefixedHandler<'s> {
    pub prefix: Cow<'s, str>,
}
impl<'s> PrefixedHandler<'s> {
    /// Constructor for the PrefixedHandler.
    ///
    /// # Arguments
    /// - `prefix`: The prefix to match violations on.
    ///
    /// # Returns
    /// A new PrefixedHandler that will only pass violations to the user that violate something
    /// starting with the given prefix.
    #[inline]
    pub fn new(prefix: impl Into<Cow<'s, str>>) -> Self { Self { prefix: prefix.into() } }
}
impl<'s> ReasonHandler for PrefixedHandler<'s> {
    type Reason = OptReason<Violation>;

    #[inline]
    fn handle(&self, problem: Problem) -> Self::Reason {
        match problem {
            Problem::QueryFailed => OptReason(None),
            Problem::Violation(Violation::Act(a)) => {
                if a.inst.name.starts_with(self.prefix.as_ref()) {
                    OptReason(Some(Violation::Act(a)))
                } else {
                    OptReason(None)
                }
            },
            Problem::Violation(Violation::Duty(d)) => {
                if d.inst.name.starts_with(self.prefix.as_ref()) {
                    OptReason(Some(Violation::Duty(d)))
                } else {
                    OptReason(None)
                }
            },
            Problem::Violation(Violation::Invariant(i)) => {
                if i.name.starts_with(self.prefix.as_ref()) {
                    OptReason(Some(Violation::Invariant(i)))
                } else {
                    OptReason(None)
                }
            },
        }
    }
}



/// Reason handler reports everything.
#[derive(Clone, Debug)]
pub struct VerboseHandler;
impl ReasonHandler for VerboseHandler {
    type Reason = Problem;

    #[inline]
    fn handle(&self, problem: Problem) -> Self::Reason { problem }
}
