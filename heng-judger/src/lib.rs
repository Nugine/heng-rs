pub mod config;
pub mod judger;

use std::sync::Arc;

use crate::config::Config;
use crate::judger::Judger;

use futures::SinkExt;
use heng_protocol::internal::http::{AcquireTokenOutput, AcquireTokenRequest};
use heng_protocol::internal::ws_json::Message as WsMessage;

use anyhow::Result;
use futures::stream::StreamExt;
use tokio::sync::mpsc;
use tokio::task;
use tokio_tungstenite as ws;
use tokio_tungstenite::tungstenite;
use tracing::{error, info, warn};

type WsStream = ws::WebSocketStream<tokio::net::TcpStream>;

pub async fn run() -> Result<()> {
    let config = Config::global();

    let remote_domain = config.client.remote_domain.as_str();

    // TODO: reconnect

    let token = get_token(remote_domain).await?;
    let ws_stream = connect_ws(remote_domain, &token).await?;

    main_loop(ws_stream).await?;

    Ok(())
}

#[tracing::instrument(err)]
async fn get_token(remote_domain: &str) -> Result<String> {
    let token_url = format!("http://{}/v1/judger/token", remote_domain);

    // TODO: AK and SK

    let body = AcquireTokenRequest {
        max_task_count: 8,
        name: None,
        core_count: None,
        system_info: None,
    };

    let http_client = reqwest::Client::new();
    let res = http_client.post(&token_url).json(&body).send().await?;
    let output = res.json::<AcquireTokenOutput>().await?;

    Ok(output.token)
}

#[tracing::instrument(err)]
async fn connect_ws(remote_domain: &str, token: &str) -> Result<WsStream> {
    let ws_url = format!("ws://{}/v1/judger/websocket?token={}", remote_domain, token);
    info!("connecting to {}", ws_url);
    let (ws_stream, _) = ws::connect_async(ws_url).await?;
    info!("connected");
    Ok(ws_stream)
}

async fn main_loop(ws_stream: WsStream) -> Result<()> {
    let (mut ws_tx, mut ws_rx) = ws_stream.split();

    let (msg_tx, mut msg_rx) = mpsc::channel::<WsMessage>(4096);

    task::spawn(async move {
        while let Some(res_msg) = msg_rx.recv().await {
            let msg = serde_json::to_string(&res_msg).unwrap();
            ws_tx.send(tungstenite::Message::Text(msg)).await?;
        }
        <Result<()>>::Ok(())
    });

    let judger = Arc::new(Judger::new(msg_tx.clone()));

    while let Some(frame) = ws_rx.next().await {
        use tungstenite::Message::*;

        let frame = frame?;

        match frame {
            Close(reason) => {
                warn!(?reason, "ws session closed");
                break;
            }
            Text(text) => {
                let msg = match serde_json::from_str(&text) {
                    Ok(m) => m,
                    Err(err) => {
                        error!(%err, "internal protocol: message format error");
                        return Err(err.into());
                    }
                };
                let judger = Arc::clone(&judger);
                task::spawn(async move { judger.handle_controller_message(msg).await });
            }
            _ => {
                warn!("drop ws message");
                drop(frame);
            }
        }
    }

    Ok(())
}
