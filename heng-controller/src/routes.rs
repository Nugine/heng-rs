use crate::auth::{self, AuthModule, ClientKind};
use crate::errors::{self, reject_anyhow, reject_error};
use crate::external::ExternalModule;
use crate::judger::{JudgeTask, JudgerInfo, JudgerModule};

use heng_utils::container::inject;

use heng_protocol::error::ErrorCode;
use heng_protocol::external::{CallbackUrls, CreateJudgeRequest};
use heng_protocol::internal::http::{AcquireTokenOutput, AcquireTokenRequest};
use heng_protocol::signature::calc_signature;
use serde::de::DeserializeOwned;
use serde_json::from_slice;
use warp::http::HeaderValue;
use warp::hyper::{HeaderMap, Method};
use warp::path::FullPath;

use std::convert::Infallible;
use std::sync::Arc;

use anyhow::Result;
use bytes::Bytes;
use serde::Deserialize;
use tokio::task;
use uuid::Uuid;
use validator::Validate;
use warp::filters::ws;
use warp::reply::{self, Response};
use warp::{Filter, Rejection, Reply};

macro_rules! impl_filter{
    ($($tt:tt)+)=> {
        impl Filter<Extract = ($($tt)+), Error = Rejection> + Clone + Send + Sync + 'static
    }
}

macro_rules! reject {
    ($code:expr) => {
        return Err(reject_error($code, None));
    };

    ($code:expr, $msg:expr) => {
        return Err(reject_error($code, Some($msg)));
    };
}

pub fn routes() -> impl_filter!(impl Reply,) {
    let prefix: _ = warp::path("v1");

    let routes: _ = judgers_routes().or(judges_routes());

    prefix.and(routes).recover(errors::recover)
}

const BODY_SIZE_HARD_LIMIT: u64 = 256 * 1024;

fn judgers_routes() -> impl_filter!(impl Reply,) {
    let prefix: _ = warp::path("judgers");

    let acquire_token: _ = warp::path("token")
        .and(warp::post())
        .and(signature_guard())
        .and_then(|(c, b)| async move { acquire_token(c, json(b)?).await });

    let websocket: _ = warp::path("websocket")
        .and(signature_guard())
        .and(warp::query::<WsQuery>())
        .and(warp::ws())
        .and_then(|(c, b), q, ws| async move { websocket(c, q, ws).await });

    let routes: _ = acquire_token.or(websocket);
    prefix.and(routes)
}

fn judges_routes() -> impl_filter!(impl Reply,) {
    let prefix: _ = warp::path("judges");

    let create_judge: _ = warp::post()
        .and(signature_guard())
        .and_then(|(c, b)| async move { create_judge(c, json(b)?).await });

    prefix.and(create_judge)
}

fn query_optional() -> impl_filter!(Option<String>,) {
    warp::query::raw()
        .map(Some)
        .or_else(|_| async { <Result<_, Rejection>>::Ok((None,)) })
}

fn json<T: DeserializeOwned>(body: Bytes) -> Result<T, Rejection> {
    match serde_json::from_slice(&body) {
        Ok(x) => Ok(x),
        Err(err) => reject!(ErrorCode::InvalidRequest, err.to_string()),
    }
}

fn signature_guard() -> impl_filter!((auth::Client, Bytes),) {
    warp::header::value("x-heng-accesskey")
        .and(warp::header::value("x-heng-signature"))
        .and(warp::method())
        .and(warp::path::full())
        .and(query_optional())
        .and(warp::header::headers_cloned())
        .and(warp::body::content_length_limit(BODY_SIZE_HARD_LIMIT))
        .and(warp::body::bytes())
        .and_then(check_signature)
}

async fn check_signature(
    access_key: HeaderValue,
    signature: HeaderValue,
    method: Method,
    path: FullPath,
    query: Option<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<(auth::Client, Bytes), Rejection> {
    let access_key = access_key
        .to_str()
        .map_err(|err| reject_anyhow(err.into()))?;

    let auth_module = inject::<AuthModule>();

    let (client_kind, secret_key) = match auth_module.lookup(access_key).map_err(reject_anyhow)? {
        Some(x) => x,
        None => reject!(ErrorCode::SignatureMismatch),
    };

    let expected_signature = calc_signature(
        &method,
        path.as_str(),
        query.as_deref().unwrap_or(""),
        &headers,
        &*body,
        &secret_key,
    );

    if expected_signature.as_bytes() != signature.as_bytes() {
        reject!(ErrorCode::SignatureMismatch)
    }

    // let body = match serde_json::from_slice::<T>(&body) {
    //     Ok(x) => x,
    //     Err(err) => reject!(ErrorCode::InvalidRequest, err.to_string()),
    // };

    Ok((
        auth::Client {
            kind: client_kind,
            access_key: access_key.into(),
        },
        body,
    ))
}

/// POST /v1/judgers/token
/// JSON: AcquireTokenRequest => AcquireTokenOutput
async fn acquire_token(
    client: auth::Client,
    body: AcquireTokenRequest,
) -> Result<Response, Rejection> {
    if let Err(err) = body.validate() {
        reject!(ErrorCode::InvalidRequest)
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
async fn websocket(
    clietn: auth::Client,
    query: WsQuery,
    ws: ws::Ws,
) -> Result<impl Reply, Rejection> {
    let judger_module = inject::<JudgerModule>();

    let judger = match judger_module.find_judger(&query.token).await {
        Some(j) => j,
        None => reject!(ErrorCode::NotRegistered),
    };

    if !judger.is_registered().await {
        reject!(ErrorCode::AlreadyConnected)
    }

    Ok(ws.on_upgrade(move |ws| judger.start_session(ws)))
}

/// POST /v1/judges
/// JSON: CreateJudgeRequest => ()
async fn create_judge(
    client: auth::Client,
    body: CreateJudgeRequest,
) -> Result<Response, Rejection> {
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
