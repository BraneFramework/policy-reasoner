//  STATE.rs
//    by Lut99
//
//  Created:
//    09 Jan 2024, 13:14:34
//  Last edited:
//    29 Jan 2024, 16:16:04
//  Auto updated?
//    Yes
//
//  Description:
//!   Implements resolvers for the policy state, e.g., which datasets
//!   there are, which domains, etc.
//

use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Display, Formatter, Result as FResult};
use std::fs;
use std::path::PathBuf;

use async_trait::async_trait;
use log::debug;
use nested_cli_parser::map_parser::MapParser;
use nested_cli_parser::NestedCliParser;
use state_resolver::{State, StateResolver};
#[cfg(feature = "brane-api-resolver")]
use ::{
    chrono::{DateTime, Utc},
    graphql_client::GraphQLQuery,
    log::{info, warn},
    reqwest::{Client, Request, Response, StatusCode},
    serde::{Deserialize, Serialize},
    specifications::data::DataInfo,
    state_resolver::StateResolverError,
    std::fs::File,
    uuid::Uuid,
    workflow::{Dataset, User},
};


/***** CONSTANTS *****/
/// The list of recognized keys for the arguments of the [`FileStateResolver`].
pub const FILE_STATE_RESOLVER_KEYS: [&'static str; 2] = ["f", "file"];

/// The list of recognized keys for the arguments of the [`BraneApiStateResolver`].
#[cfg(feature = "brane-api-resolver")]
pub const BRANE_API_STATE_RESOLVER_KEYS: [&'static str; 2] = ["u", "use-case-file"];





/***** TYPE ALIASES *****/
/// Type alias for [`DateTime<Utc>`] required by the GraphQL client.
#[cfg(feature = "brane-api-resolver")]
pub type DateTimeUtc = DateTime<Utc>;





/***** ERRORS *****/
/// Defines errors occurring in the [`FileStateResolver`].
#[derive(Debug)]
pub enum FileStateResolverError {
    /// Failed to parse the nested CLI arguments.
    CliArgumentsParse { raw: String, err: nested_cli_parser::map_parser::Error },
    /// Given the flag for the use case argument twice.
    CliDuplicatePath,
    /// The user did not tell us the path to the use case file.
    CliMissingPath,
    /// Failed to read a file.
    FileRead { path: PathBuf, err: std::io::Error },
    /// Failed to deserialize a file into JSON.
    FileDeserialize { path: PathBuf, err: serde_json::Error },
}
impl Display for FileStateResolverError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        use FileStateResolverError::*;
        match self {
            CliArgumentsParse { raw, .. } => write!(f, "Failed to parse '{raw}' as CLI argument string for a FileStateResolver"),
            CliDuplicatePath => write!(f, "Duplicate specification of file path (both 'p=...' and 'path=...' given)"),
            CliMissingPath => {
                write!(f, "File path not specified (give it as either '--state-resolver \"p=...\"' or '--state-resolver \"path=...\"')")
            },
            FileRead { path, .. } => write!(f, "Failed to read file '{}'", path.display()),
            FileDeserialize { path, .. } => write!(f, "Failed to deserialize file '{}' as JSON", path.display()),
        }
    }
}
impl Error for FileStateResolverError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        use FileStateResolverError::*;
        match self {
            CliArgumentsParse { err, .. } => Some(err),
            CliDuplicatePath => None,
            CliMissingPath => None,
            FileRead { err, .. } => Some(err),
            FileDeserialize { err, .. } => Some(err),
        }
    }
}



/// Defines a wrapper around a list of [`graphql_client::Error`]s.
#[cfg(feature = "brane-api-resolver")]
#[derive(Debug)]
pub struct GraphQlErrors(Vec<graphql_client::Error>);
#[cfg(feature = "brane-api-resolver")]
impl Display for GraphQlErrors {
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        writeln!(f, "The following errors were returned by the GraphQL server:")?;
        if !self.0.is_empty() {
            for err in &self.0 {
                writeln!(f, " - {err}")?;
            }
            writeln!(f)
        } else {
            writeln!(f, "<none>\n")
        }
    }
}
#[cfg(feature = "brane-api-resolver")]
impl Error for GraphQlErrors {}

