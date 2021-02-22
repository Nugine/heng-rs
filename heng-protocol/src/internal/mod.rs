use crate::error::ErrorCode;

use std::fmt;

use serde::{Deserialize, Serialize};

pub mod http;
pub mod ws_json;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionSettings {
    pub status_report_interval: u64, // milliseconds
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartialConnectionSettings {
    pub status_report_interval: Option<u64>, // milliseconds
}

#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
pub struct ErrorInfo {
    pub code: ErrorCode,
    pub message: Option<String>,
}

impl fmt::Display for ErrorInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        <Self as fmt::Debug>::fmt(self, f)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JudgeState {
    Confirmed,
    Pending,
    Preparing,
    Judgeing,
    Finished,
}
