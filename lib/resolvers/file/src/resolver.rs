//  RESOLVER.rs
//    by Lut99
//
//  Created:
//    10 Oct 2024, 15:55:23
//  Last edited:
//    05 Nov 2024, 11:02:05
//  Auto updated?
//    Yes
//
//  Description:
//!   Defines the actual [`StateResolver`].
//

use std::error;
use std::fmt::{Display, Formatter, Result as FResult};
use std::future::Future;
use std::marker::PhantomData;
use std::path::PathBuf;

use serde::Deserialize;
use spec::AuditLogger;
use spec::auditlogger::SessionedAuditLogger;
use spec::stateresolver::StateResolver;
use tokio::fs;
use tracing::{Level, debug, span};


/***** ERRORS *****/
/// Defines the errors that are occurring in the [`FileResolver`].
#[derive(Debug)]
pub enum Error {
    /// Failed to deserialize the target file's contents.
    FileDeserialize { to: &'static str, path: PathBuf, err: serde_json::Error },
    /// Failed to read the target file.
    FileRead { path: PathBuf, err: std::io::Error },
}
impl Display for Error {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        use Error::*;
        match self {
            FileDeserialize { to, path, .. } => write!(f, "Failed to deserialize contents of file {:?} as {}", path.display(), to),
            FileRead { path, .. } => write!(f, "Failed to read file {:?}", path.display()),
        }
    }
}
impl error::Error for Error {
    #[inline]
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        use Error::*;
        match self {
            FileDeserialize { err, .. } => Some(err),
            FileRead { err, .. } => Some(err),
        }
    }
}





/***** LIBRARY *****/
/// Defines a [`StateResolver`] that resolves a [`serde`]-[`Deserialize`]able state from an
/// arbitrary file.
#[derive(Clone, Debug)]
pub struct FileResolver<R> {
    /// The file to resolve from.
    path:      PathBuf,
    /// Remembers what we're resolving to.
    _resolved: PhantomData<R>,
}
impl<R> FileResolver<R> {
    /// Constructor for the FileResolver.
    ///
    /// # Arguments
    /// - `path`: The path to the file that we're resolving from.
    ///
    /// # Returns
    /// A new FileResolver ready for resolution.
    #[inline]
    pub fn new(path: impl Into<PathBuf>) -> Self { Self { path: path.into(), _resolved: PhantomData } }
}
impl<R: Sync + for<'de> Deserialize<'de>> StateResolver for FileResolver<R> {
    type Error = Error;
    type Resolved = R;
    type State = ();

    fn resolve<'a, L>(
        &'a self,
        _state: Self::State,
        logger: &'a SessionedAuditLogger<L>,
    ) -> impl 'a + Send + Future<Output = Result<Self::Resolved, Self::Error>>
    where
        L: Sync + AuditLogger,
    {
        async move {
            // NOTE: Using `#[instrument]` adds some unnecessary trait bounds on `S` and such.
            // NOTE: Using `entered()` carries the scope across await points, which isn't correct.
            //       As we know `fs::read_to_string()` and `serde_json::from_str()` won't call
            //       tracing themselves, we only use the guard on the debugs themselves.
            let span = span!(Level::INFO, "FileResolver::resolve", reference = logger.reference());

            // Read the file in one go// Read the file in one go
            span.in_scope(|| debug!("Opening input file '{}'...", self.path.display()));
            let state: String = match fs::read_to_string(&self.path).await {
                Ok(state) => state,
                Err(err) => return Err(Error::FileRead { path: self.path.clone(), err }),
            };

            // Parse it as JSON
            span.in_scope(|| debug!("Parsing input file '{}'...", self.path.display()));
            match serde_json::from_str(&state) {
                Ok(state) => Ok(state),
                Err(err) => Err(Error::FileDeserialize { to: std::any::type_name::<R>(), path: self.path.clone(), err }),
            }
        }
    }
}
