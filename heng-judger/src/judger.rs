use crate::config::Config;
use crate::redis::RedisModule;

use heng_protocol::error::ErrorCode;
use heng_protocol::internal::ws_json::Message as RpcMessage;
use heng_protocol::internal::ws_json::{Request as RpcRequest, Response as RpcResponse};
use heng_protocol::internal::{ConnectionSettings, ErrorInfo, PartialConnectionSettings};

use heng_protocol::internal::ws_json::{
    CreateJudgeArgs, FinishJudgeArgs, ReportStatusArgs, UpdateJudgeArgs,
};

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering::Relaxed};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use chrono::Utc;
use futures::stream::SplitStream;
use futures::StreamExt;
use futures::TryFutureExt;
use serde::Serialize;
use serde_json::value::RawValue;
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::{task, time};
use tokio_stream::wrappers::ReceiverStream;
use tokio_tungstenite as ws;
use tokio_tungstenite::tungstenite;
use tracing::{debug, error, info, warn};
use ws::tungstenite::protocol::frame::coding::CloseCode;
use ws::tungstenite::protocol::CloseFrame;

type WsStream = ws::WebSocketStream<tokio::net::TcpStream>;
type WsMessage = tungstenite::Message;

pub struct Judger {
    redis_module: RedisModule,
    settings: Settings,
    counter: Mutex<Counter>,
    session: WsSession,
}

struct WsSession {
    sender: mpsc::Sender<WsMessage>,
    seq: AtomicU32,
    callbacks: Mutex<HashMap<u32, oneshot::Sender<RpcResponse>>>,
}

struct Settings {
    status_report_interval: AtomicU64,
}

#[derive(Debug, Clone)]
struct Counter {
    pending: u64,
    judging: u64,
    finished: u64,
}

impl Judger {
    pub async fn run(redis_module: RedisModule, ws: WsStream) -> Result<()> {
        let (ws_sink, ws_stream) = ws.split();

        let (tx, rx) = mpsc::channel::<WsMessage>(4096);

        task::spawn(
            ReceiverStream::new(rx)
                .map(Ok)
                .forward(ws_sink)
                .inspect_err(|err| error!(%err, "ws forward error")),
        );

        let judger = Arc::new(Self {
            session: WsSession {
                sender: tx,
                seq: AtomicU32::new(0),
                callbacks: Mutex::new(HashMap::new()),
            },
            redis_module,
            settings: Settings {
                status_report_interval: AtomicU64::new(1000),
            },
            counter: Mutex::new(Counter {
                pending: 0,
                judging: 0,
                finished: 0,
            }),
        });

        {
            let judger = judger.clone();
            task::spawn(async move { judger.report_status_loop().await });
        }

        judger.main_loop(ws_stream).await
    }

    async fn main_loop(self: Arc<Self>, mut ws_stream: SplitStream<WsStream>) -> Result<()> {
        info!("starting main loop");
        while let Some(frame) = ws_stream.next().await {
            use tungstenite::Message::*;

            let frame = frame?;

            match frame {
                Close(reason) => {
                    warn!(?reason, "ws session closed");
                    return Ok(());
                }
                Text(text) => {
                    let rpc_msg: RpcMessage = match serde_json::from_str(&text) {
                        Ok(m) => m,
                        Err(err) => {
                            error!(%err, "internal protocol: message format error:\n{:?}\n",text);
                            let close_frame = CloseFrame {
                                code: CloseCode::Invalid,
                                reason: "internal protocol message format error".into(),
                            };
                            let _ = self.session.sender.send(Close(Some(close_frame))).await;
                            return Err(err.into());
                        }
                    };
                    match rpc_msg {
                        RpcMessage::Request { seq, body, .. } => {
                            let this = self.clone();
                            task::spawn(async move {
                                let response = this.clone().handle_rpc_request(body).await;
                                let rpc_msg = RpcMessage::Response {
                                    seq,
                                    time: Utc::now(),
                                    body: response,
                                };
                                let ws_msg =
                                    WsMessage::text(serde_json::to_string(&rpc_msg).unwrap());
                                let _ = this.session.sender.send(ws_msg).await;
                            });
                        }
                        RpcMessage::Response { seq, body, .. } => {
                            let this = self.clone();
                            task::spawn(async move {
                                let mut callbacks = this.session.callbacks.lock().await;
                                match callbacks.remove(&seq) {
                                    None => warn!(?seq, "no such callback"),
                                    Some(cb) => match cb.send(body) {
                                        Ok(()) => {}
                                        Err(_) => warn!(?seq, "the callback is timeouted"),
                                    },
                                }
                            });
                        }
                    }
                }
                _ => {
                    warn!("drop ws message");
                    drop(frame);
                }
            }
        }

        Ok(())
    }

    async fn wsrpc(&self, req: RpcRequest) -> Result<RpcResponse> {
        let session = &self.session;
        let seq = session.seq.fetch_add(1, Relaxed).wrapping_add(1);
        let (tx, rx) = oneshot::channel();
        let rpc_msg = RpcMessage::Request {
            seq,
            time: Utc::now(),
            body: req,
        };
        let ws_msg = WsMessage::text(serde_json::to_string(&rpc_msg).unwrap());

        {
            let mut callbacks = session.callbacks.lock().await;
            callbacks.insert(seq, tx);
            session.sender.send(ws_msg).await.unwrap();
        }

        let rpc_timeout = Config::global().judger.rpc_timeout;
        match time::timeout(Duration::from_millis(rpc_timeout), rx).await {
            Ok(res) => Ok(res.unwrap()),
            Err(err) => {
                let mut callbacks = session.callbacks.lock().await;
                let _ = callbacks.remove(&seq);
                return Err(anyhow::Error::new(err));
            }
        }
    }

