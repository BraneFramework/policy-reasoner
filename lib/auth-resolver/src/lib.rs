use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct AuthContext {
    pub initiator: String,
    pub system:    String,
}

#[derive(Debug)]
pub struct AuthResolverError {
    err: String,
}

impl AuthResolverError {
    pub fn new(err: String) -> Self { Self { err } }
}

impl std::fmt::Display for AuthResolverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{}", self.err) }
}

impl std::error::Error for AuthResolverError {}

#[async_trait::async_trait]
pub trait AuthResolver {
    async fn authenticate(&self, headers: &http::HeaderMap) -> Result<AuthContext, AuthResolverError>;
}
