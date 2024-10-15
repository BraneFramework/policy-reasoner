//  REASONERCONN.rs
//    by Lut99
//
//  Created:
//    11 Oct 2024, 16:54:51
//  Last edited:
//    14 Oct 2024, 11:58:58
//  Auto updated?
//    Yes
//
//  Description:
//!   Implements the actual [`ReasonerConnector`].
//


/***** LIBRARY *****/
use std::collections::HashMap;
use std::convert::Infallible;
use std::future::Future;
use std::iter::repeat;
use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;

use itertools::{Either, Itertools};
use serde::Deserialize;
use spec::auditlogger::{AuditLogger, SessionedAuditLogger};
use spec::reasonerconn::{ReasonerConnector, ReasonerResponse};
use tracing::{info, span, Level};
use workflow::Workflow;

use crate::workflow::WorkflowDatasets;


/***** TYPES *****/
/// E.g., `st_antonius_etc`.
type LocationIdentifier = String;
/// The global username as defined in [`Workflow.user`]. E.g., `test`.
type GlobalUsername = String;





/***** ERRORS *****/
/// Represents an error that occurred during the validation of a policy. These errors contain more information about the
/// problems that occurred during validation.
#[derive(thiserror::Error, Debug)]
enum PolicyError {
    #[error("Missing location: {0}")]
    MissingLocation(String),
    #[error("Missing user: {0} for location: {1}")]
    MissingUser(String, String),
}

/// Represents a validation error that occurred during the validation of a workflow. These errors contain more
/// information about the problems that occurred during validation.
#[derive(thiserror::Error, Debug)]
enum ValidationError {
    #[error("Policy Error: {0}")]
    PolicyError(PolicyError),
    #[error("Unknown dataset: {0}")]
    UnknownDataset(String),
}





/***** HELPER FUNCTIONS *****/
/// Check if all the data accesses performed in the `workflow` are done on behalf of users that have the required
/// permissions. If not all permissions are met, then [`ValidationError`]s are returned. These errors contain more
/// information about the problems that occurred during validation.
fn validate_dataset_permissions(workflow: &Workflow, policy: &PosixPolicy) -> Result<ValidationOutput, Vec<ValidationError>> {
    let _span = span!(Level::INFO, "PosixReasonerConnector::validate_dataset_permissions", workflow = workflow.id).entered();

    // The datasets used in the workflow. E.g., `st_antonius_ect`.
    let datasets: WorkflowDatasets = WorkflowDatasets::from(workflow);

    // Loop to find the permissions on the disk
    let (forbidden, errors): (Vec<_>, Vec<_>) = std::iter::empty()
        .chain(datasets.read_sets.iter().zip(repeat(PosixFilePermission::Read.to_set())))
        .chain(datasets.write_sets.iter().zip(repeat(PosixFilePermission::Write.to_set())))
        .chain(datasets.execute_sets.iter().zip(repeat(PosixFilePermission::Read | PosixFilePermission::Execute)))
        .flat_map(|((location, dataset), permission)| {
            let Some(dataset) = data_index.get(&dataset.name) else {
                return Either::Left(std::iter::once(Err(ValidationError::UnknownDataset(dataset.name.clone()))));
            };
            Either::Right(dataset.access.values().map(move |kind| match kind {
                specifications::data::AccessKind::File { path } => {
                    info!("Contents of the DataInfo object:\n{:#?}", dataset);
                    let local_identity = policy.get_local_identity(location, &workflow.user.name).map_err(ValidationError::PolicyError)?;
                    let result = satisfies_posix_permissions(path, local_identity, &permission);
                    Ok((dataset.name.clone(), path, result))
                },
            }))
        })
        // This is where we are going to focus on the problems that occurred in the validation
        // These can be separated into groups: Errors (e.g. Non-existing users / files), and
        // validation failures.
        .filter(|res| match res {
            // Filter out what was okay in either sense.
            Ok((_, _, true)) => false,
            _ => true,
        })
        .partition_map(|elem| match elem {
            Ok((dataset_identifier, _, _)) => Either::Left(dataset_identifier),
            Err(x) => Either::Right(x),
        });

    if !errors.is_empty() {
        Err(errors)
    } else if forbidden.is_empty() {
        return Ok(ValidationOutput::Ok);
    } else {
        return Ok(ValidationOutput::Fail(forbidden));
    }
}

