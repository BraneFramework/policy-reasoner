//  CONFIG.rs
//    by Lut99
//
//  Created:
//    15 Oct 2024, 14:17:44
//  Last edited:
//    15 Oct 2024, 14:37:11
//  Auto updated?
//    Yes
//
//  Description:
//!   Defines the outwards-facing config file that sets the
//!   `posix`-reasoner up.
//

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};


/***** LIBRARY *****/
/// Defines the config for the POSIX-reasoner.
///
/// Specifically, this:
/// - Maps datasets to where to find them; and
/// - Maps which users are allowed to read from/write to it.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Config {
    /// Defines the location ID of this location.
    pub id:   String,
    /// Defines a map from datasets to where to find them on the disk (that one Harry Potter movie?)
    #[serde(default = "HashMap::new", skip_serializing_if = "HashMap::is_empty")]
    pub data: HashMap<String, DataPolicy>,
}



/// Part of the [`Config`]. Represents a location (e.g., `st_antonius_etc`) and contains the global workflow
/// username to local identity mappings for this location.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DataPolicy {
    /// The location where we find this dataset on disk.
    pub path:     PathBuf,
    #[serde(default = "HashMap::new", skip_serializing_if = "HashMap::is_empty")]
    pub user_map: HashMap<String, PosixLocalIdentity>,
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
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PosixLocalIdentity {
    /// The user identifier of a Linux user.
    pub uid:  u32,
    /// A list of Linux group identifiers.
    #[serde(default = "Vec::new", skip_serializing_if = "Vec::is_empty")]
    pub gids: Vec<u32>,
}
