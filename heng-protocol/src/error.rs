use http::StatusCode;
use serde::{Deserialize, Serialize};

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorCode {
    UnknownError = 1000,
    NotSupported = 1001,
    InvalidRequest = 1002,
    NotRegistered = 1003,
    AlreadyConnected = 1004,
}

impl ErrorCode {
    pub fn as_status(self) -> StatusCode {
        match self {
            ErrorCode::UnknownError => StatusCode::INTERNAL_SERVER_ERROR,
            ErrorCode::NotSupported => StatusCode::NOT_IMPLEMENTED,
            ErrorCode::InvalidRequest => StatusCode::BAD_REQUEST,
            ErrorCode::NotRegistered => StatusCode::FORBIDDEN,
            ErrorCode::AlreadyConnected => StatusCode::BAD_REQUEST,
        }
    }
}
