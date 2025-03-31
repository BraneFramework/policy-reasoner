//! A policy reasoner implementation based on POSIX file permissions.
//!
//! This documentation is aimed at developers that want to maintain or extend the POSIX reasoner. High level
//! documentation for users of the POSIX reasoner can be found in the [Brane user
//! guide](https://wiki.enablingpersonalizedinterventions.nl/user-guide/). An explanation of the POSIX policy file can
//! also be found there.
//!
//! # Goal
//!
//! This policy reasoner is meant to be easy and widely applicable. The aim is to take few assumptions and require as
//! little configuration as possible, allowing this reasoner to function as an easy to deploy proof of concept. This
//! allows users of Brane to gather experience with the abstract concept of a policy reasoner, before one has to start
//! writing policies themselves.
//!
//! Additionally, it could function well as an initial reasoner as a user adopts Brane on their systems, since it could
//! use permissions already set on their current systems to infer which users have access to what data.
//!
//! # Design
//!
//! To describe the design of this policy reasoner, we will walk through both the setup and usage of the reasoner from a
//! high level point of view.
//!
//! First, it checks the `DATA_INDEX` environment variable or the .env file for the location of the data index. We
//! imagine this points towards a mounted distributed file system like NFS. Then, it scans the directories for data
//! index files. From these files a [DataIndex] is created which is passed on to the [PosixReasonerConnector].
//!
//! Now that the [PosixReasonerConnector] is created, it can start to handle requests. There are three types of
//! requests:
//!
//! - [Execute task](fn@PosixReasonerConnector::execute_task)
//! - [Access data](fn@PosixReasonerConnector::access_data_request)
//! - [Workflow validation](fn@PosixReasonerConnector::workflow_validation_request)
//!
//! As of now, the assumption is taken that it does not matter for this reasoner which type of request comes in, as we
//! only look at the data usage in the [Workflow].
//!
//! As one of these requests comes in, the provided [Workflow] is parsed using a [DatasetCollectorVisitor] (an
//! implementation of the new util trait [WorkflowVisitor]) and all data accesses in the workflow are gathered and
//! associated with an access type of either read, write, or execute (execute is currently unused as no usage was
//! found).
//!
//! From this point, we iterate over all the different datasets and associated requests/required permissions. For each
//! [Dataset] we look up the path in the [DataIndex]. Now that we have the path and the requested permissions, we can
//! check if the user in the mapping has access to this dataset.
//!
//! ### Current permission model
//!
//! The current permission model is based on the POSIX file permissions. This means that we check if the user has the
//! required permissions on the file. This is done by checking the file permissions of the file itself, and checking if
//! the user is either the owner of the file, in the group of the file, or if the file is world readable. The uid and
//! the gids extracted from the policy are matched against the file's uid and gid. If the file is owned by the user, the
//! owner permissions are checked. If the file is owned by a group the user is in, the group permissions are checked. If
//! neither of these is true, the other permissions are checked. If the user has the required permissions, the request is
//! approved. If not, the request is denied : [satisfies_posix_permissions].
//!
//!
//! # State of the implementation
//!
//! Right now, the POSIX policy reasoner works. One can submit workflow requests using a HTTP request, or one can use
//! the Policy Reasoner GUI to submit a workflow request to the policy reasoner. The policy reasoner will then evaluate
//! if all files are properly accessible and return a verdict.
//!
//! ## Limitations
//!
//! We had to draw a line in the sand for the current implementation, after which we considered additional details or
//! features out of the scope for the current project. If more time would have been available, there would have been a
//! fair bit of additional features that we would have liked to have implemented.
//!
//! The main origin of this line is the lack of an actual implementation inside Brane. To fully understand the
//! implications of the POSIX file permissions scheme, we need to start using the current implementation in a staging
//! environment. We should investigate the effects of multiple sites with many datasets. However, right now it is not
//! possible (or so we have been told) to mount the network shares to the reasoner container.
//!
//! Right now, this implementation is limited by the fact that it will try to load a single data index from the
//! `DATA_INDEX` environment variable. However, once we start mounting multiple network shares, we imagine that it might
//! be nice to be able to load multiple data indices.
//!
//! Another limitation is that the current implementation is not fully POSIX compliant. We still need to figure out how
//! some of the POSIX permission behaviours map into this emulation. E.g., right now we only check the file permissions
//! on the file itself, we do not check the permissions on the directory. Since we are going to be working with network
//! shares (and possible hard/symlinks) this becomes non-trivial, a working implementation is needed to investigate what
//! behaviour is desired. This is compounded by the problem that not only the user needs to be able to access the data,
//! but also the policy reasoner needs to reach at least the directory in which the file resides in order for the
//! reasoner to be able to `stat(1)` the file.
//!
//! Right now, we are seemingly limited by the fact that the policy reasoner GUI does not send at which site a [Dataset]
//! is accessed, making it impossible to fully know which of the mappings in the policy to use. There is a location
//! field that could be used, but since it is always set to `None`, we opted to fall back to the assumed location as
//! hardcoded in the static: [`ASSUMED_LOCATION`]
//!
//! # Future work
//!
//! - Support file creation: This requires us to look at the permissions of the parent directory of the potential
//!   dataset, but this might be tricky considering the fact that the policy reasoner also needs to be able to access the
//!   directory in order to check the file permissions.
//!
//! - Support multiple user mappings per location: Right now there is support in the code for multiple locations with
//!   each their own user mapping, as different sites will often comprise of different network shares. But as it is
//!   unclear right now how the volumes will be mounted on the reasoner container, it is hard to tell how such an
//!   implementation is best designed.
//!
//! - Right now, this reasoner is both a module and a binary, but it should probably just be a binary. At this moment
//!   however, documenting binaries is tricky in rust. As soon as we find a better solution, this documentation should be
//!   moved to the binary itself. This might be useful in general, but particularly it is important to document reference
//!   implementations.
//!
//! - LDAP / Active Directory support: Currently the Brane user to uid / gid mapping is embedded in the policy that is
//!   loaded at the moment the reasoner is started. The idea of mapping users to uid and gids is not unique though, these
//!   mappings can be sythesized from all sorts of resources. The most straightforward variant would be the loading of a
//!   `passwd(5)` file, but since we are aiming at distributed file systems this would probably be of limited use. In
//!   situations where file systems like NFS are often used, the users are store in Active Directory and accessed using
//!   LDAP. Writing such an adapter to function as the user map seems so be a valuable addition to this reasoner, as this
//!   would complete the picture of current existing systems and would allow a user of the reasoner to attach their
//!   already existing and managed file systems with the correct access control. This would significantly reduce the
//!   required investment of introducing policies in new Brane users.
//!
//! - ACL support: Besides regular POSIX file permissions, many file systems also support [POSIX
//!   ACL](https://web.archive.org/web/20240210045229/https://www.usenix.org/legacy/publications/library/proceedings/usenix03/tech/freenix03/full_papers/gruenbacher/gruenbacher_html/main.html).
//!   This would be an obvious and very useful extension to this reasoner. Usage of these ACLs is by far less common that
//!   the regular POSIX permission it attempts to extend, but usage is also far from uncommon and potenial users of Brane,
//!   and more specifically the policy reasoner, could have used these ACLs on their existing file systems.
//!
//! - The last point of future work is not specific to the POSIX reasoner, but more to the policy reasoner repository.
//!   There is a desperate need for more elaborate documentation. We attempted to create a nice start with this reasoner,
//!   but it can be quite daunting to figure out how every part of this system works. The author made a good effort to
//!   improve the documentation during the running of this project, and that helped a lot. That combined with the already
//!   existing implementation of the eFlint reasoner made this project possible. We hope that the POSIX (and
//!   [no_op](crate::no_op)) reasoner can help guide future contributors in either extension of the current reasoners or
//!   the addition of new reasoner types.

