use crate::error_code::ErrorCode;
use crate::errors::{self, reject_anyhow, reject_error};
use crate::judger::JudgerInfo;
use crate::App;

use heng_protocol::internal::http::{AcquireTokenOutput, AcquireTokenRequest};

use std::convert::Infallible;
use std::sync::Arc;

use anyhow::Result;
use serde::Deserialize;
use validator::Validate;
use warp::filters::ws;
use warp::http::StatusCode;
use warp::reply::{self, Response};
use warp::{Filter, Rejection, Reply};

macro_rules! impl_filter{
    () => {
        impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone + Send + Sync + 'static
    };
    ($($ty:ty,)+) => {
        impl Filter<Extract = ($($ty,)+), Error = Infallible> + Clone + Send + Sync + 'static
    };
}

pub fn routes(app: Arc<App>) -> impl_filter!() {
    let prefix = warp::path("v1");
    prefix.and(judger_routes(app)).recover(errors::recover)
}

fn judger_routes(app: Arc<App>) -> impl_filter!() {
    let prefix: _ = warp::path("judger");

    let acquire_token: _ = warp::path("token")
        .and(warp::post())
        .and(warp::body::content_length_limit(4096))
        .and(with_app(&app))
        .and(warp::body::json())
        .and_then(acquire_token);

    let websocket = warp::path("websocket")
        .and(with_app(&app))
        .and(warp::query::<WsQuery>())
        .and(warp::ws())
        .and_then(websocket);

    prefix.and(acquire_token.or(websocket))
}

fn with_app(app: &Arc<App>) -> impl_filter!(Arc<App>,) {
    let app = app.clone();
    warp::any().map(move || app.clone())
}

/// POST /v1/judgers/token
/// JSON: AcquireTokenRequest => AcquireTokenOutput
async fn acquire_token(app: Arc<App>, body: AcquireTokenRequest) -> Result<Response, Rejection> {
    if let Err(err) = body.validate() {
        return Err(reject_error(
            StatusCode::BAD_REQUEST,
            ErrorCode::InvalidRequest,
            Some(err.to_string()),
        ));
    }

    let judger = app.judger_module.clone();
    let info = JudgerInfo {
        max_task_count: body.max_task_count,
        name: body.name,
        core_count: body.core_count,
        system_info: body.software,
    };

    let ws_id = judger.register_judger(info).await.map_err(reject_anyhow)?;

    let output = AcquireTokenOutput {
        token: ws_id.to_string(),
    };
    Ok(reply::json(&output).into_response())
}

#[derive(Debug, Deserialize)]
struct WsQuery {
    token: String,
}

/// GET /v1/judgers/websocket?token={}
/// WEBSOCKET
async fn websocket(app: Arc<App>, query: WsQuery, ws: ws::Ws) -> Result<impl Reply, Rejection> {
    let judger = match app.judger_module.find_judger(&query.token).await {
        Some(j) => j,
        None => {
            return Err(reject_error(
                StatusCode::FORBIDDEN,
                ErrorCode::NotRegistered,
                None,
            ))
        }
    };

    if !judger.is_registered().await {
        return Err(reject_error(
            StatusCode::BAD_REQUEST,
            ErrorCode::AlreadyConnected,
            None,
        ));
    }

    Ok(ws.on_upgrade(move |ws| judger.start_session(ws)))
}
