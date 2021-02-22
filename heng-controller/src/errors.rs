use heng_protocol::error::ErrorCode;
use heng_protocol::internal::ErrorInfo;

use warp::hyper::StatusCode;
use warp::reject::Reject;
use warp::{reply, Rejection, Reply};

#[derive(Debug)]
struct Anyhow(anyhow::Error);
impl Reject for Anyhow {}

#[derive(Debug)]
struct Error(ErrorInfo);
impl Reject for Error {}

pub fn reject_anyhow(err: anyhow::Error) -> Rejection {
    warp::reject::custom(Anyhow(err))
}

pub fn reject_error(code: ErrorCode, message: Option<String>) -> Rejection {
    warp::reject::custom(Error(ErrorInfo { code, message }))
}

pub async fn recover(rejection: Rejection) -> Result<impl Reply, Rejection> {
    let status;
    let info;
    if let Some(Anyhow(err)) = rejection.find() {
        if let Some(err) = err.downcast_ref::<ErrorInfo>() {
            status = err.code.as_status();
            info = err.clone();
        } else {
            status = StatusCode::INTERNAL_SERVER_ERROR;
            info = ErrorInfo {
                code: ErrorCode::UnknownError,
                message: Some(err.to_string()),
            };
        }
    } else if let Some(Error(err)) = rejection.find() {
        status = err.code.as_status();
        info = err.clone();
    } else {
        return Err(rejection);
    }
    Ok(reply::with_status(reply::json(&info), status))
}
