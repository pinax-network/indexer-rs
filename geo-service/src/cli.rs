// Copyright 2023-, GraphOps, Pinax and Semiotic Labs.
// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;

use clap::Parser;

#[derive(Parser)]
pub struct Cli {
    /// Path to the configuration file.
    /// See https://github.com/graphprotocol/indexer-rs/tree/main/geo-service for examples.
    #[arg(long, value_name = "FILE", verbatim_doc_comment)]
    pub config: PathBuf,
}