/// Verifies whether the passed [`PosixLocalIdentity`] has all of the requested permissions (e.g., `Read` and `Write`)
/// on a particular file (defined by the `path`). The identity's user id and group ids are checked against the file
/// owner's user id and group id respectively. Additionally, the `Others` class permissions are also checked.
fn satisfies_posix_permissions(path: impl AsRef<Path>, local_identity: &PosixLocalIdentity, requested_permissions: &[PosixFilePermission]) -> bool {
    let metadata = std::fs::metadata(&path).expect("Could not get file metadata");

    let mode_bits = metadata.permissions().mode();
    let file_owner_uid = metadata.uid();
    let file_owner_gid = metadata.gid();

    if file_owner_uid == local_identity.uid {
        let mask = PosixFileClass::Owner.get_mode_bitmask(requested_permissions);
        if mode_bits & mask == mask {
            return true;
        }
    }

    if local_identity.gids.contains(&file_owner_gid) {
        let mask = PosixFileClass::Group.get_mode_bitmask(requested_permissions);
        if mode_bits & mask == mask {
            return true;
        }
    }

    let mask = PosixFileClass::Others.get_mode_bitmask(requested_permissions);
    mode_bits & mask == mask
}





/***** HELPERS *****/
enum ValidationOutput {
    Ok,
    // Below we might want to encapsulate the Dataset itself.
    /// The string here represents a `Dataset.name`.
    Fail(Vec<String>),
}

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
    fn get_mode_bitmask(&self, required_permissions: &[PosixFilePermission]) -> u32 {
        let alignment_multiplier = match self {
            PosixFileClass::Owner => 0o100,
            PosixFileClass::Group => 0o10,
            PosixFileClass::Others => 0o1,
        };
        required_permissions.iter().fold(0, |acc, f| acc | (alignment_multiplier * f.to_mode_bit()))
    }
}

/// Part of the [`PosixPolicy`]. Represents a location (e.g., `st_antonius_etc`) and contains the global workflow
/// username to local identity mappings for this location.
#[derive(Deserialize, Debug)]
pub struct PosixPolicyLocation {
    user_map: HashMap<GlobalUsername, PosixLocalIdentity>,
}

/// The local identity defines a user id and a list of group ids. The local identity is used on the machine on which a
/// dataset resides to check the local file permissions. For more about this permissions check see
/// [`validate_dataset_permissions`].
///
/// This identity is defined in the POSIX policy file. Global usernames in the POSIX policy map to these local
/// identities.
///
/// Example, given the POSIX policy file below, then for the `st_antonius_ect` location, the `test` global username maps
/// to a local identity that contains the uid and gids.
/// ``` yaml
///  # file: posix-policy.yml
///  content:
///    st_antonius_ect:
///      user_map:
///        test:
///          uid: 1000
///          gids:
///            - 1001
///            - 1002
///            - 1003
/// ```
#[derive(Deserialize, Debug)]
struct PosixLocalIdentity {
    /// The user identifier of a Linux user.
    uid:  u32,
    /// A list of Linux group identifiers.
    gids: Vec<u32>,
}

/// Represents a set of file permissions.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct PosixFilePermissions(u8);
impl PosixFilePermissions {
    /// Returns whether the read bit is set.
    #[inline]
    const fn read(&self) -> bool { (self.0 & (PosixFilePermission::Read.to_mode_bit() as u8)) != 0 }

