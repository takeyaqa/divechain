use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub(crate) struct SecretRequest {
    pub(crate) namespace: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub(crate) struct SecretResponse {
    pub(crate) secrets: Vec<HashMap<String, String>>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub(crate) struct ErrorResponse {
    pub(crate) error: ErrorBody,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub(crate) struct ErrorBody {
    pub(crate) code: String,
    pub(crate) message: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
#[serde(untagged)]
pub(crate) enum WireResponse {
    Success(SecretResponse),
    Error(ErrorResponse),
}