    async fn report_status_loop(&self) -> Result<()> {
        loop {
            let delay = self.settings.status_report_interval.load(Relaxed);
            time::sleep(Duration::from_millis(delay)).await;

            let result = self
                .wsrpc(RpcRequest::ReportStatus(ReportStatusArgs {
                    collect_time: Utc::now(),
                    next_report_time: Utc::now() + chrono::Duration::milliseconds(delay as i64),
                    report: None, // FIXME
                }))
                .await;

            let cnt = self.count(|cnt| cnt.clone()).await;

            match result {
                Ok(RpcResponse::Output(None)) => {
                    debug!(interval=?delay, count=?cnt, "report status")
                }
                Ok(RpcResponse::Output(Some(value))) => warn!(?value, "unexpected response"),
                Ok(RpcResponse::Error(err)) => warn!(%err, "report status"),
                Err(_) => warn!("the request failed"),
            }
        }
    }

    async fn count<T>(&self, f: impl FnOnce(&mut Counter) -> T) -> T {
        let mut counter = self.counter.lock().await;
        f(&mut counter)
    }

    async fn handle_rpc_request(self: Arc<Self>, req: RpcRequest) -> RpcResponse {
        match req {
            RpcRequest::CreateJudge(args) => to_null_response(self.create_judge(args).await),
            RpcRequest::Control(args) => to_response(self.control(args).await),
            _ => RpcResponse::Error(ErrorInfo {
                code: ErrorCode::NotSupported as u32,
                message: None,
            }),
        }
    }

    async fn control(
        &self,
        settings: Option<PartialConnectionSettings>,
    ) -> Result<ConnectionSettings> {
        if let Some(settings) = settings {
            if let Some(interval) = settings.status_report_interval {
                self.settings
                    .status_report_interval
                    .store(interval, Relaxed);
            }
        }
        let current_settings = ConnectionSettings {
            status_report_interval: self.settings.status_report_interval.load(Relaxed),
        };
        Ok(current_settings)
    }

    async fn create_judge(self: Arc<Self>, judge: CreateJudgeArgs) -> Result<()> {
        task::spawn(async move {
            self.count(|cnt| cnt.pending += 1).await;

            // let h1 = {
            //     let this = self.clone();
            //     let update = UpdateJudgeArgs {
            //         id: judge.id.clone(),
            //         state: JudgeState::Confirmed,
            //     };
            //     task::spawn(async move { this.update_judge(update).await })
            // };

            // // time::sleep(Duration::from_millis(200)).await;
            self.count(|cnt| {
                cnt.pending -= 1;
                cnt.judging += 1;
            })
            .await;

            // let h2 = {
            //     let this = self.clone();
            //     let update = UpdateJudgeArgs {
            //         id: judge.id.clone(),
            //         state: JudgeState::Judgeing,
            //     };

            //     task::spawn(async move { this.update_judge(update).await })
            // };

            // time::sleep(Duration::from_millis(200)).await;

            // h1.await.ok();
            // h2.await.ok();

            let finish = FinishJudgeArgs {
                id: judge.id.clone(),
                result: None, // TODO
            };

            self.count(|cnt| {
                cnt.judging -= 1;
                cnt.finished += 1;
            })
            .await;

            self.finish_judge(finish).await
        });
        Ok(())
    }

    async fn update_judge(&self, update: UpdateJudgeArgs) -> Result<()> {
        let res = self.wsrpc(RpcRequest::UpdateJudges(vec![update])).await?;
        let output = to_anyhow(res)?;
        if output.is_some() {
            warn!(?output, "unexpected output")
        }
        Ok(())
    }

    async fn finish_judge(&self, finish: FinishJudgeArgs) -> Result<()> {
        let res = self.wsrpc(RpcRequest::FinishJudges(vec![finish])).await?;
        let output = to_anyhow(res)?;
        if output.is_some() {
            warn!(?output, "unexpected output")
        }
        Ok(())
    }
}

fn to_response<T: Serialize>(result: Result<T>) -> RpcResponse {
    match result {
        Ok(value) => {
            let raw_value = RawValue::from_string(serde_json::to_string(&value).unwrap()).unwrap();
            RpcResponse::Output(Some(raw_value))
        }
        Err(err) => RpcResponse::Error(ErrorInfo {
            code: ErrorCode::UnknownError as u32,
            message: Some(err.to_string()),
        }),
    }
}

fn to_null_response(result: Result<()>) -> RpcResponse {
    match result {
        Ok(()) => RpcResponse::Output(None),
        Err(err) => RpcResponse::Error(ErrorInfo {
            code: ErrorCode::UnknownError as u32,
            message: Some(err.to_string()),
        }),
    }
}

fn to_anyhow(response: RpcResponse) -> Result<Option<Box<RawValue>>> {
    match response {
        RpcResponse::Output(output) => Ok(output),
        RpcResponse::Error(err) => Err(anyhow::Error::from(err)),
    }
}