use std::collections::{HashMap, HashSet};
use std::iter::repeat;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;

use audit_logger::{ConnectorContext, ConnectorWithContext, ReasonerConnectorAuditLogger, SessionedConnectorAuditLogger};
use itertools::{Either, Itertools};
use log::{debug, error, info};
use policy::{Policy, PolicyContent};
use reasonerconn::{ReasonerConnError, ReasonerConnector, ReasonerResponse};
use serde::Deserialize;
use specifications::data::{DataIndex, Location};
use state_resolver::State;
use workflow::Dataset;
use workflow::spec::Workflow;
use workflow::utils::{WorkflowVisitor, walk_workflow_preorder};

/// This location is an assumption right now, and is needed as long as the location is not passed to the workflow
/// validator.
static ASSUMED_LOCATION: &str = "surf";

/***** LIBRARY *****/
/// E.g., `st_antonius_etc`.
type LocationIdentifier = String;
/// The global username as defined in [`Workflow.user`]. E.g., `test`.
type GlobalUsername = String;

/// The overarching POSIX policy. Check out the module documentation for an overview.
#[derive(Deserialize, Debug)]
pub struct PosixPolicy {
    datasets: HashMap<LocationIdentifier, PosixPolicyLocation>,
}

impl PosixPolicy {
    /// Extracts and parses a [`PosixPolicy`] from a generic [`Policy`] object. Expects the policy to be specified and
    /// expects it to adhere to the [`PosixPolicy`] YAML structure. See [`PosixPolicy`].
    fn from_policy(policy: Policy) -> Self {
        let policy_content: PolicyContent = policy.content.first().expect("Failed to parse PolicyContent").clone();
        let content_str = policy_content.content.get().trim();
        PosixPolicy { datasets: serde_json::from_str(content_str).expect("Failed to parse PosixPolicy") }
    }

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

/// Represents an error that occurred during the validation of a policy. These errors contain more information about the
/// problems that occurred during validation.
#[derive(thiserror::Error, Debug)]
enum PolicyError {
    #[error("Missing location: {0}")]
    MissingLocation(String),
    #[error("Missing user: {0} for location: {1}")]
    MissingUser(String, String),
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
    fn to_mode_bit(self) -> u32 {
        match self {
            PosixFilePermission::Read => 4,
            PosixFilePermission::Write => 2,
            PosixFilePermission::Execute => 1,
        }
    }
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

enum ValidationOutput {
    Ok,
    // Below we might want to encapsulate the Dataset itself.
    /// The string here represents a `Dataset.name`.
    Fail(Vec<String>),
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

/// Check if all the data accesses performed in the `workflow` are done on behalf of users that have the required
/// permissions. If not all permissions are met, then [`ValidationError`]s are returned. These errors contain more
/// information about the problems that occurred during validation.
fn validate_dataset_permissions(workflow: &Workflow, data_index: &DataIndex, policy: &PosixPolicy) -> Result<ValidationOutput, Vec<ValidationError>> {
    // The datasets used in the workflow. E.g., `st_antonius_ect`.
    let datasets = find_datasets_in_workflow(workflow);

    let (forbidden, errors): (Vec<_>, Vec<_>) = std::iter::empty()
        .chain(datasets.read_sets.iter().zip(repeat(vec![PosixFilePermission::Read])))
        .chain(datasets.write_sets.iter().zip(repeat(vec![PosixFilePermission::Write])))
        .chain(datasets.execute_sets.iter().zip(repeat(vec![PosixFilePermission::Read, PosixFilePermission::Execute])))
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

/// The POSIX reasoner connector. This connector is used to validate workflows based on POSIX file permissions.
pub struct PosixReasonerConnector {
    data_index: DataIndex,
}

impl PosixReasonerConnector {
    pub fn new(data_index: DataIndex) -> Self {
        info!("Creating new PosixReasonerConnector with {} plugin", std::any::type_name::<Self>());
        debug!("Parsing nested arguments for PosixReasonerConnector<{}>", std::any::type_name::<Self>());

        PosixReasonerConnector { data_index }
    }
}

/***** LIBRARY *****/
#[async_trait::async_trait]
impl<L: ReasonerConnectorAuditLogger + Send + Sync + 'static> ReasonerConnector<L> for PosixReasonerConnector {
    async fn execute_task(
        &self,
        _logger: SessionedConnectorAuditLogger<L>,
        policy: Policy,
        _state: State,
        workflow: Workflow,
        _task: String,
    ) -> Result<ReasonerResponse, ReasonerConnError> {
        let posix_policy = PosixPolicy::from_policy(policy);
        match validate_dataset_permissions(&workflow, &self.data_index, &posix_policy) {
            Ok(ValidationOutput::Ok) => Ok(ReasonerResponse::new(true, vec![])),
            Ok(ValidationOutput::Fail(datasets)) => Ok(ReasonerResponse::new(
                false,
                datasets.into_iter().map(|dataset| format!("We do not have sufficient permissions for dataset: {dataset}")).collect(),
            )),
            Err(errors) => Ok(ReasonerResponse::new(false, errors.into_iter().map(|error| error.to_string()).collect())),
        }
    }

    async fn access_data_request(
        &self,
        _logger: SessionedConnectorAuditLogger<L>,
        policy: Policy,
        _state: State,
        workflow: Workflow,
        _data: String,
        _task: Option<String>,
    ) -> Result<ReasonerResponse, ReasonerConnError> {
        let posix_policy = PosixPolicy::from_policy(policy);
        match validate_dataset_permissions(&workflow, &self.data_index, &posix_policy) {
            Ok(ValidationOutput::Ok) => Ok(ReasonerResponse::new(true, vec![])),
            Ok(ValidationOutput::Fail(datasets)) => Ok(ReasonerResponse::new(
                false,
                datasets.into_iter().map(|dataset| format!("We do not have sufficient permissions for dataset: {dataset}")).collect(),
            )),
            Err(errors) => Ok(ReasonerResponse::new(false, errors.into_iter().map(|error| error.to_string()).collect())),
        }
    }

    async fn workflow_validation_request(
        &self,
        _logger: SessionedConnectorAuditLogger<L>,
        policy: Policy,
        _state: State,
        workflow: Workflow,
    ) -> Result<ReasonerResponse, ReasonerConnError> {
        let posix_policy = PosixPolicy::from_policy(policy);
        match validate_dataset_permissions(&workflow, &self.data_index, &posix_policy) {
            Ok(ValidationOutput::Ok) => Ok(ReasonerResponse::new(true, vec![])),
            Ok(ValidationOutput::Fail(datasets)) => Ok(ReasonerResponse::new(
                false,
                datasets.into_iter().map(|dataset| format!("We do not have sufficient permissions for dataset: {dataset}")).collect(),
            )),
            Err(errors) => Ok(ReasonerResponse::new(false, errors.into_iter().map(|error| error.to_string()).collect())),
        }
    }
}

/// The context of the POSIX reasoner connector. This context is used to identify the reasoner connector.
/// See [`ConnectorContext`] and [`ConnectorWithContext`].
#[derive(Debug, Clone, serde::Serialize)]
pub struct PosixReasonerConnectorContext {
    #[serde(rename = "type")]
    pub t: String,
    pub version: String,
}

impl std::hash::Hash for PosixReasonerConnectorContext {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.t.hash(state);
        self.version.hash(state);
    }
}

impl ConnectorContext for PosixReasonerConnectorContext {
    fn r#type(&self) -> String { self.t.clone() }

