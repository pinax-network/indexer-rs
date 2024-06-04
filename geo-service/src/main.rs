// Copyright 2023-, GraphOps, Pinax and Semiotic Labs.
// SPDX-License-Identifier: Apache-2.0

use std::process::ExitCode;

use geo_service::service::run;

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt::init();
    if let Err(e) = run().await {
        tracing::error!("Indexer service error: {e}");
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}
