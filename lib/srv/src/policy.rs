use std::fmt::Debug;
use std::sync::Arc;

use audit_logger::AuditLogger;
use auth_resolver::{AuthContext, AuthResolver};
use axum::extract::{self, Request, State};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post, put};
use axum::{Extension, Json, Router};
use policy::{Context, PolicyDataAccess, PolicyDataError};
use problem_details::ProblemDetails;
use reasonerconn::ReasonerConnector;
use serde::Serialize;
use state_resolver::StateResolver;

use crate::problem::{ConfidentialError, Problem};
use crate::{Srv, models};

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
    /// Register the routes and return the corresponding (sub)router
    ///
    /// Note: This router does not contain any prefix and should probably be nested into a base
    /// router
    pub fn policy_routes(this: Arc<Self>) -> Router {
        let authentication_middleware = axum::middleware::from_fn_with_state(this.clone(), Self::policy_auth_middleware);
        let router = Router::new()
            .route("/", get(Self::get_all_policies_handler))
            .route("/", post(Self::add_policy_handler))
            .route("/{version}", get(Self::get_policy_version_handler))
            .route("/active", get(Self::get_active_policy_handler))
            .route("/active", put(Self::set_active_policy_handler))
            .route("/active", delete(Self::deactivate_policy_handler))
            .layer(authentication_middleware)
            .with_state(this);

        router
    }

    /// Get specific version
    /// Associated endpoint: GET /v1/policies/:version
    /// Response:
    /// - 200 Policy
    /// - 400 problem+json
    /// - 404 problem+json
    async fn get_policy_version_handler(
        // Note: Should this really be an signed integer?
        extract::Path(version): extract::Path<i64>,
        State(state): State<Arc<Self>>,
    ) -> Result<Json<policy::Policy>, Problem> {
        match state.policystore.get_version(version).await {
            Ok(v) => Ok(Json(v)),
            Err(PolicyDataError::NotFound) => Err(ProblemDetails::new().with_status(axum::http::StatusCode::NOT_FOUND).into()),
            Err(PolicyDataError::GeneralError(msg)) => {
                Err(ProblemDetails::new().with_status(axum::http::StatusCode::BAD_REQUEST).with_detail(msg).into())
            },
        }
    }

    /// List policy's versions
    /// Associated endpoint: GET /v1/policies
    /// Response:
    /// - 200 Vec<PolicyVersionDescription>
    /// - 400 problem+json
    /// - 404 problem+json
    async fn get_all_policies_handler(
        Extension(_auth_ctx): Extension<AuthContext>,
        State(state): State<Arc<Self>>,
    ) -> Result<Json<Vec<policy::PolicyVersion>>, Problem> {
        match state.policystore.get_versions().await {
            Ok(v) => Ok(Json(v)),
            Err(PolicyDataError::NotFound) => Err(ProblemDetails::new().with_status(axum::http::StatusCode::NOT_FOUND).into()),
            Err(PolicyDataError::GeneralError(msg)) => {
                Err(ProblemDetails::new().with_status(axum::http::StatusCode::BAD_REQUEST).with_detail(msg).into())
            },
        }
    }

    /// Create new version of policy
    /// Associated endpoint: POST /v1/policies
    /// Request: Policy
    /// Response:
    ///  - 201 Policy
    ///    Returned policy has version in body
    ///  - 400 problem+json
    ///  - 404 problem+json
    async fn add_policy_handler(
        Extension(auth_ctx): Extension<AuthContext>,
        State(state): State<Arc<Self>>,
        Json(body): Json<models::AddPolicyPostModel>,
    ) -> Result<Json<policy::Policy>, Problem> {
        let t: Arc<Self> = state.clone();
        let mut model = body.to_domain();
        model.version.reasoner_connector_context = C::hash();
        match state
            .policystore
            .add_version(model, Context { initiator: auth_ctx.initiator.clone() }, |policy| async move {
                t.logger.log_add_policy_request::<C>(&auth_ctx, &policy).await.map_err(|err| match err {
                    audit_logger::Error::CouldNotDeliver(err) => PolicyDataError::GeneralError(err),
                })
            })
            .await
        {
            Ok(policy) => Ok(Json(policy)),
            Err(PolicyDataError::NotFound) => Err(ProblemDetails::new().with_status(axum::http::StatusCode::NOT_FOUND).into()),
            Err(PolicyDataError::GeneralError(msg)) => {
                Err(ProblemDetails::new().with_status(axum::http::StatusCode::BAD_REQUEST).with_detail(msg).into())
            },
        }
    }

    /// Show active policy
    /// Associated endpoint: GET /v1/policies/active
    /// Response:
    ///  - 200 {version: string}
    ///  - 400 problem+json
    ///  - 404 problem+json
    async fn get_active_policy_handler(
        Extension(_auth_ctx): Extension<AuthContext>,
        State(state): State<Arc<Self>>,
    ) -> Result<Json<policy::Policy>, Problem> {
        match state.policystore.get_active().await {
            Ok(v) => Ok(Json(v)),
            Err(PolicyDataError::NotFound) => {
                Err(ProblemDetails::new().with_status(axum::http::StatusCode::NOT_FOUND).with_detail("No version currently active").into())
            },
            Err(PolicyDataError::GeneralError(msg)) => {
                Err(ProblemDetails::new().with_status(axum::http::StatusCode::BAD_REQUEST).with_detail(msg).into())
            },
        }
    }

    /// Set active policy
    /// Associated endpoint: PUT /v1/policies/active
    /// Request body: {version: string}
    /// Response:
    ///  - 200 {version: string}
    ///  - 400 problem+json
    async fn set_active_policy_handler<'a>(
        Extension(auth_ctx): Extension<AuthContext>,
        State(state): State<Arc<Self>>,
        Json(body): Json<models::SetVersionPostModel>,
        // FIXME: Cannot really return Json<Policy> as the copy is too expensive
        // The vec inside policy should be Arc-ed to avoid unnecessarily deep cloning
    ) -> Result<Json<policy::Policy>, Problem> {
        // Reject activation of policy with invalid base defs
        let conn_hash = C::hash();

        if let Ok(policy) = state.policystore.get_version(body.version).await {
            if policy.version.reasoner_connector_context != conn_hash {
                return Err(ProblemDetails::new()
                    .with_status(axum::http::StatusCode::BAD_REQUEST)
                    .with_detail(format!(
                        "Cannot activate policy which has a different base policy than current the reasoners connector's base. Policy base defs \
                         hash is '{}' and connector's base defs hash is '{}'",
                        policy.version.reasoner_connector_context, conn_hash
                    ))
                    .into());
            }
        }

        let t = state.clone();
        match state
            .policystore
            .set_active(body.version, Context { initiator: auth_ctx.initiator.clone() }, |policy| async move {
                t.logger.log_set_active_version_policy(&auth_ctx, &policy).await.map_err(|err| match err {
                    audit_logger::Error::CouldNotDeliver(err) => PolicyDataError::GeneralError(err),
                })
            })
            .await
        {
            Ok(policy) => Ok(Json(policy)),
            Err(PolicyDataError::NotFound) => Err(ProblemDetails::new()
                .with_status(axum::http::StatusCode::BAD_REQUEST)
                .with_detail(format!("Invalid version: {}", body.version))
                .into()),
            Err(PolicyDataError::GeneralError(msg)) => {
                Err(ProblemDetails::new().with_status(axum::http::StatusCode::BAD_REQUEST).with_detail(msg).into())
            },
        }
    }

    /// Deactivate the current policy
    /// Associated endpoint: DELETE /v1/policies/active
    /// Response:
    ///  - 200
    ///  - 400 problem+json
    async fn deactivate_policy_handler(Extension(auth_ctx): Extension<AuthContext>, State(state): State<Arc<Self>>) -> Result<Json<()>, Problem> {
        let t = state.clone();
        match state
            .policystore
            .deactivate_policy(Context { initiator: auth_ctx.initiator.clone() }, || async move {
                t.logger.log_deactivate_policy(&auth_ctx).await.map_err(|err| match err {
                    audit_logger::Error::CouldNotDeliver(err) => PolicyDataError::GeneralError(err),
                })
            })
            .await
        {
            Ok(()) => Ok(Json(())),
            Err(PolicyDataError::NotFound) => {
                // TODO: Consider returning a 404 instead
                Err(ProblemDetails::new().with_status(axum::http::StatusCode::BAD_REQUEST).with_detail("No active version to deactivate").into())
            },
            Err(PolicyDataError::GeneralError(msg)) => {
                Err(ProblemDetails::new().with_status(axum::http::StatusCode::BAD_REQUEST).with_detail(msg).into())
            },
        }
    }

    /// Authenitcation middleware
    ///
    /// This function checks if the
    /// FIXME: Split out return type into a Result
    /// But first we need to figure out the details of this ConfidentialError type
    async fn policy_auth_middleware(State(state): State<Arc<Self>>, mut req: Request, next: Next) -> Response {
        let headers = req.headers();

        let auth_ctx = match state.pauthresolver.authenticate(headers).await {
            Ok(x) => x,
            Err(source) => return ConfidentialError(Box::new(source)).into_response(),
        };

        req.extensions_mut().insert(auth_ctx);

        next.run(req).await
    }
}
