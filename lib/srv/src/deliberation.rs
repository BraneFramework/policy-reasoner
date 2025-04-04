//  DELIBERATION.rs
//    by Lut99
//
//  Created:
//    09 Jan 2024, 13:45:18
//  Last edited:
//    12 Jun 2024, 17:51:56
//  Auto updated?
//    Yes
//
//  Description:
//!   Implements the deliberation side of the [`Srv`].
//

use std::error::Error;
use std::fmt::{Debug, Display, Formatter, Result as FResult};
use std::sync::Arc;

use audit_logger::{AuditLogger, SessionedConnectorAuditLogger};
use auth_resolver::{AuthContext, AuthResolver};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::post;
use axum::{Extension, Json, Router};
use brane_ast::SymTable;
use deliberation::spec::{
    AccessDataRequest, DataAccessResponse, DeliberationAllowResponse, DeliberationDenyResponse, DeliberationResponse, ExecuteTaskRequest,
    TaskExecResponse, Verdict, WorkflowValidationRequest, WorkflowValidationResponse,
};
use error_trace::ErrorTrace as _;
use log::{debug, error, info};
use policy::{Policy, PolicyDataAccess, PolicyDataError};
use problem_details::ProblemDetails;
use reasonerconn::ReasonerConnector;
use serde::Serialize;
use state_resolver::StateResolver;
use warp::Filter;
use warp::reject::{Reject, Rejection};
use workflow::Workflow;

use crate::Srv;
use crate::problem::Problem;

/***** HELPER FUNCTIONS *****/
/// Retrieves the currently active policy, or immediately denies the request if there is no such policy.
///
/// # Arguments
/// - `logger`: A [`SessionedConnectorAuditLogger`] on which to log the verdict if we deny because no active policy was found.
/// - `reference`: The UUID that the policy expert can use to recognize that this verdict belongs to a particular request, if any.
/// - `policystore`: The story with [`PolicyDataAccess`] from which we'll try to retrieve the active policy.
///
/// # Errors
/// This function may error (= reject the request) if no active policy was found or there was another error trying to retrieve it.
async fn get_active_policy<L: AuditLogger, P: PolicyDataAccess>(
    logger: &L,
    reference: &str,
    policystore: &P,
    // FIXME: This function should not return warp results. It should just let the handler take care of that
) -> Result<Result<Policy, (StatusCode, Json<Verdict>)>, Rejection> {
    // Attempt to get the policy first
    match policystore.get_active().await {
        Ok(policy) => Ok(Ok(policy)),
        Err(PolicyDataError::NotFound) => {
            debug!("Denying incoming request by default (no active policy found)");

            // Create the verdict
            let verdict = Verdict::Deny(DeliberationDenyResponse {
                shared: DeliberationResponse { verdict_reference: reference.into() },
                reasons_for_denial: None,
            });

            // Log it: first, the "actual response" with the reason and then the verdict returned to the user
            logger.log_reasoner_response(reference, "<reasoner not queried because no active policy is present>").await.map_err(|err| {
                // FIXME: I don't think this is debug level
                debug!("Could not log \"reasoner response\" to audit log : {:?} | request id: {}", err, reference);
                warp::reject::custom(err)
            })?;
            logger.log_verdict(reference, &verdict).await.map_err(|err| {
                // FIXME: I don't think this is debug level
                debug!("Could not log verdict to audit log : {:?} | request id: {}", err, reference);
                ConfidentialError(err).into_response();
                warp::reject::custom(err)
            })?;

            // Then send it to the user as promised
            Ok(Err((StatusCode::OK, Json(verdict))))
        },
        Err(PolicyDataError::GeneralError(err)) => {
            error!("Failed to get currently active policy: {err}");
            Err(warp::reject::custom(RejectableString(err)))
        },
    }
}

/***** HELPERS *****/
/// Defines a wrapper around a [`String`] to make it [`Reject`]able.
struct RejectableString(String);
impl Debug for RejectableString {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult { if f.alternate() { write!(f, "{:#?}", self.0) } else { write!(f, "{:?}", self.0) } }
}
impl Reject for RejectableString {}

/// Defines a wrapper around an [`Error`] that also makes it [`Reject`].
#[derive(Debug)]
struct RejectableError<E>(E);
impl<E: Display> Display for RejectableError<E> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult { write!(f, "{}", self.0) }
}
impl<E: Error> Error for RejectableError<E> {
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> { self.0.source() }
}
impl<E: 'static + Debug + Send + Sync> Reject for RejectableError<E> {}

