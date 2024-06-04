// Copyright 2023-, GraphOps, Pinax and Semiotic Labs.
// SPDX-License-Identifier: Apache-2.0

use anyhow::Error;
use axum::response::{IntoResponse, Response};
use reqwest::StatusCode;
use thegraph::types::DeploymentId;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GeoServiceError {
    #[error("Invalid status query: {0}")]
    InvalidStatusQuery(Error),
    #[error("Unsupported status query fields: {0:?}")]
    UnsupportedStatusQueryFields(Vec<String>),
    #[error("Internal server error: {0}")]
    StatusQueryError(Error),
    #[error("Invalid deployment: {0}")]
    InvalidDeployment(DeploymentId),
    #[error("Failed to process query: {0}")]
    QueryForwardingError(reqwest::Error),
}

impl From<&GeoServiceError> for StatusCode {
    fn from(err: &GeoServiceError) -> Self {
        use GeoServiceError::*;
        match err {
            InvalidStatusQuery(_) => StatusCode::BAD_REQUEST,
            UnsupportedStatusQueryFields(_) => StatusCode::BAD_REQUEST,
            StatusQueryError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            InvalidDeployment(_) => StatusCode::BAD_REQUEST,
            QueryForwardingError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

// Tell axum how to convert `GeoServiceError` into a response.
impl IntoResponse for GeoServiceError {
    fn into_response(self) -> Response {
        (StatusCode::from(&self), self.to_string()).into_response()
    }
}
