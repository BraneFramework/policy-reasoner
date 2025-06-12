//  RESOLVER.rs
//    by Lut99
//
//  Created:
//    10 Oct 2024, 15:55:23
//  Last edited:
//    05 Nov 2024, 11:15:26
//  Auto updated?
//    Yes
//
//  Description:
//!   Defines the actual [`StateResolver`].
//

use std::marker::PhantomData;
use std::path::PathBuf;

use serde::Deserialize;
use spec::AuditLogger;
use spec::auditlogger::SessionedAuditLogger;
use spec::stateresolver::StateResolver;
use tokio::fs;
use tracing::{debug, instrument};


/***** ERRORS *****/
/// Defines the errors that are occurring in the [`FileResolver`].
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Failed to deserialize the target file's contents.
    #[error("Failed to deserialize contents of file {} as {to}", path.display())]
    FileDeserialize { to: &'static str, path: PathBuf, source: serde_json::Error },
    /// Failed to read the target file.
    #[error("Failed to read file {}", path.display())]
    FileRead { path: PathBuf, source: std::io::Error },
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

    #[instrument(name = "FileResolver::resolve", skip_all, fields(reference=logger.reference()))]
    async fn resolve<'a, L>(&'a self, _state: Self::State, logger: &'a SessionedAuditLogger<L>) -> Result<Self::Resolved, Self::Error>
    where
        L: Sync + AuditLogger,
    {
        // Read the file in one go// Read the file in one go
        debug!("Opening input file '{}'...", self.path.display());
        let state_str: String = fs::read_to_string(&self.path).await.map_err(|source| Error::FileRead { path: self.path.clone(), source })?;

        // Parse it as JSON
        debug!("Parsing input file '{}'...", self.path.display());
        let state = serde_json::from_str(&state_str).map_err(|source| Error::FileDeserialize {
            to: std::any::type_name::<R>(),
            path: self.path.clone(),
            source,
        })?;

        Ok(state)
    }
}
