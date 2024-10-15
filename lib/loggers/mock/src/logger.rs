//  LOGGER.rs
//    by Lut99
//
//  Created:
//    10 Oct 2024, 14:46:33
//  Last edited:
//    10 Oct 2024, 14:47:56
//  Auto updated?
//    Yes
//
//  Description:
//!   Defines the actual [`AuditLogger`] itself.
//

use std::convert::Infallible;
use std::fmt::Display;
use std::future::Future;

use spec::auditlogger::AuditLogger;
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
    fn log_response<'a, R>(
        &'a self,
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
}