/// Defines errors occurring in the [`BraneApiResolver`].
#[cfg(feature = "brane-api-resolver")]
#[derive(Debug)]
pub enum BraneApiResolverError {
    /// Failed to parse the nested CLI arguments.
    CliArgumentsParse { raw: String, err: nested_cli_parser::map_parser::Error },
    /// Given the flag for the use case argument twice.
    CliDuplicateUseCasePath,
    /// The user did not tell us the path to the use case file.
    CliMissingUseCasePath,
    /// Failed to open the use case file given.
    UseCaseFileOpen { path: PathBuf, err: std::io::Error },
    /// Failed to read & parse the use case file given.
    UseCaseFileRead { path: PathBuf, err: serde_yaml::Error },

    /// A GraphQL request failed.
    GraphQl { from: String, errs: Option<GraphQlErrors> },
    /// Failed to build a request to a particular address.
    RequestBuild { kind: &'static str, to: String, err: reqwest::Error },
    /// Failed to send a request to a particular address.
    RequestSend { kind: &'static str, to: String, err: reqwest::Error },
    /// Failed to download the body of a response.
    ResponseBody { from: String, err: reqwest::Error },
    /// Failed to parse the body of a response.
    ResponseBodyParse { from: String, raw: String, err: serde_json::Error },
    /// The response was not a 200 OK
    ResponseFailed { from: String, code: StatusCode, response: Option<String> },
    /// The given use-case identifier was not known to us.
    UnknownUseCase { raw: String },
}
#[cfg(feature = "brane-api-resolver")]
impl Display for BraneApiResolverError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        use BraneApiResolverError::*;
        match self {
            CliArgumentsParse { raw, .. } => write!(f, "Failed to parse '{raw}' as CLI argument string for a BraneApiResolver"),
            CliDuplicateUseCasePath => write!(f, "Duplicate specification of use case file path (both 'u=...' and 'use-case-path=...' given)"),
            CliMissingUseCasePath => write!(
                f,
                "Use case file path not specified (give it as either '--state-resolver \"u=...\"' or '--state-resolver \"use-case-path=...\"')"
            ),
            UseCaseFileOpen { path, .. } => write!(f, "Failed to open use case file '{}'", path.display()),
            UseCaseFileRead { path, .. } => write!(f, "Failed to read & parse use case file '{}' as YAML", path.display()),

            GraphQl { from, .. } => write!(f, "Received GraphQL errors from '{from}'"),
            RequestBuild { kind, to, .. } => write!(f, "Failed to build {kind}-request to '{to}'"),
            RequestSend { kind, to, .. } => write!(f, "Failed to send {kind}-request to '{to}'"),
            ResponseBody { from, .. } => write!(f, "Failed to download response body from '{from}'"),
            ResponseBodyParse { from, raw, .. } => write!(
                f,
                "Failed to parse response body from '{}' as JSON\n\nRaw response:\n{}\n{}\n{}\n",
                from,
                (0..80).map(|_| '-').collect::<String>(),
                raw,
                (0..80).map(|_| '-').collect::<String>()
            ),
            ResponseFailed { from, code, response } => write!(
                f,
                "Registry at {} returned {} ({}){}",
                from,
                code.as_u16(),
                code.canonical_reason().unwrap_or("???"),
                if let Some(response) = response {
                    format!(
                        "\n\nResponse:\n{}\n{}\n{}\n",
                        (0..80).map(|_| '-').collect::<String>(),
                        response,
                        (0..80).map(|_| '-').collect::<String>()
                    )
                } else {
                    String::new()
                }
            ),
            UnknownUseCase { raw } => write!(f, "Unknown use-case identifier '{raw}'"),
        }
    }
}
#[cfg(feature = "brane-api-resolver")]
impl Error for BraneApiResolverError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        use BraneApiResolverError::*;
        match self {
            CliArgumentsParse { err, .. } => Some(err),
            CliDuplicateUseCasePath => None,
            CliMissingUseCasePath => None,
            UseCaseFileOpen { err, .. } => Some(err),
            UseCaseFileRead { err, .. } => Some(err),

            GraphQl { errs, .. } => errs.as_ref().map(|errs| {
                // Bit ugly, but needed as cast from `&GraphQlErrors` to `&dyn Error`
                let errs: &dyn Error = errs;
                errs
            }),
            RequestBuild { err, .. } => Some(err),
            RequestSend { err, .. } => Some(err),
            ResponseBody { err, .. } => Some(err),
            ResponseBodyParse { err, .. } => Some(err),
            ResponseFailed { .. } => None,
            UnknownUseCase { .. } => None,
        }
    }
}
#[cfg(feature = "brane-api-resolver")]
impl StateResolverError for BraneApiResolverError {
    #[inline]
    fn try_as_unknown_use_case(&self) -> Option<&String> { if let Self::UnknownUseCase { raw } = self { Some(raw) } else { None } }
}





