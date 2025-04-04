use axum::Json;
use axum::response::{IntoResponse, Response};
use http::StatusCode;
use problem_details::ProblemDetails;

#[derive(Debug)]
pub struct Problem(pub ProblemDetails);

impl From<ProblemDetails> for Problem {
    fn from(value: ProblemDetails) -> Self { Problem(value) }
}

impl warp::reject::Reject for Problem {}

impl IntoResponse for Problem {
    fn into_response(self) -> axum::response::Response {
        // FIXME: Does not really sound like a 404, but that is what the old bunch did
        if let Some(status) = self.0.status {
            return (status, Json(self.0)).into_response();
        };

        todo!();
    }
}

enum ErrorSide {
    User,
    Server,
}

// TODO: Should this implement error, or maybe not by design
pub struct ConfidentialError {
    pub source:      Box<dyn std::error::Error>,
    /// Note: It is the callers responsibility to determine how much of the status it is willing to
    /// disclose. Leaving the option empty will default to status code 500.
    pub status_code: Option<StatusCode>,
}

impl IntoResponse for ConfidentialError {
    fn into_response(self) -> Response {
        // TODO: Log the trace instead
        let identifier = uuid::Uuid::new_v4();
        tracing::error!("{err}\nIdentifier: {identifier}", err = self.source);


        Problem(problem_details::ProblemDetails::new().with_status(self.status_code.unwrap_or(StatusCode::INTERNAL_SERVER_ERROR))).into_response()
    }
}
