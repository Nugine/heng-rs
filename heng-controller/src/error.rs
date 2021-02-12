#[derive(Debug, Serialize)]
pub struct ErrorInfo {
    pub code: u32,
    pub message: Option<String>,
}
