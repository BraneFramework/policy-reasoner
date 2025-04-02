//! No-op policy reasoner
//! This reasoner does a little as possible and functions as base for other implementations of the
//! policy reasoner.
use std::env;
use std::fs::File;
use std::future::Future;

pub mod implementation;

use async_trait::async_trait;
use clap::Parser;
use humanlog::{DebugMode, HumanLogger};
use implementation::interface::Arguments;
use implementation::no_op::NoOpReasonerConnector;
use log::info;
use policy::{Context, Policy, PolicyDataAccess, PolicyDataError, PolicyVersion};
use policy_reasoner::auth::{JwtConfig, JwtResolver, KidResolver};
use policy_reasoner::logger::FileLogger;
use reasonerconn::ReasonerConnector;
use srv::Srv;
use state_resolver::{State, StateResolver};

/***** HELPER FUNCTIONS *****/
fn get_pauth_resolver() -> policy_reasoner::auth::JwtResolver<KidResolver> {
    let kid_resolver = KidResolver::new("./examples/config/jwk_set_expert.json").unwrap();
    let r = File::open("./examples/config/jwt_resolver.yaml").unwrap();
    let jwt_cfg: JwtConfig = serde_yaml::from_reader(r).unwrap();
    JwtResolver::new(jwt_cfg, kid_resolver).unwrap()
}
fn get_dauth_resolver() -> policy_reasoner::auth::JwtResolver<KidResolver> {
    let kid_resolver = KidResolver::new("./examples/config/jwk_set_delib.json").unwrap();
    let r = File::open("./examples/config/jwt_resolver.yaml").unwrap();
    let jwt_cfg: JwtConfig = serde_yaml::from_reader(r).unwrap();
    JwtResolver::new(jwt_cfg, kid_resolver).unwrap()
}

/***** PLUGINS *****/
/// The plugin used to do the audit logging.
type AuditLogPlugin = FileLogger;

/// The plugin used to do authentication for the policy expert API.
type PolicyAuthResolverPlugin = JwtResolver<KidResolver>;
/// The plugin used to do authentication for the deliberation API.
type DeliberationAuthResolverPlugin = JwtResolver<KidResolver>;

/// The plugin used to interact with the policy store.
type PolicyStorePlugin = DummyPolicyStore;

/// The plugin used to resolve policy input state.
struct DummyStateResolver;

#[async_trait]
impl StateResolver for DummyStateResolver {
    type Error = std::convert::Infallible;

    async fn get_state(&self, _use_case: String) -> Result<State, Self::Error> {
        Ok(State { users: Default::default(), locations: Default::default(), datasets: Default::default(), functions: Default::default() })
    }
}

type StateResolverPlugin = DummyStateResolver;

struct DummyPolicyStore;

#[async_trait]
impl PolicyDataAccess for DummyPolicyStore {
    type Error = std::convert::Infallible;

