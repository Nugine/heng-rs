#[repr(u32)]
pub enum ErrorCode {
    UnknownError = 1000,
    NotSupported = 1001,
    InvalidRequest = 1002,
    NotRegistered = 1003,
    AlreadyConnected = 1004,
}
