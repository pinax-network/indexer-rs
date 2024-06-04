// Copyright 2023-, GraphOps and Semiotic Labs.
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use async_graphql_axum::GraphQLRequest;
use axum::{extract::State, response::IntoResponse};

use crate::{error::GeoServiceError, service::GeoServiceState};

pub async fn cost(
    State(_state): State<Arc<GeoServiceState>>,
    _req: GraphQLRequest,
) -> Result<impl IntoResponse, GeoServiceError> {
    Ok("{}")
}
