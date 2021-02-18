use crate::error_code::ErrorCode;
use crate::Config;

use heng_protocol::internal::ws_json::{
    CreateJudgeArgs, Message as RpcMessage, Request as RpcRequest, Response as RpcResponse,
};
use heng_protocol::internal::ErrorInfo;

use std::collections::HashMap;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{format_err, Result};
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use futures::stream::{SplitSink, SplitStream};
use futures::{SinkExt, StreamExt, TryFutureExt};
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::task::{self, JoinHandle};
use tokio::time;
use tokio_stream::wrappers::ReceiverStream;
use tracing::{debug, error, warn};
use uuid::Uuid;
use warp::ws::{self, WebSocket};

pub struct JudgerModule {
    judger_map: DashMap<String, Judger>,
    token_ttl: u64,
}

struct Judger {
    info: JudgerInfo,
    state: JudgerState,
    register_time: DateTime<Utc>,
    online_time: Option<DateTime<Utc>>,
    offline_time: Option<DateTime<Utc>>,
}

pub struct JudgerInfo {
    pub max_task_count: u32,
    pub name: Option<String>,
    pub core_count: Option<u32>,
    pub system_info: Option<String>,
}

enum JudgerState {
    Registered { remove_task: JoinHandle<()> },
    Online(Arc<WsSession>),
    Disabled(Arc<WsSession>),
    Offline,
}

struct WsSession {
    ws_id: String,
    seq: AtomicU32,
    callbacks: Mutex<HashMap<u32, oneshot::Sender<RpcResponse>>>,
    sender: mpsc::Sender<ws::Message>,
}

impl WsSession {
    async fn close_ws(&self) {
        let _ = self.sender.send(ws::Message::close()).await;
    }
}

impl JudgerModule {
    pub fn new(config: &Config) -> Result<Arc<Self>> {
        let token_ttl = config.judger.token_ttl;
        Ok(Arc::new(Self {
            judger_map: DashMap::new(),
            token_ttl,
        }))
    }

    pub async fn register_judger(self: Arc<Self>, info: JudgerInfo) -> Result<String> {
        let ws_id = Uuid::new_v4().to_string();

        let remove_task = {
            let ws_id = ws_id.clone();
            let token_ttl = self.token_ttl;
            let this = Arc::downgrade(&self);
            task::spawn(async move {
                time::sleep(Duration::from_millis(token_ttl)).await;
                let this = match this.upgrade() {
                    Some(t) => t,
                    None => return,
                };
                let item = this.judger_map.remove_if(&ws_id, |_, judger| {
                    matches!(judger.state, JudgerState::Registered { .. })
                });
                if let Some((k, v)) = item {
                    debug!(ws_id=?k, register_time=?v.register_time, "remove registered judger");
                }
            })
        };

        let judger = Judger {
            info,
            state: JudgerState::Registered { remove_task },
            register_time: Utc::now(),
            online_time: None,
            offline_time: None,
        };

        let _ = self.judger_map.insert(ws_id.clone(), judger);

        Ok(ws_id)
    }

    pub fn is_registered(&self, ws_id: &str) -> bool {
        match self.judger_map.get(ws_id) {
            Some(judger) => matches!(judger.state, JudgerState::Registered { .. }),
            None => false,
        }
    }

    pub async fn start_session(self: Arc<Self>, ws_id: String, ws: WebSocket) {
        let (ws_sink, ws_stream) = ws.split();

        let (tx, rx) = mpsc::channel::<ws::Message>(4096);
        task::spawn(
            ReceiverStream::new(rx)
                .map(Ok)
                .forward(ws_sink)
                .inspect_err(|err| error!(%err, "ws forward error")),
        );

        let session = Arc::new(WsSession {
            ws_id,
            seq: AtomicU32::new(0),
            callbacks: Mutex::new(HashMap::new()),
            sender: tx,
        });

        match self.judger_map.get_mut(&session.ws_id) {
            Some(mut judger) => {
                judger.state = JudgerState::Online(session.clone());
                judger.online_time = Some(Utc::now());
            }
            None => {
                session.close_ws().await;
                return drop(session);
            }
        };

        task::spawn(self.run_session(session, ws_stream));
    }

