use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Validate, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcquireTokenRequest {
    #[validate(range(min = 1, max = 64))]
    pub max_task_count: u32,

    #[validate(length(max = 256))]
    pub name: Option<String>,

    pub core_count: Option<u32>,

    #[validate(length(max = 256))]
    pub software: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AcquireTokenOutput {
    pub token: String,
}
