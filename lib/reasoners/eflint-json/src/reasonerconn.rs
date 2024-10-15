//  REASONERCONN.rs
//    by Lut99
//
//  Created:
//    09 Oct 2024, 15:52:06
//  Last edited:
//    11 Oct 2024, 16:56:24
//  Auto updated?
//    Yes
//
//  Description:
//!   Defines a [`ReasonerConnector`] for an eFLINT JSON reasoner.
//

use std::collections::HashMap;
use std::error;
use std::fmt::{Display, Formatter, Result as FResult};
use std::future::Future;
use std::marker::PhantomData;

use eflint_json::spec::auxillary::Version;
use eflint_json::spec::{Phrase, PhraseResult, Request, RequestCommon, RequestPhrases, ResponsePhrases};
use error_trace::{ErrorTrace as _, Trace};
use spec::auditlogger::{AuditLogger, SessionedAuditLogger};
use spec::reasonerconn::{ReasonerConnector, ReasonerResponse};
use tracing::{Level, debug, span};

use crate::reasons::ReasonHandler;
use crate::spec::EFlintable;


/***** ERRORS *****/
/// Defines the errors returned by the [`EFlintJsonReasonerConnectorector`].
#[derive(Debug)]
pub enum Error<R, S, Q> {
    /// Failed to log the reasoner's response to the given logger.
    LogResponse { to: &'static str, err: Trace },
    /// Failed to receive a [`ResponsePhrases`] to the remote reasoner (as raw).
    ReasonerResponse { addr: String, err: reqwest::Error },
    /// Failed to send a [`RequestPhrases`] to the remote reasoner.
    ReasonerRequest { addr: String, err: reqwest::Error },
    /// Failed to extract the reasons for failure (i.e., violations) from a parsed [`ResponsePhrases`] object.
    ResponseExtractReasons { addr: String, raw: String, err: R },
    /// The query returned in the response was of an illegal ending type.
    ResponseIllegalQuery { addr: String, raw: String },
    /// Failed to parse the response of the reasoner as a valid [`ResponsePhrases`] object.
    ResponseParse { addr: String, raw: String, err: serde_json::Error },
    /// Failed to serialize the state to eFLINT.
    StateToEFlint { err: S },
    /// Failed ot serialize the question to eFLINT.
    QuestionToEFlint { err: Q },
}
impl<R, S, Q> Display for Error<R, S, Q> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        use Error::*;
        match self {
            LogResponse { to, .. } => write!(f, "Failed to log the reasoner's response to {to}"),
            ReasonerResponse { addr, .. } => write!(f, "Failed to fetch reply from remote reasoner at {addr:?}"),
            ReasonerRequest { addr, .. } => write!(f, "Failed to set PhrasesRequest to reasoner at {addr:?}"),
            ResponseExtractReasons { addr, raw, .. } => write!(
                f,
                "Failed to extract reasons (i.e., violations) from the response of reasoner at {:?}\n\nParsed response:\n{}\n{}\n{}\n\n",
                addr,
                (0..80).map(|_| '-').collect::<String>(),
                raw,
                (0..80).map(|_| '-').collect::<String>()
            ),
            ResponseIllegalQuery { addr, raw } => write!(
                f,
                "Reasoner at {:?} returned result of instance query as last state change; this is unsupported!\n\nParsed response:\n{}\n{}\n{}\n\n",
                addr,
                (0..80).map(|_| '-').collect::<String>(),
                raw,
                (0..80).map(|_| '-').collect::<String>()
            ),
            ResponseParse { addr, raw, .. } => write!(
                f,
                "Failed to parse response from reasoner at {:?}\n\nRaw response:\n{}\n{}\n{}\n\n",
                addr,
                (0..80).map(|_| '-').collect::<String>(),
                raw,
                (0..80).map(|_| '-').collect::<String>()
            ),
            StateToEFlint { .. } => write!(f, "Failed to serialize given state to eFLINT"),
            QuestionToEFlint { .. } => write!(f, "Failed to serialize given question to eFLINT"),
        }
    }
}
impl<R: 'static + error::Error, S: 'static + error::Error, Q: 'static + error::Error> error::Error for Error<R, S, Q> {
    #[inline]
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        use Error::*;
        match self {
            LogResponse { err, .. } => Some(err),
            ReasonerResponse { err, .. } => Some(err),
            ReasonerRequest { err, .. } => Some(err),
            ResponseExtractReasons { err, .. } => Some(err),
            ResponseIllegalQuery { .. } => None,
            ResponseParse { err, .. } => Some(err),
            StateToEFlint { err } => Some(err),
            QuestionToEFlint { err } => Some(err),
        }
    }
}





/***** AUXILLARY *****/
/// Defines the eFLINT reasoner state to submit to it.
#[derive(Clone, Debug)]
pub struct State<S> {
    /// The policy used.
    pub policy: Vec<Phrase>,
    /// The rest of the state that is appended to the end of the request.
    pub state:  S,
}





/***** LIBRARY *****/
/// Defines the interface to abackend eFLINT JSON reasoner.
#[derive(Clone, Debug)]
pub struct EFlintJsonReasonerConnector<R, S, Q> {
    /// The address where we find the reasoner.
    addr: String,
    /// The reasoner handler that determines if and which reasons to give.
    reason_handler: R,

