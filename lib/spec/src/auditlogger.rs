//  AUDITLOGGER.rs
//    by Lut99
//
//  Created:
//    09 Oct 2024, 13:38:41
//  Last edited:
//    05 Nov 2024, 11:13:21
//  Auto updated?
//    Yes
//
//  Description:
//!   Defines the interface between the reasoner and some component
//!   creating audit trails.
//

use std::error::Error;
use std::fmt::Display;
use std::future::Future;

use serde::Serialize;

use crate::context::Context;
use crate::reasonerconn::ReasonerResponse;


/***** AUXILLARY *****/
/// Defines a wrapper around any [`AuditLogger`] that extends it with some kind of [`tracing`]-like
/// session information.
#[derive(Clone, Debug)]
pub struct SessionedAuditLogger<L> {
    /// The reference used to recognize the logs based on user input.
    reference: String,
    /// The nested logger
    logger:    L,
}
impl<L> SessionedAuditLogger<L> {
    /// Constructor for the SessionedAuditLogger.
    ///
    /// # Arguments
    /// - `reference`: The reference used to recognize the logs based on user input.
    /// - `logger`: The nested logger.
    ///
    /// # Returns
    /// A new instance of Self, ready for logging.
    #[inline]
    pub fn new(reference: impl Into<String>, logger: L) -> Self { Self { reference: reference.into(), logger } }

    /// Provides read-only access to the internal reference.
    #[inline]
    pub fn reference(&self) -> &str { &self.reference }
}
impl<L: AuditLogger> SessionedAuditLogger<L> {
    /// Alias for [`AuditLogger::log_response()`] but using the internal response instead of the
    /// given one.
    ///
    /// # Arguments
    /// - `response`: The [`ReasonerResponse`] that we're logging.
    /// - `raw`: The raw response produced by the reasoner, if applicable.
    pub fn log_response<'a, R>(
        &'a self,
        response: &'a ReasonerResponse<R>,
        raw: Option<&'a str>,
    ) -> impl 'a + Send + Future<Output = Result<(), <Self as AuditLogger>::Error>>
    where
        R: Sync + Display,
    {
        L::log_response(&self.logger, &self.reference, response, raw)
    }

    /// Logs that the reasoner is being asked a question.
    ///
    /// # Arguments
    /// - `state`: Some serializable state given as input to the reasoner.
    /// - `question`: Some serializable question that we're asking.
    pub fn log_question<'a, S, Q>(
        &'a self,
        state: &'a S,
        question: &'a Q,
    ) -> impl 'a + Send + Future<Output = Result<(), <Self as AuditLogger>::Error>>
    where
        S: Sync + Serialize,
        Q: Sync + Serialize,
    {
        L::log_question(&self.logger, &self.reference, state, question)
    }
}
impl<L: AuditLogger> AuditLogger for SessionedAuditLogger<L> {
    type Error = L::Error;

    fn log_context<'a, C>(&'a self, context: &'a C) -> impl 'a + Send + Future<Output = Result<(), Self::Error>>
    where
        C: ?Sized + Sync + Context,
    {
        L::log_context(&self.logger, context)
    }

    fn log_response<'a, R>(
        &'a self,
        reference: &'a str,
        response: &'a ReasonerResponse<R>,
        raw: Option<&'a str>,
    ) -> impl 'a + Send + Future<Output = Result<(), Self::Error>>
    where
        R: Sync + Display,
    {
        L::log_response(&self.logger, reference, response, raw)
    }

    fn log_question<'a, S, Q>(
        &'a self,
        reference: &'a str,
        state: &'a S,
        question: &'a Q,
    ) -> impl 'a + Send + Future<Output = Result<(), Self::Error>>
    where
        S: Sync + Serialize,
        Q: Sync + Serialize,
    {
        L::log_question(&self.logger, reference, state, question)
    }
}





