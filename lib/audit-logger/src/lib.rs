use std::borrow::Cow;
use std::collections::hash_map::DefaultHasher;
use std::fmt::Debug;
use std::hash::{Hash as _, Hasher as _};

use auth_resolver::AuthContext;
use deliberation::spec::Verdict;
use enum_debug::EnumDebug;
use policy::Policy;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use state_resolver::State;
use workflow::Workflow;

#[derive(Debug)]
pub enum Error {
    CouldNotDeliver(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CouldNotDeliver(msg) => {
                write!(f, "Could not deliver: {}", msg)
            },
        }
    }
}

impl std::error::Error for Error {}

impl warp::reject::Reject for Error {}

pub trait ConnectorContext {
    fn r#type(&self) -> String;
    fn version(&self) -> String;
}

/// Defines an intermediary that allows us to conveniently log `ReasonerConnector`'s context.
pub trait ConnectorWithContext {
    /// The type returned by [`ReasonerConnector::full_context()`].
    type Context: ConnectorContext + std::hash::Hash + Sync + Send + Serialize + Clone + core::fmt::Debug;

    /// Returns hash of connector's context.
    ///
    /// If anything about the connector changes that can have an effect on the evaluation of a policy
    /// the returned hash must be different
    fn hash() -> String {
        let mut hasher = DefaultHasher::new();
        Self::context().hash(&mut hasher);
        // digest()
        let h = hasher.finish();
        hex::encode(h.to_be_bytes())
    }
    /// Returns so-called "full context" about the reasoner connector that is relevant for the audit log.
    ///
    /// In particular, this should al least contain the type of the connector used and its version.
    fn context() -> Self::Context;
}

/// Collects everything we might want to log in an [`AuditLogger`].
///
/// TODO: Nicer to move this to the toplevel crate, `logger.rs`.
#[derive(Clone, Debug, Deserialize, EnumDebug, Serialize)]
#[serde(tag = "kind", rename_all = "SCREAMING-KEBAB-CASE")]
pub enum LogStatement<'a> {
    /// A request that asks if a task may be executed has been received.
    ExecuteTask {
        reference: Cow<'a, str>,
        auth:      Cow<'a, AuthContext>,
        policy:    i64,
        state:     Cow<'a, State>,
        workflow:  Cow<'a, Workflow>,
        task:      Cow<'a, str>,
    },
    /// A request that asks if an asset may be accessed has been received.
    AssetAccess {
        reference: Cow<'a, str>,
        auth:      Cow<'a, AuthContext>,
        policy:    i64,
        state:     Cow<'a, State>,
        workflow:  Cow<'a, Workflow>,
        data:      Cow<'a, str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        task:      Option<Cow<'a, str>>,
    },
    /// A request that asks if a workflow is permitted has been received.
    WorkflowValidate {
        reference: Cow<'a, str>,
        auth:      Cow<'a, AuthContext>,
        policy:    i64,
        state:     Cow<'a, State>,
        workflow:  Cow<'a, Workflow>,
    },

    /// Logs the raw response of a reasoner.
    ReasonerResponse { reference: Cow<'a, str>, response: Cow<'a, str> },
    /// Logs the official response of a reasoner.
    ReasonerVerdict { reference: Cow<'a, str>, verdict: Cow<'a, Verdict> },

    /// Logs the reasoner backend for during startup.
    ReasonerContext { connector_context: Value, connector_context_hash: String },
    /// Logs the arrival of a new policy.
    PolicyAdd { auth: Cow<'a, AuthContext>, connector_context_hash: String, policy: Cow<'a, Policy> },
    /// Logs the activation of an existing policy.
    PolicyActivate { auth: Cow<'a, AuthContext>, policy: Cow<'a, Policy> },
    /// Logs the deactivation of the current active policy.
    PolicyDeactivate { auth: Cow<'a, AuthContext> },
}
impl<'a> LogStatement<'a> {
    /// Constructor for a [`LogStatement::ExecuteTask`] that makes it a bit more convenient to initialize.
    ///
    /// # Arguments
    /// - `reference`: The reference ID for this request.
    /// - `auth`: The [`AuthContext`] that explains who performed the request.
    /// - `policy`: The [`Policy`] used to evaluate the request.
    /// - `state`: The [`State`] given to the policy for evaluation.
    /// - `workflow`: The [`Workflow`] that is being evaluated.
    /// - `task`: Which task in the `workflow` we're specifically considering.
    ///
    /// # Returns
    /// A new [`LogStatement::ExecuteTask`] that is initialized with the given properties.
    #[inline]
    pub fn execute_task(reference: &'a str, auth: &'a AuthContext, policy: i64, state: &'a State, workflow: &'a Workflow, task: &'a str) -> Self {
        Self::ExecuteTask {
            reference: Cow::Borrowed(reference),
            auth: Cow::Borrowed(auth),
            policy,
            state: Cow::Borrowed(state),
            workflow: Cow::Borrowed(workflow),
            task: Cow::Borrowed(task),
        }
    }