    async fn run_session(self: Arc<Self>, session: Arc<WsSession>, mut ws: SplitStream<WebSocket>) {
        // {
        //     let this = self.clone();
        //     let ws_id = session.ws_id.clone();
        //     task::spawn(async move {
        //         let instant = time::Instant::now();
        //         let mut tasks = Vec::new();
        //         for _ in 0..1000 {
        //             let ws_id = ws_id.clone();
        //             let this = this.clone();
        //             let benchmark = async move {
        //                 for _ in 0..1000_u32 {
        //                     let args = CreateJudgeArgs {
        //                         id: "0000".to_owned(),
        //                     };
        //                     if let Err(err) = this.create_judge(&ws_id, args).await {
        //                         error!(%err);
        //                         break;
        //                     }
        //                 }
        //             };

        //             tasks.push(task::spawn(benchmark));
        //         }
        //         futures::future::join_all(tasks).await;
        //         tracing::info!(duration = ?instant.elapsed(), "benchmark finished");
        //     });
        // }

        while let Some(msg) = ws.next().await {
            let msg = match msg {
                Ok(m) => m,
                Err(err) => {
                    error!(%err, "ws run error");
                    session.close_ws().await;
                    self.set_judger_offline(&session.ws_id);
                    return;
                }
            };

            let text = match msg.to_str() {
                Ok(t) => t,
                Err(()) => {
                    warn!("ignore non-text message");
                    continue;
                }
            };

            let rpc_msg = match serde_json::from_str::<RpcMessage>(text) {
                Ok(r) => r,
                Err(err) => {
                    error!(%err, ?text, "failed to parse ws text message");
                    continue;
                }
            };

            match rpc_msg {
                RpcMessage::Request { seq, body, .. } => {
                    let this = self.clone();
                    let session = session.clone();
                    task::spawn(async move {
                        let response = this.handle_rpc_request(body).await;
                        let rpc_msg = RpcMessage::Response {
                            seq,
                            time: Utc::now(),
                            body: response,
                        };
                        let ws_msg = ws::Message::text(serde_json::to_string(&rpc_msg).unwrap());
                        let _ = session.sender.send(ws_msg).await;
                    });
                }
                RpcMessage::Response { seq, body, .. } => {
                    let session = session.clone();
                    task::spawn(async move {
                        let mut callbacks = session.callbacks.lock().await;
                        match callbacks.remove(&seq) {
                            None => warn!(?seq, "no such callback"),
                            Some(cb) => match cb.send(body) {
                                Ok(()) => {}
                                Err(_) => warn!(?seq, "the callback has been cancelled"),
                            },
                        }
                    });
                }
            }
        }
    }

    fn set_judger_offline(&self, ws_id: &str) {
        if let Some(mut judger) = self.judger_map.get_mut(ws_id) {
            judger.state = JudgerState::Offline;
            judger.offline_time = Some(Utc::now());

            // TODO: notify scheduler, re-dispatch all running tasks in the judger
        }
    }

    async fn wsrpc(&self, ws_id: &str, req: RpcRequest) -> Result<RpcResponse> {
        let judger = match self.judger_map.get(ws_id) {
            Some(j) => j,
            None => return Err(format_err!("judger not found")),
        };

        let session = match judger.state {
            JudgerState::Online(ref s) => &**s,
            _ => return Err(format_err!("can not perform wsrpc on the judger")),
        };

        let seq = session.seq.fetch_add(1, Relaxed).wrapping_add(1);
        let (tx, rx) = oneshot::channel();
        let rpc_msg = RpcMessage::Request {
            seq,
            time: Utc::now(),
            body: req,
        };
        let ws_msg = ws::Message::text(serde_json::to_string(&rpc_msg).unwrap());

        {
            let mut callbacks = session.callbacks.lock().await;
            callbacks.insert(seq, tx);
            session.sender.send(ws_msg).await.unwrap();
        }

        Ok(rx.await.unwrap())
    }

    async fn handle_rpc_request(self: Arc<Self>, req: RpcRequest) -> RpcResponse {
        drop(req);
        // RpcResponse::Output(None)
        RpcResponse::Error(ErrorInfo {
            code: ErrorCode::NotSupported as u32,
            message: None,
        })
    }
}

impl JudgerModule {
    pub async fn create_judge(&self, ws_id: &str, args: CreateJudgeArgs) -> Result<()> {
        let res = self.wsrpc(ws_id, RpcRequest::CreateJudge(args)).await?;
        match res {
            RpcResponse::Output(output) => {
                if output.is_some() {
                    warn!("expected null response");
                }
            }
            RpcResponse::Error(err) => return Err(anyhow::Error::new(err)),
        }
        Ok(())
    }
}
