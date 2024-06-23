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
use regex::Regex;
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
    pub geo_node_status_url: String,
    pub geo_node_query_base_url: String,
}

struct GeoService {
    state: Arc<GeoServiceState>,
}

impl GeoService {
    fn new(state: Arc<GeoServiceState>) -> Self {
        Self { state }
    }
}

fn rewrite_request(mut value: Value) -> Value {
    if let Some(query) = value.get_mut("query") {
        if let Some(query_str) = query.as_str() {
            let cleaned = regex::Regex::new(r"\s*,?\s*block\s*:\s*(null|\{[^}]*\})\s*,?")
                .unwrap()
                .replace_all(query_str, "")
                .to_string();
            let cleaned = regex::Regex::new(r"\(\s*,\s*")
                .unwrap()
                .replace_all(&cleaned, "(")
                .to_string();
            let cleaned = regex::Regex::new(r"\(\s*\)")
                .unwrap()
                .replace_all(&cleaned, "")
                .to_string();
            *query = Value::String(cleaned);
        }
    }
    value
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
        let deployment_url =
            Url::parse(&format!("{}/graphql", &self.state.geo_node_query_base_url))
                .map_err(|_| GeoServiceError::InvalidDeployment(deployment))?;

        // strip stuff from the request that we don't want to forward
        let rewritten_request = rewrite_request(request.clone());
        tracing::info!("Forwarding request: {:?}", rewritten_request);
        let response = self
            .state
            .geo_node_client
            .post(deployment_url)
            .json(&rewritten_request)
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
        geo_node_status_url: config
            .geo
            .geo_node
            .as_ref()
            .expect("Config must have `geo.geo_node.status_url` set")
            .status_url
            .clone(),
        geo_node_query_base_url: config
            .geo
            .geo_node
            .as_ref()
            .expect("config must have `geo.geo_node.query_url` set")
            .query_base_url
            .clone(),
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
