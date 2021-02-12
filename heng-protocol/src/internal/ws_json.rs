use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Message {
    #[serde(rename = "req")]
    Request {
        seq: u32,
        time: DateTime<Utc>,
        body: Request,
    },
    #[serde(rename = "res")]
    Response {
        seq: u32,
        time: DateTime<Utc>,
        body: Option<Box<RawValue>>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Request {
    Judge(JudgeArgs),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JudgeArgs {
    pub placeholder: u32,
}
