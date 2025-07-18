//  REASONS.rs
//    by Lut99
//
//  Created:
//    25 Apr 2025, 16:36:41
//  Last edited:
//    06 May 2025, 12:44:08
//  Auto updated?
//    Yes
//
//  Description:
//!   Defines reason handlers for the haskell interpreter.
//

use std::borrow::Cow;
use std::fmt::{Display, Formatter, Result as FResult};

use serde::{Deserialize, Serialize};
use spec::reasons::{ManyReason, NoReason};

use crate::trace::Violation;


/***** AUXILLARY *****/
/// Defines either a failed [`Query`](crate::trace::Query) or a [`Violation`].
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
    /// - `problems`: A sequence of [`Problem::QueryFailed`]s and/or [`Problem::Violation`]s that
    ///   describes the reasons.
    ///
    /// # Returns
    /// A [`Self::Reason`](ReasonHandler::Reason) that represents the outputted reason.
    fn handle(&self, problems: impl IntoIterator<Item = Problem>) -> Self::Reason;
}





/***** LIBRARY *****/
/// Reason handler that doesn't report anything.
#[derive(Clone, Debug)]
pub struct SilentHandler;
impl ReasonHandler for SilentHandler {
    type Reason = NoReason;

    #[inline]
    fn handle(&self, _problems: impl IntoIterator<Item = Problem>) -> Self::Reason { NoReason }
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
impl ReasonHandler for PrefixedHandler<'_> {
    type Reason = ManyReason<String>;

    #[inline]
    fn handle(&self, problems: impl IntoIterator<Item = Problem>) -> Self::Reason {
        let mut reason = ManyReason::new();
        for problem in problems {
            match problem {
                Problem::QueryFailed => continue,
                Problem::Violation(Violation::Act(a)) => {
                    if a.inst.name.starts_with(self.prefix.as_ref()) {
                        reason.push(Violation::Act(a).to_string());
                    }
                },
                Problem::Violation(Violation::Duty(d)) => {
                    if d.inst.name.starts_with(self.prefix.as_ref()) {
                        reason.push(Violation::Duty(d).to_string());
                    }
                },
                Problem::Violation(Violation::Invariant(i)) => {
                    if i.name.starts_with(self.prefix.as_ref()) {
                        reason.push(Violation::Invariant(i).to_string());
                    }
                },
            }
        }
        reason
    }
}



/// Reason handler reports everything.
#[derive(Clone, Debug)]
pub struct VerboseHandler;
impl ReasonHandler for VerboseHandler {
    type Reason = ManyReason<String>;

    #[inline]
    fn handle(&self, problems: impl IntoIterator<Item = Problem>) -> Self::Reason {
        ManyReason::from_iter(problems.into_iter().map(|p| p.to_string()))
    }
}