/***** LIBRARY *****/
/// Defines a generic interface to write to an audit trail.
///
/// Note that this logger may be used across threads. As such, any mutability must be inferior.
pub trait AuditLogger {
    /// Defines the errors returned by this logger.
    type Error: Error;


    /// Logs the context of a reasoner at startup.
    ///
    /// # Arguments
    /// - `context`: Something [`Serialize`]able that we want to write at startup.
    fn log_context<'a, C>(&'a self, context: &'a C) -> impl 'a + Send + Future<Output = Result<(), Self::Error>>
    where
        C: ?Sized + Sync + Context;

    /// Log the response of a reasoner.
    ///
    /// # Arguments
    /// - `reference`: Some reference that links the response to a particular query.
    /// - `response`: The [`ReasonerResponse`] that we're logging.
    /// - `raw`: The raw response produced by the reasoner, if applicable.
    fn log_response<'a, R>(
        &'a self,
        reference: &'a str,
        response: &'a ReasonerResponse<R>,
        raw: Option<&'a str>,
    ) -> impl 'a + Send + Future<Output = Result<(), Self::Error>>
    where
        R: Sync + Display;

    /// Logs that the reasoner is being asked a question.
    ///
    /// # Arguments
    /// - `reference`: Some reference that links the response to a particular answer.
    /// - `state`: Some serializable state given as input to the reasoner.
    /// - `question`: Some serializable question that we're asking.
    fn log_question<'a, S, Q>(
        &'a self,
        reference: &'a str,
        state: &'a S,
        question: &'a Q,
    ) -> impl 'a + Send + Future<Output = Result<(), Self::Error>>
    where
        S: Sync + Serialize,
        Q: Sync + Serialize;
}

// Standard impls
impl<'a, T: AuditLogger> AuditLogger for &'a T {
    type Error = T::Error;

    #[inline]
    fn log_context<'s, C>(&'s self, context: &'s C) -> impl 's + Send + Future<Output = Result<(), Self::Error>>
    where
        C: ?Sized + Sync + Context,
    {
        <T as AuditLogger>::log_context(self, context)
    }

    #[inline]
    fn log_response<'s, R>(
        &'s self,
        reference: &'s str,
        response: &'s ReasonerResponse<R>,
        raw: Option<&'s str>,
    ) -> impl 's + Send + Future<Output = Result<(), Self::Error>>
    where
        R: Sync + Display,
    {
        <T as AuditLogger>::log_response(self, reference, response, raw)
    }

    #[inline]
    fn log_question<'s, S, Q>(
        &'s self,
        reference: &'s str,
        state: &'s S,
        question: &'s Q,
    ) -> impl 's + Send + Future<Output = Result<(), Self::Error>>
    where
        S: Sync + Serialize,
        Q: Sync + Serialize,
    {
        <T as AuditLogger>::log_question(self, reference, state, question)
    }
}
impl<'a, T: AuditLogger> AuditLogger for &'a mut T {
    type Error = T::Error;

    #[inline]
    fn log_context<'s, C>(&'s self, context: &'s C) -> impl 's + Send + Future<Output = Result<(), Self::Error>>
    where
        C: ?Sized + Sync + Context,
    {
        <T as AuditLogger>::log_context(self, context)
    }

    #[inline]
    fn log_response<'s, R>(
        &'s self,
        reference: &'s str,
        response: &'s ReasonerResponse<R>,
        raw: Option<&'s str>,
    ) -> impl 's + Send + Future<Output = Result<(), Self::Error>>
    where
        R: Sync + Display,
    {
        <T as AuditLogger>::log_response(self, reference, response, raw)
    }

    #[inline]
    fn log_question<'s, S, Q>(
        &'s self,
        reference: &'s str,
        state: &'s S,
        question: &'s Q,
    ) -> impl 's + Send + Future<Output = Result<(), Self::Error>>
    where
        S: Sync + Serialize,
        Q: Sync + Serialize,
    {
        <T as AuditLogger>::log_question(self, reference, state, question)
    }
}
