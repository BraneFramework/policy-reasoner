//  LOGGER.rs
//    by Lut99
//
//  Created:
//    10 Oct 2024, 14:46:33
//  Last edited:
//    02 Dec 2024, 14:21:38
//  Auto updated?
//    Yes
//
//  Description:
//!   Defines the actual [`AuditLogger`] itself.
//

use std::convert::Infallible;
use std::fmt::Display;

use serde::Serialize;
use spec::auditlogger::AuditLogger;
use spec::reasonerconn::{ReasonerContext, ReasonerResponse};


/***** LIBRARY *****/
/// Implements an [`AuditLogger`] that doesn't log anything.
#[derive(Clone, Copy, Debug)]
pub struct MockLogger;
impl Default for MockLogger {
    #[inline]
    fn default() -> Self { Self::new() }
}
impl MockLogger {
    /// Constructor for the MockLogger that initializes it.
    /// # Returns
    /// A new instance of self, ready for action.
    #[inline]
    pub const fn new() -> Self { Self }
}
impl AuditLogger for MockLogger {
    type Error = Infallible;

    #[inline]
    async fn log_context<'a, C>(&'a self, _context: &'a C) -> Result<(), Self::Error>
    where
        C: ?Sized + ReasonerContext,
    {
        println!("AUDIT LOG: log_context");
        Ok(())
    }

    #[inline]
    async fn log_response<'a, R>(&'a self, _reference: &'a str, _response: &'a ReasonerResponse<R>, _raw: Option<&'a str>) -> Result<(), Self::Error>
    where
        R: Display,
    {
        println!("AUDIT LOG: log_response");
        Ok(())
    }

    #[inline]
    async fn log_question<'a, S, Q>(&'a self, _reference: &'a str, _state: &'a S, _question: &'a Q) -> Result<(), Self::Error>
    where
        S: Serialize,
        Q: Serialize,
    {
        println!("AUDIT LOG: log_question");
        Ok(())
    }
}
