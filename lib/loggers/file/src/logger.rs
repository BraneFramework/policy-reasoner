//  LOGGER.rs
//    by Lut99
//
//  Created:
//    10 Oct 2024, 14:16:24
//  Last edited:
//    10 Oct 2024, 14:40:18
//  Auto updated?
//    Yes
//
//  Description:
//!   Defines the actual [`AuditLogger`] itself.
//

use std::borrow::Cow;
use std::error;
use std::fmt::{Debug, Display, Formatter, Result as FResult};
use std::future::Future;
use std::path::PathBuf;

use enum_debug::EnumDebug as _;
use serde::Serialize;
use spec::auditlogger::AuditLogger;
use spec::reasonerconn::ReasonerResponse;
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
            $handle.write_all(contents.as_bytes()).await.map_err(|err| Error::FileWrite { path: ($path), err })
        }
    };
}

/// Wraps a [`writeln!`]-macro to return its error as a [`FileLoggerError`].
macro_rules! writeln_file {
    ($path:expr, $handle:expr) => {
        // Psych we actually don't wrap that macro, since we're doing async ofc
        async {
            use tokio::io::AsyncWriteExt as _;
            $handle.write_all(b"\n").await.map_err(|err| Error::FileWrite { path: ($path), err })
        }
    };
    ($path:expr, $handle:expr, $($t:tt)+) => {
        // Psych we actually don't wrap that macro, since we're doing async ofc
        async {
            use tokio::io::AsyncWriteExt as _;
            let mut contents: String = format!($($t)*);
            contents.push('\n');
            $handle.write_all(contents.as_bytes()).await.map_err(|err| Error::FileWrite { path: ($path), err })
        }
    };
}





/***** ERRORS *****/
/// Defines the errors emitted by the [`FileLogger`].
#[derive(Debug)]
pub enum Error {
    /// Failed to create a new file.
    FileCreate { path: PathBuf, err: std::io::Error },
    /// Failed to open an existing file.
    FileOpen { path: PathBuf, err: std::io::Error },
    /// Failed to shutdown an open file.
    FileShutdown { path: PathBuf, err: std::io::Error },
    /// Failed to write to a new file.
    FileWrite { path: PathBuf, err: std::io::Error },
    /// Failed to serialize a logging statement.
    LogStatementSerialize { kind: String, err: serde_json::Error },
}
impl Display for Error {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        use Error::*;
        match self {
            FileCreate { path, .. } => write!(f, "Failed to create a new file at {:?}", path.display()),
            FileOpen { path, .. } => write!(f, "Failed to open existing file {:?}", path.display()),
            FileShutdown { path, .. } => write!(f, "Failed to shutdown open file {:?}", path.display()),
            FileWrite { path, .. } => write!(f, "Failed to write to flie {:?}", path.display()),
            LogStatementSerialize { kind, .. } => write!(f, "Failed to serialize statement LogStatement::{kind}"),
        }
    }
}
impl error::Error for Error {
    #[inline]
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        use Error::*;
        match self {
            FileCreate { err, .. } => Some(err),
            FileOpen { err, .. } => Some(err),
            FileShutdown { err, .. } => Some(err),
            FileWrite { err, .. } => Some(err),
            LogStatementSerialize { err, .. } => Some(err),
        }
    }
}





/***** LIBRARY *****/
/// Implements an [`AuditLogger`] that writes everything to a local file.
#[derive(Clone, Debug)]
pub struct FileLogger {
    /// The identifier of who/what is writing.
    id:   String,
    /// The path we log to.
    path: PathBuf,
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
    pub fn new(id: impl Into<String>, path: impl Into<PathBuf>) -> Self { Self { id: id.into(), path: path.into() } }

    /// Writes a log statement to the logging file.
    ///
    /// # Arguments
    /// - `stmt`: The [`LogStatement`] that determines what we're gonna log.
    ///
    /// # Errors
    /// This function errors if we failed to perform the logging completely (i.e., either write or flush).
    async fn log<T: Clone + Debug + Serialize>(&self, stmt: LogStatement<'_, T>) -> Result<(), Error> {
        // Step 1: Open the log file
        let mut handle: File = if !self.path.exists() {
            debug!("Creating new log file at '{}'...", self.path.display());
            match File::create(&self.path).await {
                Ok(handle) => handle,
                Err(err) => return Err(Error::FileCreate { path: self.path.clone(), err }),
            }
        } else {
            debug!("Opening existing log file at '{}'...", self.path.display());
            match OpenOptions::new().write(true).append(true).open(&self.path).await {
                Ok(handle) => handle,
                Err(err) => return Err(Error::FileOpen { path: self.path.clone(), err }),
            }
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
        match serde_json::to_string(&stmt) {
            Ok(message) => writeln_file!(self.path.clone(), &mut handle, " {message}").await?,
            Err(err) => return Err(Error::LogStatementSerialize { kind: format!("{stmt:?}"), err }),
        }

        // Finally flush the file
        debug!("Flushing log file...");
        if let Err(err) = handle.shutdown().await {
            return Err(Error::FileShutdown { path: self.path.clone(), err });
        }
        drop(handle);

        // Done, a smashing success
        Ok(())
    }
}
impl AuditLogger for FileLogger {
    type Error = Error;

    #[inline]
    fn log_response<'a, R>(
        &'a self,
        reference: &'a str,
        response: &'a ReasonerResponse<R>,
        raw: Option<&'a str>,
    ) -> impl 'a + Future<Output = Result<(), Self::Error>>
    where
        R: Display,
    {
        async move {
            // Serialize the response first
            let res: ReasonerResponse<String> = match response {
                ReasonerResponse::Success => ReasonerResponse::Success,
                ReasonerResponse::Violated(r) => ReasonerResponse::Violated(r.to_string()),
            };

            // Log it
            self.log(LogStatement::ReasonerResponse {
                reference: Cow::Borrowed(reference),
                response: Cow::Borrowed(&res),
                raw: raw.map(Cow::Borrowed),
            })
            .await
        }
    }
}
