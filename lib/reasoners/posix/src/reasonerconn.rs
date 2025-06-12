//  REASONERCONN.rs
//    by Lut99
//
//  Created:
//    11 Oct 2024, 16:54:51
//  Last edited:
//    06 May 2025, 12:52:13
//  Auto updated?
//    Yes
//
//  Description:
//!   Implements the actual [`ReasonerConnector`].
//


use std::borrow::Cow;
/***** LIBRARY *****/
use std::iter::repeat;
use std::ops::BitOr;
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};

use error_trace::{ErrorTrace as _, Trace};
use serde::{Deserialize, Serialize};
use spec::auditlogger::{AuditLogger, SessionedAuditLogger};
use spec::reasonerconn::{ReasonerConnector, ReasonerContext, ReasonerResponse};
use spec::reasons::NoReason;
use thiserror::Error;
use tokio::fs;
use tracing::{debug, info};
use workflow::Workflow;

use crate::config::{Config, DataPolicy, PosixLocalIdentity};
use crate::workflow::WorkflowDatasets;


/***** ERRORS *****/
/// Represents an error that occurs during validation of the policy.
#[derive(Debug, Error)]
pub enum Error {
    /// Failed to retrieve a file's metadata.
    #[error("Failed to get file {:?} metadata", path.display())]
    FileMetadata { path: PathBuf, source: std::io::Error },
    /// Failed to log the context of the reasoner.
    #[error("Failed to log the reasoner's context to {to}")]
    LogContext { to: &'static str, source: Trace },
    /// Failed to log the reasoner's response to the given logger.
    #[error("Failed to log the reasoner's response to {to}")]
    LogResponse { to: &'static str, source: Trace },
    /// Failed to log the question to the given logger.
    #[error("Failed to log the question to {to}")]
    LogQuestion { to: &'static str, source: Trace },
    /// The dataset was unknown to us.
    #[error("Unknown dataset {data:?}")]
    UnknownDataset { data: String },
}





/***** HELPER FUNCTIONS *****/
/// Verifies whether the passed [`PosixLocalIdentity`] has all of the requested permissions (e.g., `Read` and `Write`)
/// on a particular file (defined by the `path`). The identity's user id and group ids are checked against the file
/// owner's user id and group id respectively. Additionally, the `Others` class permissions are also checked.
async fn satisfies_posix_permissions(
    path: impl AsRef<Path>,
    local_identity: Option<&PosixLocalIdentity>,
    requested_permissions: PosixFilePermissions,
) -> Result<bool, Error> {
    #[inline]
    const fn is_user_owner(owner_id: u32, local_identity: Option<&PosixLocalIdentity>) -> bool {
        if let Some(id) = local_identity { owner_id == id.uid } else { false }
    }
    #[inline]
    fn is_group_owner(group_id: u32, local_identity: Option<&PosixLocalIdentity>) -> bool {
        if let Some(id) = local_identity { id.gids.contains(&group_id) } else { false }
    }

    let path: &Path = path.as_ref();
    let metadata = fs::metadata(path).await.map_err(|source| Error::FileMetadata { path: path.into(), source })?;

    // First, get the appropriate UIDs from the file
    let mode_bits = metadata.permissions().mode();
    let file_owner_uid = metadata.uid();
    let file_owner_gid = metadata.gid();
    debug!("Checking if user {local_identity:?} is owner of file with UID={file_owner_uid},GID={file_owner_gid}");

    // Then decide which permissions to base ourselves on
    let mask: u32 = if is_user_owner(file_owner_uid, local_identity) {
        // If the user owns the file, then we exclusively assume user permissions
        let mask: u32 = PosixFileClass::Owner.get_mode_bitmask(requested_permissions);
        debug!("Using OWNER permissions ({mask:o})");
        mask
    } else if is_group_owner(file_owner_gid, local_identity) {
        // If the user has a group owning the file, then we exclusively assume group permissions
        let mask: u32 = PosixFileClass::Group.get_mode_bitmask(requested_permissions);
        debug!("Using GROUP permissions ({mask:o})");
        mask
    } else {
        // In any other scenario (including user unknown), we assume other permissions
        let mask: u32 = PosixFileClass::Others.get_mode_bitmask(requested_permissions);
        debug!("Using OTHER permissions ({mask:o})");
        mask
    };

    // Finally, check if the permissions align
    Ok(mode_bits & mask == mask)
}





/***** HELPERS *****/
/// Represents a POSIX file class, also known as a scope. See:
/// <https://en.wikipedia.org/wiki/File-system_permissions#Classes>.
#[derive(Copy, Clone, Deserialize)]
enum PosixFileClass {
    Owner,
    Group,
    Others,
}
impl PosixFileClass {
    /// Given a list of [`PosixFilePermission`]s will return an octal mode bitmask for this [`PosixFileClass`].
    ///
    /// This bitmask represents what mode bits should be set on a file such that this class (e.g., `Owner`) satisfies
    /// the permissions (e.g, `Read`, `Write`). In this case it would be `0o400` (Read for Owner) and `0o200` (Write for
    /// Owner), which sums to the returned `0o600` (Read and Write for Owner).
    fn get_mode_bitmask(&self, required_permissions: PosixFilePermissions) -> u32 {
        let alignment_multiplier = match self {
            PosixFileClass::Owner => 0o100,
            PosixFileClass::Group => 0o10,
            PosixFileClass::Others => 0o1,
        };
        alignment_multiplier * (required_permissions.as_u8() as u32)
    }
}

/// Represents a set of file permissions.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct PosixFilePermissions(u8);
impl PosixFilePermissions {
    /// Returns the raw bit pattern for this permission set.
    #[inline]
    const fn as_u8(&self) -> u8 { self.0 }
}

/// Represents a POSIX file permission. See: <https://en.wikipedia.org/wiki/File-system_permissions#Permissions>.
#[derive(Debug, Copy, Clone)]
enum PosixFilePermission {
    Read,
    Write,
    Execute,
}
impl PosixFilePermission {
    /// Returns this permission's mode bit.
    /// - `Read` → `4`
    /// - `Write` → `2`
    /// - `Execute` → `1`.
    ///
    /// For more about POSIX permission bits see:
    /// <https://en.wikipedia.org/wiki/File-system_permissions#Numeric_notation>.
    ///
    /// Also see the related [`UserType::get_mode_bitmask`].
    const fn to_mode_bit(self) -> u32 {
        match self {
            PosixFilePermission::Read => 4,
            PosixFilePermission::Write => 2,
            PosixFilePermission::Execute => 1,
        }
    }

    /// Returns the permission as a set of permissions.
    ///
    /// # Returns
    /// A [`PosixFilePermissions`] representing the ste.
    #[inline]
    fn to_set(self) -> PosixFilePermissions { PosixFilePermissions(self.to_mode_bit() as u8) }
}
impl BitOr<Self> for PosixFilePermission {
    type Output = PosixFilePermissions;

    #[inline]
    fn bitor(self, rhs: PosixFilePermission) -> Self::Output { PosixFilePermissions((self.to_mode_bit() | rhs.to_mode_bit()) as u8) }
}





/***** AUXILLARY *****/
/// The [`ReasonerContext`] returned by the [`PosixReasonerConnector`].
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PosixReasonerContext {
    /// The version of this reasoner.
    pub version: String,
    /// The language identifier of this reasoner.
    pub language: String,
    /// The language's version identifier of this reasoner.
    pub language_version: String,
}
impl Default for PosixReasonerContext {
    #[inline]
    fn default() -> Self { Self { version: env!("CARGO_PKG_VERSION").into(), language: "posix".into(), language_version: "0.2.0".into() } }
}
impl ReasonerContext for PosixReasonerContext {
    #[inline]
    fn version(&self) -> Cow<str> { Cow::Borrowed(&self.version) }

    #[inline]
    fn language(&self) -> Cow<str> { Cow::Borrowed(&self.language) }

    #[inline]
    fn language_version(&self) -> Cow<str> { Cow::Borrowed(&self.language_version) }
}

/// The overarching input to the POSIX reasoner.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct State {
    /// The policy to give.
    pub config:   Config,
    /// The workflow considered.
    pub workflow: Workflow,
}





/***** LIBRARY *****/
/// The POSIX reasoner connector. This connector is used to validate workflows based on POSIX file permissions.
pub struct PosixReasonerConnector;
impl PosixReasonerConnector {
    /// Constructor for the PosixReasonerConnector.
    ///
    /// This constructor logs asynchronously.
    ///
    /// # Arguments
    /// - `logger`: A logger to write this reasoner's context to.
    ///
    /// # Errors
    /// This function may error if it failed to log to the given `logger`.
    #[inline]
    pub async fn new_async<L: AuditLogger>(logger: &mut L) -> Result<Self, Error> {
        logger
            .log_context(&PosixReasonerContext::default())
            .await
            .map_err(|err| Error::LogContext { to: std::any::type_name::<L>(), source: err.freeze() })?;
        Ok(Self)
    }
}
impl ReasonerConnector for PosixReasonerConnector {
    type Context = PosixReasonerContext;
    type Error = Error;
    type Question = ();
    type Reason = NoReason;
    type State = State;

    #[inline]
    fn context(&self) -> Self::Context { PosixReasonerContext::default() }

    #[inline]
    async fn consult<'a, L>(
        &'a self,
        state: Self::State,
        _question: Self::Question,
        logger: &'a SessionedAuditLogger<L>,
    ) -> Result<ReasonerResponse<Self::Reason>, Self::Error>
    where
        L: Sync + AuditLogger,
    {
        // Log the input
        logger
            .log_question(&state, &())
            .await
            .map_err(|err| Error::LogQuestion { to: std::any::type_name::<SessionedAuditLogger<L>>(), source: err.freeze() })?;

        // The datasets used in the workflow. E.g., `st_antonius_ect`.
        let datasets: WorkflowDatasets = WorkflowDatasets::new(&state.config.id, &state.workflow);
        debug!("Found datasets in workflow {:?}: {:#?}", state.workflow.id, datasets);

        // Loop to find the permissions on the disk
        for ((location, dataset), permission) in std::iter::empty()
            .chain(datasets.read_sets.iter().zip(repeat(PosixFilePermission::Read.to_set())))
            .chain(datasets.write_sets.iter().zip(repeat(PosixFilePermission::Write.to_set())))
            .chain(datasets.execute_sets.iter().zip(repeat(PosixFilePermission::Read | PosixFilePermission::Execute)))
        {
            info!("Testing dataset {:?} for permission to {:?} for user {:?}", dataset.id, permission, location);

            // Find the location of the dataset in the list
            let policy: &DataPolicy = match state.config.data.get(&dataset.id) {
                Some(data) => data,
                None => return Err(Error::UnknownDataset { data: dataset.id.clone() }),
            };

            // Now check the policy!
            if !satisfies_posix_permissions(&policy.path, policy.user_map.get(&location.id), permission).await? {
                logger
                    .log_response(&ReasonerResponse::Violated(NoReason), Some("false"))
                    .await
                    .map_err(|err| Error::LogResponse { to: std::any::type_name::<SessionedAuditLogger<L>>(), source: err.freeze() })?;
                return Ok(ReasonerResponse::Violated(NoReason));
            }
        }

        // If none of them failed prematurely, then we're done
        logger
            .log_response(&ReasonerResponse::<NoReason>::Success, Some("true"))
            .await
            .map_err(|err| Error::LogResponse { to: std::any::type_name::<SessionedAuditLogger<L>>(), source: err.freeze() })?;
        Ok(ReasonerResponse::Success)
    }
}
