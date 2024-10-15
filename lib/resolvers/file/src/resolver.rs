//  RESOLVER.rs
//    by Lut99
//
//  Created:
//    10 Oct 2024, 15:55:23
//  Last edited:
//    10 Oct 2024, 16:10:46
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
impl<R: for<'de> Deserialize<'de>> StateResolver for FileResolver<R> {
    type Error = Error;
    type Resolved = R;
    type State = ();

    fn resolve<L>(&self, _state: Self::State, logger: &SessionedAuditLogger<L>) -> impl Future<Output = Result<Self::Resolved, Self::Error>>
    where
        L: AuditLogger,
    {
        async move {
            // NOTE: Using `#[instrument]` adds some unnecessary trait bounds on `S` and such.
            let _span = span!(Level::INFO, "FileResolver::resolve", reference = logger.reference()).entered();

            // Read the file in one go// Read the file in one go
            debug!("Opening input file '{}'...", self.path.display());
            let state: String = match fs::read_to_string(&self.path).await {
                Ok(state) => state,
                Err(err) => return Err(Error::FileRead { path: self.path.clone(), err }),
            };

            // Parse it as JSON
            debug!("Parsing input file '{}'...", self.path.display());
            match serde_json::from_str(&state) {
                Ok(state) => Ok(state),
                Err(err) => Err(Error::FileDeserialize { to: std::any::type_name::<R>(), path: self.path.clone(), err }),
            }
        }
    }
}