    /// Dummy variable for remembering which state is being used.
    _state:    PhantomData<S>,
    /// Dummy variable for remembering which question is being used.
    _question: PhantomData<Q>,
}
impl<R, S, Q> EFlintJsonReasonerConnector<R, S, Q> {
    /// Constructor for the EFlintJsonReasonerConnector.
    ///
    /// # Arguments
    /// - `addr`: The address of the remote reasoner that we will connect to.
    /// - `handler`: The [`ReasonHandler`] that determines how errors from the reasoners are propagated to the user.
    ///
    /// # Returns
    /// A new instance of Self, ready for reasoning.
    #[inline]
    pub fn new(addr: impl Into<String>, handler: R) -> Self {
        Self { addr: addr.into(), reason_handler: handler, _state: PhantomData, _question: PhantomData }
    }
}
impl<R, S, Q> ReasonerConnector for EFlintJsonReasonerConnector<R, S, Q>
where
    R: ReasonHandler,
    R::Reason: Display,
    R::Error: 'static,
    S: EFlintable,
    S::Error: 'static,
    Q: EFlintable,
    Q::Error: 'static,
{
    type Error = Error<R::Error, S::Error, Q::Error>;
    type Question = Q;
    type Reason = R::Reason;
    type State = State<S>;

    fn consult<L>(
        &self,
        state: Self::State,
        question: Self::Question,
        logger: &SessionedAuditLogger<L>,
    ) -> impl Future<Output = Result<ReasonerResponse<Self::Reason>, Self::Error>>
    where
        L: AuditLogger,
    {
        async move {
            // NOTE: Using `#[instrument]` adds some unnecessary trait bounds on `S` and such.
            let _span = span!(Level::INFO, "EFlintJsonReasonerConnector::consult", reference = logger.reference()).entered();

            // Build the full policy
            debug!("Building full policy...");
            let mut phrases: Vec<Phrase> = state.policy;
            phrases.extend(state.state.to_eflint().map_err(|err| Error::StateToEFlint { err })?);
            phrases.extend(question.to_eflint().map_err(|err| Error::QuestionToEFlint { err })?);
            debug!("Full request length: {} phrase(s)", phrases.len());

            // Build the request
            let request: Request = Request::Phrases(RequestPhrases {
                common: RequestCommon { version: Version::v0_1_0(), extensions: HashMap::new() },
                phrases,
                updates: true,
            });
            debug!("Full request:\n\n{}\n\n", serde_json::to_string_pretty(&request).unwrap_or_else(|_| "<serialization failure>".into()));

            // Send it on its way
            debug!("Sending eFLINT phrases request to '{}'", self.addr);
            let client = reqwest::Client::new();
            let res = client.post(&self.addr).json(&request).send().await.map_err(|err| Error::ReasonerRequest { addr: self.addr.clone(), err })?;

            debug!("Awaiting response...");
            let raw_body = res.text().await.map_err(|err| Error::ReasonerResponse { addr: self.addr.clone(), err })?;

            debug!("Parsing response...");
            // NOTE: No 'map_err' to avoid moving 'raw_body' out on the happy path
            let response: ResponsePhrases = match serde_json::from_str(&raw_body) {
                Ok(res) => res,
                Err(err) => return Err(Error::ResponseParse { addr: self.addr.clone(), raw: raw_body, err }),
            };

            debug!("Analysing response...");
            // TODO proper handle invalid query and unexpected result
            let verdict: ReasonerResponse<R::Reason> = response
                .results
                .last()
                .map(|r| match r {
                    PhraseResult::BooleanQuery(r) => {
                        if r.result {
                            Ok(ReasonerResponse::Success)
                        } else {
                            Ok(ReasonerResponse::Violated(self.reason_handler.extract_reasons(&response).map_err(|err| {
                                Error::ResponseExtractReasons {
                                    addr: self.addr.clone(),
                                    raw: serde_json::to_string_pretty(&response).unwrap_or_else(|_| "<serialization error>".into()),
                                    err,
                                }
                            })?))
                        }
                    },
                    PhraseResult::InstanceQuery(_) => Err(Error::ResponseIllegalQuery {
                        addr: self.addr.clone(),
                        raw:  serde_json::to_string_pretty(&response).unwrap_or_else(|_| "<serialization error>".into()),
                    }),
                    PhraseResult::StateChange(r) => {
                        if !r.violated {
                            Ok(ReasonerResponse::Success)
                        } else {
                            Ok(ReasonerResponse::Violated(self.reason_handler.extract_reasons(&response).map_err(|err| {
                                Error::ResponseExtractReasons {
                                    addr: self.addr.clone(),
                                    raw: serde_json::to_string_pretty(&response).unwrap_or_else(|_| "<serialization error>".into()),
                                    err,
                                }
                            })?))
                        }
                    },
                })
                .transpose()?
                .unwrap_or(ReasonerResponse::Success);

            // OK, report and return
            logger
                .log_response(&verdict, Some(&raw_body))
                .await
                .map_err(|err| Error::LogResponse { to: std::any::type_name::<SessionedAuditLogger<L>>(), err: err.freeze() })?;
            debug!("Final reasoner verdict: {verdict:?}");
            Ok(verdict)
        }
    }
}
