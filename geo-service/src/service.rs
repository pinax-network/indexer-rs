// Copyright 2023-, GraphOps, Pinax and Semiotic Labs.
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;
use std::time::Duration;

use super::{config::Config, error::GeoServiceError, routes};
use anyhow::Error;
use axum::{async_trait, routing::post, Json, Router};
use indexer_common::indexer_service::http::{IndexerServiceImpl, IndexerServiceResponse};
use reqwest::Url;
use serde_json::{json, Value};
use thegraph::types::{Attestation, DeploymentId};

use crate::cli::Cli;

use clap::Parser;
use indexer_common::indexer_service::http::{
    IndexerService, IndexerServiceOptions, IndexerServiceRelease,
};
use tracing::error;

#[derive(Debug)]
struct GeoServiceResponse {
    inner: String,
    attestable: bool,
}

impl GeoServiceResponse {
    pub fn new(inner: String, attestable: bool) -> Self {
        Self { inner, attestable }
    }
}

impl IndexerServiceResponse for GeoServiceResponse {
    type Data = Json<Value>;
    type Error = GeoServiceError; // not used

    fn is_attestable(&self) -> bool {
        self.attestable
    }

    fn as_str(&self) -> Result<&str, Self::Error> {
        Ok(self.inner.as_str())
    }

    fn finalize(self, attestation: Option<Attestation>) -> Self::Data {
        Json(json!({
            "graphQLResponse": self.inner,
            "attestation": attestation
        }))
    }
}

pub struct GeoServiceState {
    pub config: Config,
    pub geo_node_client: reqwest::Client,
}

struct GeoService {
    state: Arc<GeoServiceState>,
}

impl GeoService {
    fn new(state: Arc<GeoServiceState>) -> Self {
        Self { state }
    }
}

#[async_trait]
impl IndexerServiceImpl for GeoService {
    type Error = GeoServiceError;
    type Request = serde_json::Value;
    type Response = GeoServiceResponse;
    type State = GeoServiceState;

    async fn process_request(
        &self,
        deployment: DeploymentId,
        request: Self::Request,
    ) -> Result<(Self::Request, Self::Response), Self::Error> {
        let deployment_url = Url::parse(&format!(
            "{}/graphql",
            &self.state.config.geo.query_base_url
        ))
        .map_err(|_| GeoServiceError::InvalidDeployment(deployment))?;

        tracing::debug!("Query request: {:?}", request);
        let response = self
            .state
            .geo_node_client
            .post(deployment_url)
            .json(&request)
            .send()
            .await
            .map_err(GeoServiceError::QueryForwardingError)?;

        let attestable = response
            .headers()
            .get("graph-attestable")
            .map_or(true, |value| {
                value.to_str().map(|value| value == "true").unwrap_or(true) // default is true
            });

        let body = response
            .text()
            .await
            .map_err(GeoServiceError::QueryForwardingError)?;

        tracing::debug!("Query response: {:?}", body);
        Ok((request, GeoServiceResponse::new(body, attestable)))
    }
}

/// Run the geo indexer service
pub async fn run() -> Result<(), Error> {
    // Parse command line and environment arguments
    let cli = Cli::parse();

    // Load the json-rpc service configuration, which is a combination of the
    // general configuration options for any indexer service and specific
    // options added for JSON-RPC
    let config = Config::load(&cli.config).map_err(|e| {
        error!(
            "Invalid configuration file `{}`: {}",
            cli.config.display(),
            e
        );
        e
    })?;

    // Parse basic configurations
    build_info::build_info!(fn build_info);
    let release = IndexerServiceRelease::from(build_info());

    // Some of the geo service configuration goes into the so-called
    // "state", which will be passed to any request handler, middleware etc.
    // that is involved in serving requests
    let state = Arc::new(GeoServiceState {
        config: config.clone(),
        geo_node_client: reqwest::ClientBuilder::new()
            .tcp_nodelay(true)
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to init HTTP client for Geo Node"),
    });

    IndexerService::run(IndexerServiceOptions {
        release,
        config: config.common.clone(),
        url_namespace: "subgraphs",
        metrics_prefix: "geo",
        service_impl: GeoService::new(state.clone()),
        extra_routes: Router::new()
            .route("/cost", post(routes::cost::cost))
            .route("/status", post(routes::status))
            .with_state(state),
    })
    .await
}
