//  LOGGER.rs
//    by Lut99
//
//  Created:
//    10 Oct 2024, 14:16:24
//  Last edited:
//    02 Dec 2024, 14:21:52
//  Auto updated?
//    Yes
//
//  Description:
//!   Defines the actual [`AuditLogger`] itself.
//

use std::borrow::Cow;
use std::fmt::{Debug, Display};
use std::path::PathBuf;

use enum_debug::EnumDebug as _;
use serde::Serialize;
use serde_json::Value;
use spec::auditlogger::AuditLogger;
use spec::reasonerconn::{ReasonerContext, ReasonerResponse};
use tokio::fs::{File, OpenOptions};
use tokio::io::AsyncWriteExt as _;
use tracing::debug;

use crate::stmt::LogStatement;


/***** HELPER MACROS *****/
/// Wraps a [`write!`]-macro to return its error as a [`FileLoggerError`].
macro_rules! write_file {
    ($path:expr, $handle:expr, $($t:tt)+) => {
        // Psych we actually don't wrap that macro, since we're doing async ofc
        async {
            use tokio::io::AsyncWriteExt as _;
            let contents: String = format!($($t)+);
            $handle.write_all(contents.as_bytes()).await.map_err(|source| Error::FileWrite { path: ($path), source })
        }
    };
}

/// Wraps a [`writeln!`]-macro to return its error as a [`FileLoggerError`].
macro_rules! writeln_file {
    ($path:expr, $handle:expr) => {
        // Psych we actually don't wrap that macro, since we're doing async ofc
        async {
            use tokio::io::AsyncWriteExt as _;
            $handle.write_all(b"\n").await.map_err(|source| Error::FileWrite { path: ($path), source })
        }
    };
    ($path:expr, $handle:expr, $($t:tt)+) => {
        // Psych we actually don't wrap that macro, since we're doing async ofc
        async {
            use tokio::io::AsyncWriteExt as _;
            let mut contents: String = format!($($t)*);
            contents.push('\n');
            $handle.write_all(contents.as_bytes()).await.map_err(|source| Error::FileWrite { path: ($path), source })
        }
    };
}





/***** ERRORS *****/
/// Defines the errors emitted by the [`FileLogger`].
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Failed to create a new file.
    #[error("Failed to create a new file at: {}", path.display())]
    FileCreate { path: PathBuf, source: std::io::Error },
    /// Failed to open an existing file.
    #[error("Failed to open existing file: {}", path.display())]
    FileOpen { path: PathBuf, source: std::io::Error },
    /// Failed to shutdown an open file.
    #[error("Failed to shutdown open file: {}", path.display())]
    FileShutdown { path: PathBuf, source: std::io::Error },
    /// Failed to write to a new file.
    #[error("Failed to write to file: {}", path.display())]
    FileWrite { path: PathBuf, source: std::io::Error },
    /// Failed to serialize a logging statement.
    #[error("Failed to serialize statement LogStatement::{kind}")]
    LogStatementSerialize { kind: String, source: serde_json::Error },
}





