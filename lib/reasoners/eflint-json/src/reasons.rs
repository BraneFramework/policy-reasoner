//  REASONS.rs
//    by Lut99
//
//  Created:
//    09 Oct 2024, 16:37:52
//  Last edited:
//    17 Oct 2024, 09:55:31
//  Auto updated?
//    Yes
//
//  Description:
//!   Defines how the eFLINT reasoner deals with reasons for failure.
//

use std::convert::Infallible;
use std::error::Error;
use std::fmt::Debug;

use eflint_json::spec::ResponsePhrases;
use spec::reasons::{ManyReason, NoReason};


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