/***** CONFIG *****/
/// Defines the use case configuration file as far as the state resolver is concerned.
#[cfg(feature = "brane-api-resolver")]
#[derive(Clone, Debug, Deserialize, Serialize)]
struct UseCaseFile {
    /// The list of use cases.
    use_cases: HashMap<String, UseCaseBrane>,
}

/// Defines what we need to know about a Brane instance map.
#[cfg(feature = "brane-api-resolver")]
#[derive(Clone, Debug, Deserialize, Serialize)]
struct UseCaseBrane {
    /// The address of the API registry to resolve state with.
    #[serde(alias = "brane-api", alias = "registry")]
    api: String,
}





/***** LIBRARY *****/
/// Defines a resolver that resolves from a static file.
#[derive(Debug)]
pub struct FileStateResolver {
    /// The state read from the file.
    state: State,
}

impl FileStateResolver {
    /// Constructor for the FileStateResolver.
    ///
    /// # Arguments
    /// - `cli_args`: A raw string with CLI arguments given to us by the `policy-reasoner` executable.
    ///
    /// # Returns
    /// A new FileStateResolver instance.
    ///
    /// # Errors
    /// This function may error if it failed to read the given file.
    #[inline]
    pub fn new(cli_args: String) -> Result<Self, FileStateResolverError> {
        // Parse the arguments using the [`MapParser`].
        debug!("Parsing nested arguments for FileStateResolver");
        let parser = MapParser::new(["p", "path"]);
        let args: HashMap<String, Option<String>> = match parser.parse(&cli_args) {
            Ok(args) => args,
            Err(err) => return Err(FileStateResolverError::CliArgumentsParse { raw: cli_args, err }),
        };

        // See what to do with it
        let path: PathBuf = match (args.get("p".into()), args.get("path")) {
            (Some(_), Some(_)) => return Err(FileStateResolverError::CliDuplicatePath),
            (Some(Some(path)), _) | (_, Some(Some(path))) => path.into(),
            _ => return Err(FileStateResolverError::CliMissingPath),
        };

        // Read the file in one go
        debug!("Opening input file '{}'...", path.display());
        let state: String = match fs::read_to_string(&path) {
            Ok(state) => state,
            Err(err) => return Err(FileStateResolverError::FileRead { path: path.into(), err }),
        };

        // Parse it as JSON
        debug!("Parsing input file '{}'...", path.display());
        let state: State = match serde_json::from_str(&state) {
            Ok(state) => state,
            Err(err) => return Err(FileStateResolverError::FileDeserialize { path: path.into(), err }),
        };

        // Build ourselves with it
        Ok(Self { state })
    }
}

#[async_trait]
impl StateResolver for FileStateResolver {
    type Error = std::convert::Infallible;