    /// Constructor for a [`LogStatement::AssetAccess`] that makes it a bit more convenient to initialize.
    ///
    /// # Arguments
    /// - `reference`: The reference ID for this request.
    /// - `auth`: The [`AuthContext`] that explains who performed the request.
    /// - `policy`: The [`Policy`] used to evaluate the request.
    /// - `state`: The [`State`] given to the policy for evaluation.
    /// - `workflow`: The [`Workflow`] that is being evaluated.
    /// - `task`: Which task in the `workflow` we're specifically considering.
    ///
    /// # Returns
    /// A new [`LogStatement::AssetAccess`] that is initialized with the given properties.
    #[inline]
    pub fn asset_access(
        reference: &'a str,
        auth: &'a AuthContext,
        policy: i64,
        state: &'a State,
        workflow: &'a Workflow,
        data: &'a str,
        task: &'a Option<String>,
    ) -> Self {
        Self::AssetAccess {
            reference: Cow::Borrowed(reference),
            auth: Cow::Borrowed(auth),
            policy,
            state: Cow::Borrowed(state),
            workflow: Cow::Borrowed(workflow),
            data: Cow::Borrowed(data),
            task: task.as_ref().map(|t| Cow::Borrowed(t.as_str())),
        }
    }

    /// Constructor for a [`LogStatement::WorkflowValidate`] that makes it a bit more convenient to initialize.
    ///
    /// # Arguments
    /// - `reference`: The reference ID for this request.
    /// - `auth`: The [`AuthContext`] that explains who performed the request.
    /// - `policy`: The [`Policy`] used to evaluate the request.
    /// - `state`: The [`State`] given to the policy for evaluation.
    /// - `workflow`: The [`Workflow`] that is being evaluated.
    /// - `task`: Which task in the `workflow` we're specifically considering.
    ///
    /// # Returns
    /// A new [`LogStatement::WorkflowValidate`] that is initialized with the given properties.
    #[inline]
    pub fn workflow_validate(reference: &'a str, auth: &'a AuthContext, policy: i64, state: &'a State, workflow: &'a Workflow) -> Self {
        Self::WorkflowValidate {
            reference: Cow::Borrowed(reference),
            auth: Cow::Borrowed(auth),
            policy,
            state: Cow::Borrowed(state),
            workflow: Cow::Borrowed(workflow),
        }
    }

    /// Constructor for a [`LogStatement::ReasonerResponse`] that makes it a bit more convenient to initialize.
    ///
    /// # Arguments
    /// - `reference`: The reference ID for this request.
    /// - `response`: The raw response as returned by the reasoner.
    ///
    /// # Returns
    /// A new [`LogStatement::ReasonerResponse`] that is initialized with the given properties.
    #[inline]
    pub fn reasoner_response(reference: &'a str, response: &'a str) -> Self {
        Self::ReasonerResponse { reference: Cow::Borrowed(reference), response: Cow::Borrowed(response) }
    }

    /// Constructor for a [`LogStatement::ReasonerVerdict`] that makes it a bit more convenient to initialize.
    ///
    /// # Arguments
    /// - `reference`: The reference ID for this request.
    /// - `verdict`: The verdict given by the reasoner.
    ///
    /// # Returns
    /// A new [`LogStatement::ReasonerVerdict`] that is initialized with the given properties.
    #[inline]
    pub fn reasoner_verdict(reference: &'a str, verdict: &'a Verdict) -> Self {
        Self::ReasonerVerdict { reference: Cow::Borrowed(reference), verdict: Cow::Borrowed(verdict) }
    }

    /// Constructor for a [`LogStatement::ReasonerContext`] that makes it a bit more convenient to initialize.
    ///
    /// # Arguments
    /// - `context`: The context that is used to give answers with this reasoner.
    ///
    /// # Returns
    /// A new [`LogStatement::ReasonerContext`] that is initialized with the given properties.
    #[inline]
    pub fn reasoner_context<C: ConnectorWithContext>() -> Self {
        Self::ReasonerContext {
            connector_context:      serde_json::to_value(C::context())
                .unwrap_or_else(|err| panic!("Could not serialize context of {}: {}", std::any::type_name::<C>(), err)),
            connector_context_hash: C::hash(),
        }
    }