/***** LIBRARY *****/
/// Implements an [`AuditLogger`] that writes everything to a local file.
#[derive(Clone, Debug)]
pub struct FileLogger {
    /// The identifier of who/what is writing.
    id: String,
    /// The path we log to.
    path: PathBuf,
    /// Whether the user has already printed the context or not.
    #[cfg(debug_assertions)]
    logged_context: std::sync::Arc<std::sync::atomic::AtomicBool>,
}
impl FileLogger {
    /// Constructor for the FileLogger that initializes it pointing to the given file.
    ///
    /// # Arguments
    /// - `identifier`: Some identifier that represents who writes the log statement. E.g., `policy-reasoner v1.2.3`.
    /// - `path`: The path to the file to log to.
    ///
    /// # Returns
    /// A new instance of self, ready for action.
    #[inline]
    pub fn new(id: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self {
            id: id.into(),
            path: path.into(),
            #[cfg(debug_assertions)]
            logged_context: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Writes a log statement to the logging file.
    ///
    /// # Arguments
    /// - `stmt`: The [`LogStatement`] that determines what we're gonna log.
    ///
    /// # Errors
    /// This function errors if we failed to perform the logging completely (i.e., either write or flush).
    async fn log(&self, stmt: LogStatement<'_>) -> Result<(), Error> {
        // Step 1: Open the log file
        let mut handle: File = if !self.path.exists() {
            debug!("Creating new log file at '{}'...", self.path.display());
            File::create(&self.path).await.map_err(|source| Error::FileCreate { path: self.path.clone(), source })?
        } else {
            debug!("Opening existing log file at '{}'...", self.path.display());
            OpenOptions::new()
                .write(true)
                .append(true)
                .open(&self.path)
                .await
                .map_err(|source| Error::FileOpen { path: self.path.clone(), source })?
        };

        // // Navigate to the end of the file
        // let end_pos: u64 = match handle.seek(SeekFrom::End(0)).await {
        //     Ok(pos) => pos,
        //     Err(err) => return Err(FileLoggerError::FileSeek { path: self.path.clone(), err }),
        // };
        // debug!("End of file is after {end_pos} bytes");

        // Write the message
        debug!("Writing {}-statement to logfile...", stmt.variant());
        // Write who wrote it
        write_file!(self.path.clone(), &mut handle, "[{}]", self.id).await?;
        // Print the timestamp
        write_file!(self.path.clone(), &mut handle, "[{}]", chrono::Local::now().format("%Y-%m-%d %H:%M:%S")).await?;
        // Then write the logged message
        let message = serde_json::to_string(&stmt).map_err(|source| Error::LogStatementSerialize { kind: format!("{stmt:?}"), source })?;
        writeln_file!(self.path.clone(), &mut handle, " {message}").await?;

        // Finally flush the file
        debug!("Flushing log file...");
        handle.shutdown().await.map_err(|source| Error::FileShutdown { path: self.path.clone(), source })?;

        drop(handle);

        // Done, a smashing success
        Ok(())
    }
}
impl AuditLogger for FileLogger {
    type Error = Error;

    #[inline]
    async fn log_context<'a, C>(&'a self, context: &'a C) -> Result<(), Self::Error>
    where
        C: ?Sized + Sync + ReasonerContext,
    {
        // Serialize the context first
        let context: Value =
            serde_json::to_value(context).map_err(|source| Error::LogStatementSerialize { kind: "LogStatement::Context".into(), source })?;

        // Log it
        self.log(LogStatement::Context { context }).await?;
        #[cfg(debug_assertions)]
        self.logged_context.store(true, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    #[inline]
    async fn log_response<'a, R>(&'a self, reference: &'a str, response: &'a ReasonerResponse<R>, raw: Option<&'a str>) -> Result<(), Self::Error>
    where
        R: Sync + Display,
    {
        #[cfg(debug_assertions)]
        if !self.logged_context.load(std::sync::atomic::Ordering::Relaxed) {
            tracing::warn!("Logging reasoner response without having logged the reasoner context; please call FileLogger::log_context() first.");
        }

        // Serialize the response first
        let response: Value = serde_json::to_value(&match response {
            ReasonerResponse::Success => ReasonerResponse::Success,
            ReasonerResponse::Violated(reasons) => ReasonerResponse::Violated(reasons.to_string()),
        })
        .map_err(|source| Error::LogStatementSerialize { kind: "LogStatement::ReasonerResponse".into(), source })?;

        // Log it
        self.log(LogStatement::ReasonerResponse { reference: Cow::Borrowed(reference), response, raw: raw.map(Cow::Borrowed) }).await
    }

    #[inline]
    async fn log_question<'a, S, Q>(&'a self, reference: &'a str, state: &'a S, question: &'a Q) -> Result<(), Self::Error>
    where
        S: Sync + Serialize,
        Q: Sync + Serialize,
    {
        #[cfg(debug_assertions)]
        if !self.logged_context.load(std::sync::atomic::Ordering::Relaxed) {
            tracing::warn!("Logging reasoner response without having logged the reasoner context; please call FileLogger::log_context() first.");
        }

        // Serialize the state & question first
        let state: Value =
            serde_json::to_value(state).map_err(|source| Error::LogStatementSerialize { kind: "LogStatement::ReasonerConsult".into(), source })?;
        let question: Value =
            serde_json::to_value(question).map_err(|source| Error::LogStatementSerialize { kind: "LogStatement::ReasonerConsult".into(), source })?;

        // Log it
        self.log(LogStatement::ReasonerConsult { reference: Cow::Borrowed(reference), state, question }).await
    }
}