    async fn get_state(&self, _use_case: String) -> Result<State, Self::Error> {
        // Simply return a clone of the internal one
        Ok(self.state.clone())
    }
}



/// Defines a resolver that resolves state using Brane's API service.
#[cfg(feature = "brane-api-resolver")]
#[derive(Debug)]
pub struct BraneApiResolver {
    /// A map from use case identifiers to where we can find the relevant Brane API registry.
    use_cases: HashMap<String, UseCaseBrane>,
}

#[cfg(feature = "brane-api-resolver")]
impl BraneApiResolver {
    /// Constructor for the BraneApiResolver.
    ///
    /// # Arguments
    /// - `cli_args`: A raw string with CLI arguments given to us by the `policy-reasoner` executable.
    ///
    /// # Returns
    /// A new BraneApiResolver instance.
    ///
    /// # Errors
    /// This function errors if `cli_args` was not parsed successfully.
    pub fn new(cli_args: String) -> Result<Self, BraneApiResolverError> {
        // Parse the arguments using the [`MapParser`].
        debug!("Parsing nested arguments for BraneApiResolver");
        let parser = MapParser::new(["u", "use-case-path"]);
        let args: HashMap<String, Option<String>> = match parser.parse(&cli_args) {
            Ok(args) => args,
            Err(err) => return Err(BraneApiResolverError::CliArgumentsParse { raw: cli_args, err }),
        };

        // See what to do with it
        let use_cases: UseCaseFile = match (args.get("u".into()), args.get("use-case-path")) {
            (Some(_), Some(_)) => return Err(BraneApiResolverError::CliDuplicateUseCasePath),
            (Some(Some(path)), _) | (_, Some(Some(path))) => {
                // Attempt to open the file
                debug!("Opening use case file '{path}'...");
                let handle: File = match File::open(path) {
                    Ok(handle) => handle,
                    Err(err) => return Err(BraneApiResolverError::UseCaseFileOpen { path: path.into(), err }),
                };

                // Attempt to parse the file
                debug!("Parsing use case file '{path}'...");
                match serde_yaml::from_reader(handle) {
                    Ok(use_cases) => use_cases,
                    Err(err) => return Err(BraneApiResolverError::UseCaseFileRead { path: path.into(), err }),
                }
            },
            _ => return Err(BraneApiResolverError::CliMissingUseCasePath),
        };

        // Done, store the list of use cases!
        Ok(Self { use_cases: use_cases.use_cases })
    }
}

#[cfg(feature = "brane-api-resolver")]
#[async_trait]
impl StateResolver for BraneApiResolver {
    type Error = BraneApiResolverError;

