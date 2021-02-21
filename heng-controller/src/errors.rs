use heng_protocol::error::ErrorCode;
use heng_protocol::internal::ErrorInfo;

use warp::hyper::StatusCode;
use warp::reject::Reject;
use warp::{reply, Rejection, Reply};

#[derive(Debug)]
struct Anyhow(anyhow::Error);
impl Reject for Anyhow {}

#[derive(Debug)]
struct Error(StatusCode, ErrorInfo);
impl Reject for Error {}

pub fn reject_anyhow(err: anyhow::Error) -> Rejection {
    warp::reject::custom(Anyhow(err))
}

pub fn reject_error(status: StatusCode, code: ErrorCode, msg: Option<String>) -> Rejection {
    warp::reject::custom(Error(
        status,
        ErrorInfo {
            code: code as u32,
            message: msg,
        },
    ))
}

pub async fn recover(rejection: Rejection) -> Result<impl Reply, Rejection> {
    let status;
    let info;
    if let Some(Anyhow(err)) = rejection.find() {
        status = StatusCode::INTERNAL_SERVER_ERROR;
        info = ErrorInfo {
            code: ErrorCode::UnknownError as u32,
            message: Some(err.to_string()),
        };
    } else if let Some(Error(s, err)) = rejection.find() {
        status = *s;
        info = err.clone();
    } else {
        return Err(rejection);
    }
    Ok(reply::with_status(reply::json(&info), status))
}
