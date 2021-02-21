use crate::common::{DynamicFile, File, Judge, JudgeResult, JudgeState, Test};

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateJudgeRequest {
    pub data: Option<File>,
    pub dynamic_files: Option<Vec<DynamicFile>>,
    pub judge: Judge,
    pub test: Test,
    pub callback_urls: CallbackUrls,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CallbackUrls {
    pub update: String,
    pub finish: String,
}

pub struct UpdateJudgeCallback {
    pub state: JudgeState,
}

pub struct FinishJudgeCallback {
    pub result: JudgeResult,
}
