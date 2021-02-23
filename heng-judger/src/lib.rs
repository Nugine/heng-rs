#![deny(clippy::all)]

pub mod config;
pub mod judger;
pub mod redis;

use crate::config::Config;
use crate::judger::Judger;

use heng_protocol::internal::http::{AcquireTokenOutput, AcquireTokenRequest};

use anyhow::{format_err, Result};
use redis::RedisModule;
use tokio_tungstenite as ws;
use tracing::{error, info};

type WsStream = ws::WebSocketStream<tokio::net::TcpStream>;

pub async fn run() -> Result<()> {
    info!("initializing redis module");
    let redis_module = RedisModule::new()?;
    info!("redis module is initialized");

    let config = Config::global();
    let remote_domain = config.judger.remote_domain.as_str();

    let token = get_token(remote_domain).await?;
    let ws = connect_ws(remote_domain, &token).await?;

    Judger::run(redis_module, ws).await
}

#[tracing::instrument(err)]
async fn get_token(remote_domain: &str) -> Result<String> {
    let token_url = format!("http://{}/v1/judgers/token", remote_domain);

    // TODO: AK and SK

    let body = AcquireTokenRequest {
        max_task_count: 8,
        name: None,
        core_count: None,
        software: None,
    };

    let http_client = reqwest::Client::new();
    let res = http_client.post(&token_url).json(&body).send().await?;
    if res.status().is_success() {
        let output = res.json::<AcquireTokenOutput>().await?;
        Ok(output.token)
    } else {
        let status = res.status();
        let text = res.text().await.unwrap();
        error!(?status, ?text, "failed to acquire token");
        Err(format_err!("failed to acquire token"))
    }
}

#[tracing::instrument(err)]
async fn connect_ws(remote_domain: &str, token: &str) -> Result<WsStream> {
    let ws_url = format!(
        "ws://{}/v1/judgers/websocket?token={}",
        remote_domain, token
    );
    info!("connecting to {}", ws_url);
    let (ws_stream, _) = ws::connect_async(ws_url).await?;
    info!("connected");
    Ok(ws_stream)
}