    /// Returns whether the write bit is set.
    #[inline]
    const fn write(&self) -> bool { (self.0 & (PosixFilePermission::Write.to_mode_bit() as u8)) != 0 }

    /// Returns whether the execute bit is set.
    #[inline]
    const fn exec(&self) -> bool { (self.0 & (PosixFilePermission::Execute.to_mode_bit() as u8)) != 0 }

    /// Returns the raw bit pattern for this permission set.
    #[inline]
    const fn as_u8(&self) -> u8 { self.0 }
}
impl BitAnd<PosixFilePermission> for PosixFilePermissions {
    type Output = Self;

    #[inline]
    fn bitand(self, rhs: PosixFilePermission) -> Self::Output { Self(self.0 & (rhs.to_mode_bit() as u8)) }
}
impl BitAndAssign<PosixFilePermission> for PosixFilePermissions {
    #[inline]
    fn bitand_assign(&mut self, rhs: PosixFilePermission) { self.0 &= rhs.to_mode_bit() as u8; }
}
impl BitOr<PosixFilePermission> for PosixFilePermissions {
    type Output = Self;

    #[inline]
    fn bitor(self, rhs: PosixFilePermission) -> Self::Output { Self(self.0 | (rhs.to_mode_bit() as u8)) }
}
impl BitOrAssign<PosixFilePermission> for PosixFilePermissions {
    #[inline]
    fn bitor_assign(&mut self, rhs: PosixFilePermission) { self.0 |= rhs.to_mode_bit() as u8; }
}
impl BitAnd<Self> for PosixFilePermissions {
    type Output = Self;

    #[inline]
    fn bitand(self, rhs: Self) -> Self::Output { Self(self.0 & rhs.0) }
}
impl BitAndAssign<Self> for PosixFilePermissions {
    #[inline]
    fn bitand_assign(&mut self, rhs: Self) { self.0 &= rhs.0; }
}
impl BitOr<Self> for PosixFilePermissions {
    type Output = Self;

    #[inline]
    fn bitor(self, rhs: Self) -> Self::Output { Self(self.0 | rhs.0) }
}
impl BitOrAssign<Self> for PosixFilePermissions {
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) { self.0 |= rhs.0; }
}
impl From<PosixFilePermission> for PosixFilePermissions {
    #[inline]
    fn from(value: PosixFilePermission) -> Self { Self(value.to_mode_bit() as u8) }
}
impl From<PosixFilePermissions> for u8 {
    #[inline]
    fn from(value: PosixFilePermissions) -> Self { value.as_u8() }
}
impl From<&PosixFilePermissions> for u8 {
    #[inline]
    fn from(value: &PosixFilePermissions) -> Self { value.as_u8() }
}
impl From<&mut PosixFilePermissions> for u8 {
    #[inline]
    fn from(value: &mut PosixFilePermissions) -> Self { value.as_u8() }
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
    fn to_set(&self) -> PosixFilePermissions { PosixFilePermissions(self.to_mode_bit() as u8) }
}
impl BitAnd<Self> for PosixFilePermission {
    type Output = PosixFilePermissions;

    #[inline]
    fn bitand(self, rhs: PosixFilePermission) -> Self::Output { PosixFilePermissions((self.to_mode_bit() & rhs.to_mode_bit()) as u8) }
}
impl BitOr<Self> for PosixFilePermission {
    type Output = PosixFilePermissions;

    #[inline]
    fn bitor(self, rhs: PosixFilePermission) -> Self::Output { PosixFilePermissions((self.to_mode_bit() | rhs.to_mode_bit()) as u8) }
}
impl BitAnd<PosixFilePermissions> for PosixFilePermission {
    type Output = PosixFilePermissions;

    #[inline]
    fn bitand(self, rhs: PosixFilePermissions) -> Self::Output { PosixFilePermissions((self.to_mode_bit() as u8) & rhs.0) }
}
impl BitOr<PosixFilePermissions> for PosixFilePermission {
    type Output = PosixFilePermissions;

