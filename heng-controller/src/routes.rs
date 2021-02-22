use crate::container::inject;
use crate::errors::{self, reject_anyhow, reject_error};
use crate::external::ExternalModule;
use crate::judger::{JudgeTask, JudgerInfo, JudgerModule};

use heng_protocol::error::ErrorCode;
use heng_protocol::external::{CallbackUrls, CreateJudgeRequest};
use heng_protocol::internal::http::{AcquireTokenOutput, AcquireTokenRequest};
use tokio::task;
use uuid::Uuid;

use std::sync::Arc;

use anyhow::Result;
use serde::Deserialize;
use validator::Validate;
use warp::filters::ws;
use warp::reply::{self, Response};
use warp::{Filter, Rejection, Reply};

macro_rules! impl_filter{
    () => {
        impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone + Send + Sync + 'static
    };
}

pub fn routes() -> impl_filter!() {
    let prefix: _ = warp::path("v1");

    let routes: _ = judgers_routes().or(judges_routes());
    prefix.and(routes).recover(errors::recover)
}

fn judgers_routes() -> impl_filter!() {
    let prefix: _ = warp::path("judgers");

    let acquire_token: _ = warp::path("token")
        .and(warp::post())
        .and(warp::body::content_length_limit(4096))
        .and(warp::body::json())
        .and_then(acquire_token);

    let websocket: _ = warp::path("websocket")
        .and(warp::query::<WsQuery>())
        .and(warp::ws())
        .and_then(websocket);

    let routes: _ = acquire_token.or(websocket);
    prefix.and(routes)
}

fn judges_routes() -> impl_filter!() {
    let prefix: _ = warp::path("judges");

    let create_judge = warp::post()
        .and(warp::body::content_length_limit(256 * 1024))
        .and(warp::body::json())
        .and_then(create_judge);

    prefix.and(create_judge)
}

/// POST /v1/judgers/token
/// JSON: AcquireTokenRequest => AcquireTokenOutput
async fn acquire_token(body: AcquireTokenRequest) -> Result<Response, Rejection> {
    if let Err(err) = body.validate() {
        return Err(reject_error(
            ErrorCode::InvalidRequest,
            Some(err.to_string()),
        ));
    }

    let judger_module = inject::<JudgerModule>();

    let info = JudgerInfo {
        max_task_count: body.max_task_count,
        name: body.name,
        core_count: body.core_count,
        system_info: body.software,
    };

    let ws_id = judger_module
        .register_judger(info)
        .await
        .map_err(reject_anyhow)?;

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
async fn websocket(query: WsQuery, ws: ws::Ws) -> Result<impl Reply, Rejection> {
    let judger_module = inject::<JudgerModule>();

    let judger = match judger_module.find_judger(&query.token).await {
        Some(j) => j,
        None => return Err(reject_error(ErrorCode::NotRegistered, None)),
    };

    if !judger.is_registered().await {
        return Err(reject_error(ErrorCode::AlreadyConnected, None));
    }

    Ok(ws.on_upgrade(move |ws| judger.start_session(ws)))
}

/// POST /v1/judges
/// JSON: CreateJudgeRequest => ()
async fn create_judge(body: CreateJudgeRequest) -> Result<Response, Rejection> {
    let judger_module = inject::<JudgerModule>();
    let external_module = inject::<ExternalModule>();

    let task_id: Arc<str> = Uuid::new_v4().to_string().into();

    external_module
        .save_judge(&*task_id, &body)
        .await
        .map_err(reject_anyhow)?;

    let CreateJudgeRequest {
        data,
        dynamic_files,
        judge,
        test,
        callback_urls,
    } = body;

    let CallbackUrls {
        update: update_url,
        finish: finish_url,
    } = callback_urls;

    let update_callback = {
        let (tx, rx) = async_channel::bounded(0);
        task::spawn(async move {
            while let Ok((task_id, state)) = rx.recv().await {
                dbg!((&update_url, &task_id, &state));
            }
        });
        tx
    };

    let finish_callback = {
        let (tx, rx) = async_channel::bounded::<(Arc<str>, _)>(0);
        task::spawn(async move {
            if let Ok((task_id, result)) = rx.recv().await {
                let _ = external_module.remove_judge(&*task_id).await;
                dbg!(&finish_url, &task_id, &result);
            }
        });
        tx
    };

    let judge_task = JudgeTask {
        id: task_id,
        data,
        dynamic_files,
        judge,
        test,
        update_callback,
        finish_callback,
    };

    judger_module
        .schedule(judge_task)
        .await
        .map_err(reject_anyhow)?;

    Ok(reply::reply().into_response())
}