    async fn get_state(&self, use_case: String) -> Result<State, Self::Error> {
        info!("Resolving state using `brane-api` for use-case '{use_case}'");

        // Attempt to find a registry to call
        let address: &str = match self.use_cases.get(&use_case) {
            Some(address) => &address.api,
            None => return Err(BraneApiResolverError::UnknownUseCase { raw: use_case }),
        };
        debug!("Use case '{use_case}' is known, requesting state from registry at '{address}'");

        // Do the users call first
        let users: Vec<User> = {
            warn!("Cannot request list of users from 'brane-api', as it does not have this information; assuming empty list");
            vec![]
        };
        debug!("Retrieved {} users", users.len());

        // Next, retrieve domains
        let locations: Vec<User> = {
            let url: String = format!("{address}/infra/registries");
            debug!("Retrieving list of domains from '{url}'...");

            // Build the request
            let client = Client::new();
            let req: Request = match client.get(&url).build() {
                Ok(req) => req,
                Err(err) => return Err(BraneApiResolverError::RequestBuild { kind: "GET", to: url, err }),
            };
            let res: Response = match client.execute(req).await {
                Ok(res) => res,
                Err(err) => return Err(BraneApiResolverError::RequestSend { kind: "GET", to: url, err }),
            };
            if !res.status().is_success() {
                return Err(BraneApiResolverError::ResponseFailed { from: url, code: res.status(), response: res.text().await.ok() });
            }

            // Attempt to parse the result as a map of domains
            let body: String = match res.text().await {
                Ok(body) => body,
                Err(err) => return Err(BraneApiResolverError::ResponseBody { from: url, err }),
            };
            let registries: HashMap<String, String> = match serde_json::from_str(&body) {
                Ok(regs) => regs,
                Err(err) => return Err(BraneApiResolverError::ResponseBodyParse { from: url, raw: body, err }),
            };

            // The keys of the map are our domains
            registries.into_keys().map(|name| User { name }).collect()
        };
        debug!("Retrieved {} locations", locations.len());

        // Then we retrieve the list of available datasets
        let datasets: Vec<Dataset> = {
            let url: String = format!("{address}/data/info");
            debug!("Retrieving list of datasets from '{url}'...");

            // Build the request
            let client = Client::new();
            let req: Request = match client.get(&url).build() {
                Ok(req) => req,
                Err(err) => return Err(BraneApiResolverError::RequestBuild { kind: "GET", to: url, err }),
            };
            let res: Response = match client.execute(req).await {
                Ok(res) => res,
                Err(err) => return Err(BraneApiResolverError::RequestSend { kind: "GET", to: url, err }),
            };
            if !res.status().is_success() {
                return Err(BraneApiResolverError::ResponseFailed { from: url, code: res.status(), response: res.text().await.ok() });
            }

            // Attempt to parse the result as a list of [`DataInfo`]s
            let body: String = match res.text().await {
                Ok(body) => body,
                Err(err) => return Err(BraneApiResolverError::ResponseBody { from: url, err }),
            };
            let datasets: HashMap<String, DataInfo> = match serde_json::from_str(&body) {
                Ok(regs) => regs,
                Err(err) => return Err(BraneApiResolverError::ResponseBodyParse { from: url, raw: body, err }),
            };

            // We build our own objects from this
            datasets.into_keys().map(|name| Dataset { name, from: None }).collect()
        };
        debug!("Retrieved {} datasets", datasets.len());

        // Finally, retrieve the list of containers
        let functions: Vec<Dataset> = {
            // Build the GraphQL file
            #[derive(GraphQLQuery)]
            #[graphql(schema_path = "src/graphql/api_schema.json", query_path = "src/graphql/search_packages.graphql", response_derives = "Debug")]
            pub struct SearchPackages;

            let url: String = format!("{address}/graphql");
            debug!("Retrieving list of datasets from '{url}'...");

            // Prepare GraphQL query.
            let variables = search_packages::Variables { term: None };
            let graphql_query = SearchPackages::build_query(variables);

            // Send a request
            let client = Client::new();
            let graphql_response = match client.post(&url).json(&graphql_query).send().await {
                Ok(res) => res,
                Err(err) => return Err(BraneApiResolverError::RequestSend { kind: "POST", to: url, err }),
            };
            let graphql_response: String = match graphql_response.text().await {
                Ok(res) => res,
                Err(err) => return Err(BraneApiResolverError::ResponseBody { from: url, err }),
            };
            let graphql_response: graphql_client::Response<search_packages::ResponseData> = match serde_json::from_str(&graphql_response) {
                Ok(res) => res,
                Err(err) => return Err(BraneApiResolverError::ResponseBodyParse { from: url, raw: graphql_response, err }),
            };

            // See if any data was returned
            if let Some(data) = graphql_response.data {
                data.packages.into_iter().map(|package| Dataset { name: package.name, from: Some("<central>".into()) }).collect()
            } else {
                return Err(BraneApiResolverError::GraphQl { from: url, errs: graphql_response.errors.map(|errs| GraphQlErrors(errs)) });
            }
        };
        debug!("Retrieved {} functions", functions.len());

        // Done, return it as one set
        let state = State { users, locations, datasets, functions };
        debug!("Complete state retrieved from '{address}': {state:#?}");
        Ok(state)
    }
}