    async fn add_version<F: 'static + Send + Future<Output = Result<(), PolicyDataError>>>(
        &self,
        _version: Policy,
        _context: Context,
        _transaction: impl 'static + Send + FnOnce(Policy) -> F,
    ) -> Result<Policy, PolicyDataError> {
        #[allow(unreachable_code)]
        Ok(Policy {
            description: String::from("This is a dummy policy"),
            version:     policy::PolicyVersion {
                creator: None,
                created_at: chrono::DateTime::from_timestamp_nanos(0).into(),
                version: Some(1),
                version_description: String::from("This is a dummy version of a dummy policy"),
                // TODO: Compute hash by hand
                reasoner_connector_context: String::from("No context exists"),
            },
            content:     Vec::new(),
        })
    }

    async fn get_version(&self, _version: i64) -> Result<Policy, PolicyDataError> {
        #[allow(unreachable_code)]
        Ok(Policy {
            description: String::from("This is a dummy policy"),
            version:     policy::PolicyVersion {
                creator: None,
                created_at: chrono::DateTime::from_timestamp_nanos(0).into(),
                version: Some(1),
                version_description: String::from("This is a dummy version of a dummy policy"),
                // TODO: Compute hash by hand
                reasoner_connector_context: String::from("No context exists"),
            },
            content:     Vec::new(),
        })
    }

    async fn get_most_recent(&self) -> Result<Policy, PolicyDataError> {
        #[allow(unreachable_code)]
        Ok(Policy {
            description: String::from("This is a dummy policy"),
            version:     policy::PolicyVersion {
                creator: None::<String>,
                created_at: chrono::DateTime::from_timestamp_nanos(0).into(),
                version: Some(1),
                version_description: String::from("This is a dummy version of a dummy policy"),
                // TODO: Compute hash by hand
                reasoner_connector_context: String::from("No context exists"),
            },
            content:     Vec::new(),
        })
    }

    async fn get_versions(&self) -> Result<Vec<PolicyVersion>, PolicyDataError> {
        #[allow(unreachable_code)]
        Ok(vec![PolicyVersion {
            creator: None,
            created_at: chrono::DateTime::from_timestamp_nanos(0).into(),
            version: Some(1),
            version_description: String::from("This is a dummy version of a dummy policy"),
            // TODO: Compute hash by hand
            reasoner_connector_context: String::from("No context exists"),
        }])
    }

    async fn get_active(&self) -> Result<Policy, PolicyDataError> {
        #[allow(unreachable_code)]
        Ok(Policy {
            description: String::from("This is a dummy policy"),
            version:     policy::PolicyVersion {
                creator: None,
                created_at: chrono::DateTime::from_timestamp_nanos(0).into(),
                version: Some(1),
                version_description: String::from("This is a dummy version of a dummy policy"),
                // TODO: Compute hash by hand
                reasoner_connector_context: String::from("No context exists"),
            },
            content:     Vec::new(),
        })
    }

    async fn set_active<F: 'static + Send + Future<Output = Result<(), PolicyDataError>>>(
        &self,
        _version: i64,
        _context: Context,
        _transaction: impl 'static + Send + FnOnce(Policy) -> F,
    ) -> Result<Policy, PolicyDataError> {
        #[allow(unreachable_code)]
        Ok(Policy {
            description: String::from("This is a dummy policy"),
            version:     policy::PolicyVersion {
                creator: None,
                created_at: chrono::DateTime::from_timestamp_nanos(0).into(),
                version: Some(1),
                version_description: String::from("This is a dummy version of a dummy policy"),
                // TODO: Compute hash by hand
                reasoner_connector_context: String::from("No context exists"),
            },
            content:     Vec::new(),
        })
    }

    async fn deactivate_policy<F: 'static + Send + Future<Output = Result<(), PolicyDataError>>>(
        &self,
        _context: Context,
        _transaction: impl 'static + Send + FnOnce() -> F,
    ) -> Result<(), PolicyDataError> {
        // Nothing to do
        Ok(())
    }
}

/***** ENTRYPOINT *****/
#[tokio::main]
async fn main() {
    let args: Arguments = Arguments::parse();

    let rconn = NoOpReasonerConnector::new();

    // Setup a logger
    if let Err(err) = HumanLogger::terminal(if args.trace { DebugMode::Full } else { DebugMode::Debug }).init() {
        eprintln!("WARNING: Failed to setup logger: {err} (no logging for this session)");
    }

    info!("{} - v{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));

    run_app(args, rconn).await;
}

async fn run_app<R>(args: Arguments, rconn: R)
where
    R: ReasonerConnector<AuditLogPlugin> + Send + Sync + 'static,
{
    // Initialize the plugins
    let log_identifier = format!("{binary} v{version}", binary = env!("CARGO_BIN_NAME"), version = env!("CARGO_PKG_VERSION"));
    let logger: AuditLogPlugin = FileLogger::new(log_identifier, "./audit-log.log");
    let pauthresolver: PolicyAuthResolverPlugin = get_pauth_resolver();
    let dauthresolver: DeliberationAuthResolverPlugin = get_dauth_resolver();
    let pstore: PolicyStorePlugin = DummyPolicyStore {};

    let sresolve: StateResolverPlugin = DummyStateResolver {};

    // Run them!
    let server = Srv::new(args.address, logger, rconn, pstore, sresolve, pauthresolver, dauthresolver);

    server.run().await;
}
