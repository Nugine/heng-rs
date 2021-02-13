use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;

use super::{ErrorInfo, JudgeState, PartialConnectionSettings};

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
        body: Response,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "method", content = "args")]
pub enum Request {
    CreateJudge(CreateJudgeArgs),
    Control(Option<PartialConnectionSettings>),
    ReportStatus(ReportStatusArgs),
    UpdateJudges(Vec<UpdateJudgeArgs>),
    FinishJudges(Vec<FinishJudgeArgs>),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Response {
    #[serde(rename = "output")]
    Output(Option<Box<RawValue>>),
    #[serde(rename = "error")]
    Error(ErrorInfo),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateJudgeArgs {
    pub id: String, // TODO: complete this definition
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportStatusArgs {
    pub collect_time: DateTime<Utc>,
    pub next_report_time: DateTime<Utc>,
    pub report: Option<Box<RawValue>>, // FIXME: define type StatusReport
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateJudgeArgs {
    pub id: String,
    pub state: JudgeState,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FinishJudgeArgs {
    pub id: String,
    pub result: Option<Box<RawValue>>, // FIXME: define type JudgeResult
}
