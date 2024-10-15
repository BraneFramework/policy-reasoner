//  REASONS.rs
//    by Lut99
//
//  Created:
//    09 Oct 2024, 16:37:52
//  Last edited:
//    11 Oct 2024, 14:04:46
//  Auto updated?
//    Yes
//
//  Description:
//!   Defines how the eFLINT reasoner deals with reasons for failure.
//

use std::convert::Infallible;
use std::error::Error;
use std::fmt::{Debug, Display, Formatter, Result as FResult};
use std::ops::{Deref, DerefMut};

use eflint_json::spec::ResponsePhrases;


/***** AUXILLARY *****/
/// Represents that no reason is used.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct NoReason;
impl Display for NoReason {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult { write!(f, "<no reason>") }
}

/// Represents that multiple reasons can be given.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ManyReason<R>(Vec<R>);
impl<R> Default for ManyReason<R> {
    #[inline]
    fn default() -> Self { Self::new() }
}
impl<R> ManyReason<R> {
    /// Constructor for the ManyReason that initializes it as empty.
    ///
    /// # Returns
    /// A new ManyReason that doesn't have any reasons embedded in it yet.
    #[inline]
    pub fn new() -> Self { Self(Vec::new()) }

    /// Constructor for the ManyReason that initializes it as empty but with space allocated for
    /// a certain number of reasons.
    ///
    /// # Arguments
    /// - `capacity`: The (minimum) number of reasons to allocate space for.
    ///
    /// # Returns
    /// A new ManyReason that doesn't have any reasons embedded in it yet but space for at least `capacity` reasons.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self { Self(Vec::with_capacity(capacity)) }
}
impl<R: Display> Display for ManyReason<R> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        for i in 0..self.0.len() {
            if i > 0 && i < self.0.len() - 1 {
                write!(f, ", ")?;
            } else if i == self.0.len() {
                write!(f, " and ")?;
            }
            write!(f, "{}", self.0[i])?;
        }
        Ok(())
    }
}
impl<R> Deref for ManyReason<R> {
    type Target = Vec<R>;

    #[inline]
    fn deref(&self) -> &Self::Target { &self.0 }
}
impl<R> DerefMut for ManyReason<R> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.0 }
}
impl<R> FromIterator<R> for ManyReason<R> {
    #[inline]
    fn from_iter<T: IntoIterator<Item = R>>(iter: T) -> Self { Self(iter.into_iter().collect()) }
}
impl<R, I: IntoIterator<Item = R>> From<I> for ManyReason<R> {
    #[inline]
    fn from(value: I) -> Self { Self::from_iter(value) }
}





/***** LIBRARY *****/
/// Abstracts over different strategies for handling errors.
pub trait ReasonHandler {
    /// The type of the reason(s) returned by this handler.
    type Reason: Debug;
    /// The type of error(s) returned by this handler.
    type Error: Error;


    /// Given an eFLINT response struct, return reasons for failure (i.e., violations).
    ///
    /// You can assume this function will only be called if the query was somehow unsuccessfull.
    /// Doesn't mean there are violations necessarily; it may also be a failed boolean query, for
    /// instance.
    ///
    /// # Arguments
    /// - `response`: The [`ResponsePhrases`] returned by the reasoner.
    ///
    /// # Returns
    /// A [`ReasonHandler::Reason`] that describes why the policy was not compliant.
    ///
    /// # Errors
    /// This function may fail if the input `response` was in an invalid state.
    fn extract_reasons(&self, response: &ResponsePhrases) -> Result<Self::Reason, Self::Error>;
}



/// An eFLINT [`ReasonHandler`] that does not communicate failures.
#[derive(Clone, Debug)]
pub struct EFlintSilentReasonHandler;
impl ReasonHandler for EFlintSilentReasonHandler {
    type Error = Infallible;
    type Reason = NoReason;

    #[inline]
    fn extract_reasons(&self, _response: &ResponsePhrases) -> Result<Self::Reason, Self::Error> { Ok(NoReason) }
}

/// An eFLINT [`ReasonHandler`] that only communicates violations who's name starts with some prefix.
#[derive(Clone, Debug)]
pub struct EFlintPrefixedReasonHandler {
    /// The prefix to use to filter violations.
    pub prefix: String,
}
impl EFlintPrefixedReasonHandler {
    /// Constructor for the EFlintPrefixedReasonHandler.
    ///
    /// # Arguments
    /// - `prefix`: The prefix to use to filter violations.
    ///
    /// # Returns
    /// A new instance of Self, ready for handling.
    #[inline]
    pub fn new(prefix: impl Into<String>) -> Self { Self { prefix: prefix.into() } }
}
impl ReasonHandler for EFlintPrefixedReasonHandler {
    type Error = Infallible;
    type Reason = ManyReason<String>;

    #[inline]
    fn extract_reasons(&self, response: &ResponsePhrases) -> Result<Self::Reason, Self::Error> {
        Ok(response
            .results
            .last()
            .map(|r| match r {
                eflint_json::spec::PhraseResult::StateChange(sc) => match &sc.violations {
                    Some(v) => v.iter().filter(|v| v.identifier.starts_with(&self.prefix)).map(|v| v.identifier.clone()).collect(),
                    None => ManyReason::new(),
                },
                _ => ManyReason::new(),
            })
            .unwrap_or_default())
    }
}