    fn version(&self) -> String { self.version.clone() }
}

impl ConnectorWithContext for PosixReasonerConnector {
    type Context = PosixReasonerConnectorContext;

    #[inline]
    fn context() -> Self::Context { PosixReasonerConnectorContext { t: "posix".into(), version: "0.1.0".into() } }
}

/// The datasets accessed and/or modified in a workflow. These are grouped by file permission type. For creating this
/// struct see: [`find_datasets_in_workflow`].
struct WorkflowDatasets {
    read_sets:    Vec<(Location, Dataset)>,
    write_sets:   Vec<(Location, Dataset)>,
    execute_sets: Vec<(Location, Dataset)>,
}

fn find_datasets_in_workflow(workflow: &Workflow) -> WorkflowDatasets {
    debug!("Walking the workflow in order to find datasets. Starting with {:?}", &workflow.start);
    let mut visitor =
        DatasetCollectorVisitor { read_sets: Default::default(), write_sets: Default::default(), execute_sets: Default::default() };

    walk_workflow_preorder(&workflow.start, &mut visitor);

    WorkflowDatasets { read_sets: visitor.read_sets, write_sets: visitor.write_sets, execute_sets: visitor.execute_sets }
}

/// Implements a visitor that traverses a [`Workflow`] and collect the datasets that are accessed and/or modified in
/// the workflow. See: [`WorkflowDatasets`] and [`WorkflowVisitor`].
struct DatasetCollectorVisitor {
    pub read_sets:    Vec<(Location, Dataset)>,
    pub write_sets:   Vec<(Location, Dataset)>,
    pub execute_sets: Vec<(Location, Dataset)>,
}

impl WorkflowVisitor for DatasetCollectorVisitor {
    fn visit_task(&mut self, task: &workflow::ElemTask) {
        // FIXME: Location is not currently sent as part of the workflow validation request,
        // this makes this not really possible to do now. To ensure the code is working
        // however, we will for the mean time assume the location

        let location = task.location.clone().unwrap_or_else(|| String::from(ASSUMED_LOCATION));
        if let Some(output) = &task.output {
            self.read_sets.push((location.clone(), output.clone()));
        }
    }

    fn visit_commit(&mut self, commit: &workflow::ElemCommit) {
        let location = commit.location.clone().unwrap_or_else(|| String::from(ASSUMED_LOCATION));
        self.read_sets.extend(repeat(location.clone()).zip(commit.input.iter().cloned()));

        // TODO: Maybe create a dedicated enum type for this e.g. NewDataset for datasets that will be
        // created, might fail if one already exists.
        let location = commit.location.clone().unwrap_or_else(|| String::from(ASSUMED_LOCATION));
        self.write_sets.push((location.clone(), Dataset { name: commit.data_name.clone(), from: None }));
    }

    // TODO: We do not really have a location for this one right now, we should figure out how to
    // interpret this
    fn visit_stop(&mut self, stop_sets: &HashSet<Dataset>) {
        let location = String::from(ASSUMED_LOCATION);
        self.write_sets.extend(repeat(location).zip(stop_sets.iter().cloned()));
    }
}