    /// Constructor for a [`LogStatement::PolicyAdd`] that makes it a bit more convenient to initialize.
    ///
    /// # Arguments
    /// - `auth`: The [`AuthContext`] that explains who performed the request.
    /// - `context`: The context that is used to give answers with this reasoner.
    /// - `policy`: The [`Policy`] added to the checker in this request.
    ///
    /// # Returns
    /// A new [`LogStatement::ReasonerContext`] that is initialized with the given properties.
    #[inline]
    pub fn policy_add<C: ConnectorWithContext>(auth: &'a AuthContext, policy: &'a Policy) -> Self {
        Self::PolicyAdd { auth: Cow::Borrowed(auth), connector_context_hash: C::hash(), policy: Cow::Borrowed(policy) }
    }

    /// Constructor for a [`LogStatement::PolicyActivate`] that makes it a bit more convenient to initialize.
    ///
    /// # Arguments
    /// - `auth`: The [`AuthContext`] that explains who performed the request.
    /// - `policy`: The [`Policy`] that got activated in this request.
    ///
    /// # Returns
    /// A new [`LogStatement::PolicyActivate`] that is initialized with the given properties.
    #[inline]
    pub fn policy_activate(auth: &'a AuthContext, policy: &'a Policy) -> Self {
        Self::PolicyActivate { auth: Cow::Borrowed(auth), policy: Cow::Borrowed(policy) }
    }

    /// Constructor for a [`LogStatement::PolicyDeactivate`] that makes it a bit more convenient to initialize.
    ///
    /// # Arguments
    /// - `auth`: The [`AuthContext`] that explains who performed the request.
    ///
    /// # Returns
    /// A new [`LogStatement::PolicyDeactivate`] that is initialized with the given properties.
    #[inline]
    pub fn policy_deactivate(auth: &'a AuthContext) -> Self { Self::PolicyDeactivate { auth: Cow::Borrowed(auth) } }
}

#[async_trait::async_trait]
pub trait AuditLogger: ReasonerConnectorAuditLogger {
    async fn log_exec_task_request(
        &self,
        reference: &str,
        auth: &AuthContext,
        policy: i64,
        state: &State,
        workflow: &Workflow,
        task: &str,
    ) -> Result<(), Error>;

    // OK so it _may_ have a few functions. But again, not worth it to make a struct IMO.
    #[allow(clippy::too_many_arguments)]
    async fn log_data_access_request(
        &self,
        reference: &str,
        auth: &AuthContext,
        policy: i64,
        state: &State,
        workflow: &Workflow,
        data: &str,
        task: &Option<String>,
    ) -> Result<(), Error>;

    async fn log_validate_workflow_request(
        &self,
        reference: &str,
        auth: &AuthContext,
        policy: i64,
        state: &State,
        workflow: &Workflow,
    ) -> Result<(), Error>;

    async fn log_verdict(&self, reference: &str, verdict: &Verdict) -> Result<(), Error>;

    /// Dumps the full context of the reasoner on startup.
    ///
    /// Note that it's recommended to use `ReasonerConnector::FullContext` for this, to include the full base specification.
    async fn log_reasoner_context<C: ConnectorWithContext>(&self) -> Result<(), Error>;
    /// Logs that a new policy has been added, including the full policy.
    ///
    /// Note that it's recommended to use `ReasonerConnector::Context` for this, as the full base spec as already been logged at startup.
    async fn log_add_policy_request<C: ConnectorWithContext>(&self, auth: &AuthContext, policy: &Policy) -> Result<(), Error>;

    async fn log_set_active_version_policy(&self, auth: &AuthContext, policy: &Policy) -> Result<(), Error>;

    async fn log_deactivate_policy(&self, auth: &AuthContext) -> Result<(), Error>;
}

#[async_trait::async_trait]
pub trait ReasonerConnectorAuditLogger {
    async fn log_reasoner_response(&self, reference: &str, response: &str) -> Result<(), Error>;
}

pub struct SessionedConnectorAuditLogger<Logger: ReasonerConnectorAuditLogger> {
    pub reference: String,
    logger: Logger,
}
impl<Logger: ReasonerConnectorAuditLogger> SessionedConnectorAuditLogger<Logger> {
    pub fn new(reference: String, logger: Logger) -> Self { Self { reference, logger } }

    pub async fn log_reasoner_response(&self, response: &str) -> Result<(), Error> {
        self.logger.log_reasoner_response(&self.reference, response).await
    }
}