    #[inline]
    fn bitor(self, rhs: PosixFilePermissions) -> Self::Output { PosixFilePermissions((self.to_mode_bit() as u8) | rhs.0) }
}





/***** LIBRARY *****/
/// The overarching input to the POSIX reasoner.
#[derive(Debug)]
pub struct State {
    /// The policy to give.
    pub policy:   PosixPolicy,
    /// The workflow considered.
    pub workflow: Workflow,
}

/// The overarching POSIX policy. Check out the module documentation for an overview.
#[derive(Deserialize, Debug)]
pub struct PosixPolicy {
    datasets: HashMap<LocationIdentifier, PosixPolicyLocation>,
}
impl PosixPolicy {
    /// Given a location (e.g., `st_antonius_ect`) and the workflow user's name (e.g., `test`), returns the
    /// [`PosixLocalIdentity`] for that user.
    ///
    /// The returned identity is used for file permission checks. For more about this permissions check see
    /// [`validate_dataset_permissions`].
    fn get_local_identity(&self, location: &str, workflow_user: &str) -> Result<&PosixLocalIdentity, PolicyError> {
        self.datasets
            .get(location)
            .ok_or_else(|| PolicyError::MissingLocation(location.to_owned()))?
            .user_map
            .get(workflow_user)
            .ok_or_else(|| PolicyError::MissingUser(workflow_user.to_owned(), location.to_owned()))
    }
}



/// The POSIX reasoner connector. This connector is used to validate workflows based on POSIX file permissions.
pub struct PosixReasonerConnector;
impl PosixReasonerConnector {
    #[inline]
    pub const fn new() -> Self { PosixReasonerConnector }
}
impl ReasonerConnector for PosixReasonerConnector {
    type Error = Infallible;
    type Question = ();
    type Reason = Vec<String>;
    type State = State;

    fn consult<L>(
        &self,
        state: Self::State,
        _question: Self::Question,
        logger: &SessionedAuditLogger<L>,
    ) -> impl Future<Output = Result<ReasonerResponse<Self::Reason>, Self::Error>>
    where
        L: AuditLogger,
    {
        async move {
            let _span = span!(Level::INFO, "ReasonerConnector::consult", reference = logger.reference()).entered();

            match validate_dataset_permissions(&state.workflow, &state.policy) {
                Ok(ValidationOutput::Ok) => Ok(ReasonerResponse::Success),
                Ok(ValidationOutput::Fail(datasets)) => Ok(ReasonerResponse::Violated(
                    datasets.into_iter().map(|dataset| format!("We do not have sufficient permissions for dataset: {dataset}")).collect(),
                )),
                Err(errors) => Ok(ReasonerResponse::Violated(errors.into_iter().map(|error| error.to_string()).collect())),
            }
        }
    }
}

// /// The context of the POSIX reasoner connector. This context is used to identify the reasoner connector.
// /// See [`ConnectorContext`] and [`ConnectorWithContext`].
// #[derive(Debug, Clone, serde::Serialize)]
// pub struct PosixReasonerConnectorContext {
//     #[serde(rename = "type")]
//     pub t: String,
//     pub version: String,
// }

// impl std::hash::Hash for PosixReasonerConnectorContext {
//     fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
//         self.t.hash(state);
//         self.version.hash(state);
//     }
// }

// impl ConnectorContext for PosixReasonerConnectorContext {
//     fn r#type(&self) -> String { self.t.clone() }

//     fn version(&self) -> String { self.version.clone() }
// }

// impl ConnectorWithContext for PosixReasonerConnector {
//     type Context = PosixReasonerConnectorContext;

//     #[inline]
//     fn context() -> Self::Context { PosixReasonerConnectorContext { t: "posix".into(), version: "0.1.0".into() } }
// }