impl<E: 'static + Debug + Send + Sync> IntoResponse for RejectableError<E> {
    fn into_response(self) -> axum::response::Response {
        // FIXME: This does not seem to match any recovery filter.
        // This would mean we would return an "Got err: " followed, by the error, but I cannot
        // imagine that we actually want to do so
        todo!()
    }
}

/***** IMPLEMENTATION *****/
impl<L, C, P, S, PA, DA> Srv<L, C, P, S, PA, DA>
where
    L: 'static + AuditLogger + Send + Sync + Clone,
    C: 'static + ReasonerConnector<L> + Send + Sync,
    P: 'static + PolicyDataAccess + Send + Sync,
    S: 'static + StateResolver + Send + Sync,
    PA: 'static + AuthResolver + Send + Sync,
    DA: 'static + AuthResolver + Send + Sync,
    C::Context: Send + Sync + Debug + Serialize,
{
    pub fn deliberation_routes(this: Arc<Self>) -> Router {
        // let authentication_middleware = axum::middleware::from_fn_with_state();

        let router = Router::new()
            .route("execute-task", post(Self::handle_execute_task_request))
            .route("access-data", post(Self::handle_access_data_request))
            .route("execute-workflow", post(Self::handle_validate_workflow_request))

            // .layer(authentication_middleware)
            .with_state(this);

        router
    }

    // POST /v1/deliberation/execute-task
    async fn handle_execute_task_request(
        Extension(auth_ctx): Extension<AuthContext>,
        State(state): State<Arc<Self>>,
        Json(body): Json<ExecuteTaskRequest>,
    ) -> Result<(StatusCode, Json<Verdict>), Problem> {
        info!("Handling exec-task request");
        let ExecuteTaskRequest { use_case, workflow, task_id } = body;
        let verdict_reference: String = uuid::Uuid::new_v4().into();

        // First, resolve the task ID in the workflow to the ProgramCounter ID needed for `task_id` below (and before we pass it by ownership to be converted)
        debug!("Compiling WIR workflow to Checker Workflow...");
        let task_pc: String = task_id.resolved(&workflow.table).to_string();

        // Read the body's workflow as a Checker Workflow
        let workflow: Workflow = match Workflow::try_from(workflow) {
            Ok(workflow) => workflow,
            Err(err) => {
                return Err(ProblemDetails::new().with_status(StatusCode::BAD_REQUEST).with_detail(&err.to_string()).into());
            },
        };
        // Get the task ID based on the request's target ID
        let task_id = format!("{}-{}-task", workflow.id, task_pc);
        debug!("Considering task '{}' in workflow '{}'", task_id, workflow.id);

        debug!("Retrieving state...");
        let state = match state.stateresolver.get_state(use_case).await {
            Ok(state) => state,
            Err(err) => {
                error!("Could not retrieve state: {err} | request id: {verdict_reference}");
                // FIXME: Why not just use a Problem? The entire rejection path is kinda lost on me
                // here.
                return Err(RejectableError(err));
            },
        };
        debug!(
            "Got state with {} datasets, {} functions, {} locations and {} users",
            state.datasets.len(),
            state.functions.len(),
            state.locations.len(),
            state.users.len()
        );

        let verdict_reference: String = uuid::Uuid::new_v4().into();
        debug!("Generated verdict_reference: {}", verdict_reference);

        debug!("Retrieving active policy...");
        let policy: Policy = match get_active_policy(&this.logger, &verdict_reference, &this.policystore).await? {
            Ok(policy) => policy,
            Err(err) => return Ok(err),
        };
        // let policy = this.policystore.get_active().await.unwrap();
        debug!("Got policy with {} bodies", policy.content.len());

        state
            .logger
            .log_exec_task_request(&verdict_reference, &auth_ctx, policy.version.version.unwrap(), &state, &workflow, &task_id)
            .await
            .map_err(|err| {
                debug!("Could not log exec task request to audit log : {:?} | request id: {}", err, verdict_reference);
                warp::reject::custom(err)
            })?;

        debug!("Consulting reasoner connector...");

        match this
            .reasonerconn
            .execute_task(SessionedConnectorAuditLogger::new(verdict_reference.clone(), state.logger.clone()), policy, state, workflow, task_id)
            .await
        {
            Ok(v) => {
                let resp: Verdict = if !v.success {
                    Verdict::Deny(DeliberationDenyResponse {
                        shared: TaskExecResponse { verdict_reference: verdict_reference.clone() },
                        reasons_for_denial: Some(v.errors),
                    })
                } else {
                    Verdict::Allow(DeliberationAllowResponse {
                        shared:    TaskExecResponse { verdict_reference: verdict_reference.clone() },
                        // TODO implement signature
                        signature: "signature".into(),
                    })
                };

                this.logger.log_verdict(&verdict_reference, &resp).await.map_err(|err| {
                    debug!("Could not log execute task verdict to audit log : {:?} | request id: {}", err, verdict_reference);
                    warp::reject::custom(err)
                })?;

                Ok(warp::reply::with_status(warp::reply::json(&resp), warp::hyper::StatusCode::OK))
            },
            Err(err) => Ok(warp::reply::with_status(warp::reply::json(&format!("{}", err)), warp::hyper::StatusCode::OK)),
        }
    }

    // POST /v1/deliberation/access-data
    async fn handle_access_data_request(
        auth_ctx: AuthContext,
        this: Arc<Self>,
        body: AccessDataRequest,
    ) -> Result<warp::reply::WithStatus<warp::reply::Json>, warp::reject::Rejection> {
        info!("Handling access-data request");
        let AccessDataRequest { use_case, workflow, data_id, task_id } = body;

        let verdict_reference: String = uuid::Uuid::new_v4().into();

        debug!("Compiling WIR workflow to Checker Workflow...");

        // Read the body's workflow as a Checker Workflow
        // NOTE: We need the deep clone of the table here to ensure that the `Arc` in the WIR is not duplicated. Nice design, Tim!
        let table: SymTable = (*workflow.table).clone();
        let workflow: Workflow = match Workflow::try_from(workflow) {
            Ok(workflow) => workflow,
            Err(err) => {
                return Ok(warp::reply::with_status(warp::reply::json(&err.trace().to_string()), warp::hyper::StatusCode::BAD_REQUEST));
            },
        };

        debug!("Retrieving state...");
        let state = match this.stateresolver.get_state(use_case).await {
            Ok(state) => state,
            Err(err) => {
                error!("Could not retrieve state: {err} | request id: {verdict_reference}");
                return Err(warp::reject::custom(RejectableError(err)));
            },
        };
        debug!(
            "Got state with {} datasets, {} functions, {} locations and {} users",
            state.datasets.len(),
            state.functions.len(),
            state.locations.len(),
            state.users.len()
        );

        debug!("Retrieving active policy...");
        let policy = match get_active_policy(&this.logger, &verdict_reference, &this.policystore).await? {
            Ok(policy) => policy,
            Err(err) => return Ok(err),
        };
        debug!("Got policy with {} bodies", policy.content.len());

        let task_id: Option<String> = match task_id {
            Some(task_id) => {
                // First, resolve the task ID in the workflow to the ProgramCounter ID needed for `task_id` below (and before we pass it by ownership to be converted)
                let task_pc: String = task_id.resolved(&table).to_string();

                // Get the task ID based on the request's target ID
                let task_id = format!("{}-{}-task", workflow.id, task_pc);
                debug!("Considering task '{}' in workflow '{}'", task_id, workflow.id);
                Some(task_id)
            },
            None => None,
        };

        debug!("Retrieving active policy...");
        let policy = match this.policystore.get_active().await {
            Ok(p) => p,
            Err(_) => {
                let resp = Verdict::Deny(DeliberationDenyResponse {
                    shared: DataAccessResponse { verdict_reference: verdict_reference.clone() },
                    reasons_for_denial: vec![].into(),
                });

                this.logger.log_data_access_request(&verdict_reference, &auth_ctx, -1, &state, &workflow, &data_id, &task_id).await.map_err(
                    |err| {
                        debug!("Could not log data access request to audit log : {:?} | request id: {}", err, verdict_reference);
                        warp::reject::custom(err)
                    },
                )?;

                this.logger.log_verdict(&verdict_reference, &resp).await.map_err(|err| {
                    debug!("Could not log data access verdict to audit log : {:?} | request id: {}", err, verdict_reference);
                    warp::reject::custom(err)
                })?;

                return Ok(warp::reply::with_status(warp::reply::json(&resp), warp::hyper::StatusCode::OK));
            },
        };
        debug!("Got policy with {} bodies", policy.content.len());

        this.logger
            .log_data_access_request(&verdict_reference, &auth_ctx, policy.version.version.unwrap(), &state, &workflow, &data_id, &task_id)
            .await
            .map_err(|err| {
                debug!("Could not log data access request to audit log : {:?} | request id: {}", err, verdict_reference);
                warp::reject::custom(err)
            })?;

        debug!("Consulting reasoner connector...");

        match this
            .reasonerconn
            .access_data_request(
                SessionedConnectorAuditLogger::new(verdict_reference.clone(), this.logger.clone()),
                policy,
                state,
                workflow,
                data_id,
                task_id,
            )
            .await
        {
            Ok(v) => {
                let resp: Verdict = if !v.success {
                    Verdict::Deny(DeliberationDenyResponse {
                        shared: DataAccessResponse { verdict_reference: verdict_reference.clone() },
                        reasons_for_denial: Some(v.errors),
                    })
                } else {
                    Verdict::Allow(DeliberationAllowResponse {
                        shared:    DataAccessResponse { verdict_reference: verdict_reference.clone() },
                        // TODO implement signature
                        signature: "signature".into(),
                    })
                };

                this.logger.log_verdict(&verdict_reference, &resp).await.map_err(|err| {
                    debug!("Could not log data access verdict to audit log : {:?} | request id: {}", err, verdict_reference);
                    warp::reject::custom(err)
                })?;

                Ok(warp::reply::with_status(warp::reply::json(&resp), warp::hyper::StatusCode::OK))
            },
            Err(err) => Ok(warp::reply::with_status(warp::reply::json(&format!("{}", err)), warp::hyper::StatusCode::OK)),
        }
    }

    // POST /v1/deliberation/validate-workflow
    async fn handle_validate_workflow_request(
        auth_ctx: AuthContext,
        this: Arc<Self>,
        body: WorkflowValidationRequest,
    ) -> Result<warp::reply::WithStatus<warp::reply::Json>, warp::reject::Rejection> {
        info!("Handling validate request");
        let WorkflowValidationRequest { use_case, workflow } = body;

        let verdict_reference: String = uuid::Uuid::new_v4().into();

        debug!("Compiling WIR workflow to Checker Workflow...");
        // Read the body's workflow as a Checker Workflow
        let workflow: Workflow = match Workflow::try_from(workflow) {
            Ok(workflow) => workflow,
            Err(err) => {
                return Ok(warp::reply::with_status(warp::reply::json(&err.to_string()), warp::hyper::StatusCode::BAD_REQUEST));
            },
        };

        debug!("Retrieving state...");
        let state = match this.stateresolver.get_state(use_case).await {
            Ok(state) => state,
            Err(err) => {
                error!("Could not retrieve state: {err} | request id: {verdict_reference}");
                return Err(warp::reject::custom(RejectableError(err)));
            },
        };
        debug!(
            "Got state with {} datasets, {} functions, {} locations and {} users",
            state.datasets.len(),
            state.functions.len(),
            state.locations.len(),
            state.users.len()
        );

        let verdict_reference: String = uuid::Uuid::new_v4().into();
        debug!("Generated verdict_reference: {}", verdict_reference);

        debug!("Retrieving active policy...");
        let policy = match get_active_policy(&this.logger, &verdict_reference, &this.policystore).await? {
            Ok(policy) => policy,
            Err(err) => return Ok(err),
        };
        debug!("Got policy with {} bodies", policy.content.len());

        this.logger.log_validate_workflow_request(&verdict_reference, &auth_ctx, policy.version.version.unwrap(), &state, &workflow).await.map_err(
            |err| {
                debug!("Could not log validate workflow request to audit log : {:?} | request id: {}", err, verdict_reference);
                warp::reject::custom(err)
            },
        )?;

        debug!("Consulting reasoner connector...");

        match this
            .reasonerconn
            .workflow_validation_request(SessionedConnectorAuditLogger::new(verdict_reference.clone(), this.logger.clone()), policy, state, workflow)
            .await
        {
            Ok(v) => {
                let resp: Verdict = if !v.success {
                    Verdict::Deny(DeliberationDenyResponse {
                        shared: WorkflowValidationResponse { verdict_reference: verdict_reference.clone() },
                        reasons_for_denial: Some(v.errors),
                    })
                } else {
                    Verdict::Allow(DeliberationAllowResponse {
                        shared:    WorkflowValidationResponse { verdict_reference: verdict_reference.clone() },
                        // TODO implement signature
                        signature: "signature".into(),
                    })
                };

                this.logger.log_verdict(&verdict_reference, &resp).await.map_err(|err| {
                    debug!("Could not log workflow validation verdict to audit log : {:?} | request id: {}", err, verdict_reference);
                    warp::reject::custom(err)
                })?;

                Ok(warp::reply::with_status(warp::reply::json(&resp), warp::hyper::StatusCode::OK))
            },
            Err(err) => Ok(warp::reply::with_status(warp::reply::json(&format!("{}", err)), warp::hyper::StatusCode::OK)),
        }
    }

    pub fn with_deliberation_api_auth(this: Arc<Self>) -> impl Filter<Extract = (AuthContext,), Error = warp::Rejection> + Clone {
        Self::with_self(this.clone()).and(warp::header::headers_cloned()).and_then(|this: Arc<Self>, headers| async move {
            match this.dauthresolver.authenticate(headers).await {
                Ok(v) => Ok(v),
                Err(err) => Err(warp::reject::custom(err)),
            }
        })
    }
}
