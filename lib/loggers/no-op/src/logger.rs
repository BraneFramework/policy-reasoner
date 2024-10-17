//  LOGGER.rs
//    by Lut99
//
//  Created:
//    10 Oct 2024, 14:46:33
//  Last edited:
//    17 Oct 2024, 13:14:34
//  Auto updated?
//    Yes
//
//  Description:
//!   Defines the actual [`AuditLogger`] itself.
//

use std::convert::Infallible;
use std::fmt::Display;
use std::future::Future;

use serde::Serialize;
use spec::auditlogger::AuditLogger;
use spec::context::Context;
use spec::reasonerconn::ReasonerResponse;


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
    fn log_context<'a, C>(&'a mut self, _context: &'a C) -> impl 'a + Future<Output = Result<(), Self::Error>>
    where
        C: ?Sized + Context,
    {
        async move {
            println!("AUDIT LOG: log_context");
            Ok(())
        }
    }

    #[inline]
    fn log_response<'a, R>(
        &'a mut self,
        _reference: &'a str,
        _response: &'a ReasonerResponse<R>,
        _raw: Option<&'a str>,
    ) -> impl 'a + Future<Output = Result<(), Self::Error>>
    where
        R: Display,
    {
        async move {
            println!("AUDIT LOG: log_response");
            Ok(())
        }
    }

    #[inline]
    fn log_question<'a, S, Q>(
        &'a mut self,
        _reference: &'a str,
        _state: &'a S,
        _question: &'a Q,
    ) -> impl 'a + Future<Output = Result<(), Self::Error>>
    where
        S: Serialize,
        Q: Serialize,
    {
        async move {
            println!("AUDIT LOG: log_question");
            Ok(())
        }
    }
}
