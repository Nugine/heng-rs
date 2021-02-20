use crate::Config;

use heng_protocol::internal::ws_json::{
    CreateJudgeArgs, Message as RpcMessage, Request as RpcRequest, Response as RpcResponse,
};

use std::collections::HashMap;
use std::mem;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::{Arc, Weak};
use std::time::Duration;

use anyhow::{format_err, Result};
use chrono::Utc;
use futures::stream::SplitStream;
use futures::{StreamExt, TryFutureExt};
use tokio::sync::{mpsc, oneshot, Mutex, RwLock};
use tokio::task::{self, JoinHandle};
use tokio::time;
use tokio_stream::wrappers::ReceiverStream;
use tracing::{debug, error, warn};
use uuid::Uuid;
use warp::ws::{self, WebSocket};

pub struct JudgerModule {
    judger_map: RwLock<HashMap<Arc<str>, Arc<Judger>>>,
}

pub struct Judger {
    ws_id: Arc<str>,
    module: Weak<JudgerModule>,
    info: JudgerInfo,
    state: RwLock<JudgerState>,
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
    seq: AtomicU32,
    callbacks: Mutex<HashMap<u32, oneshot::Sender<RpcResponse>>>,
    sender: mpsc::Sender<ws::Message>,
}

impl JudgerModule {
    pub fn new() -> Result<Arc<Self>> {
        Ok(Arc::new(Self {
            judger_map: RwLock::new(HashMap::new()),
        }))
    }

    pub async fn register_judger(self: Arc<Self>, info: JudgerInfo) -> Result<Arc<str>> {
        let ws_id: Arc<str> = Uuid::new_v4().to_string().into();

        let remove_task = {
            let ws_id = ws_id.clone();
            let token_ttl = Config::global().judger.token_ttl;
            let this = self.clone();
            task::spawn(async move {
                time::sleep(Duration::from_millis(token_ttl)).await;
                let mut judger_map: _ = this.judger_map.write().await;
                let to_remove = match judger_map.get(&ws_id) {
                    Some(judger) => {
                        let judger_state = judger.state.read().await;
                        matches!(&*judger_state, JudgerState::Registered { .. })
                    }
                    None => false,
                };
                if to_remove {
                    judger_map.remove(&ws_id).unwrap();
                    debug!(?ws_id, "remove registered judger");
                }
            })
        };

        let judger = Arc::new(Judger {
            ws_id: ws_id.clone(),
            module: Arc::downgrade(&self),
            info,
            state: RwLock::new(JudgerState::Registered { remove_task }),
        });

        let mut judger_map: _ = self.judger_map.write().await;
        let _ = judger_map.insert(ws_id.clone(), judger);

        Ok(ws_id)
    }

    pub async fn find_judger(&self, ws_id: &str) -> Option<Arc<Judger>> {
        self.judger_map.read().await.get(ws_id).map(Arc::clone)
    }
}

impl Judger {
    pub async fn is_registered(&self) -> bool {
        let state = self.state.read().await;
        matches!(*state, JudgerState::Registered { .. })
    }

    pub async fn start_session(self: Arc<Self>, ws: WebSocket) {
        let (ws_sink, ws_stream) = ws.split();

        let (tx, rx) = mpsc::channel::<ws::Message>(4096);
        task::spawn(
            ReceiverStream::new(rx)
                .map(Ok)
                .forward(ws_sink)
                .inspect_err(|err| error!(%err, "ws forward error")),
        );

        let session = Arc::new(WsSession {
            seq: AtomicU32::new(0),
            callbacks: Mutex::new(HashMap::new()),
            sender: tx,
        });

        {
            let mut state = self.state.write().await;
            if !matches!(*state, JudgerState::Registered { .. }) {
                warn!(ws_id = ?self.ws_id, "judger is already connected");
                let close_msg = ws::Message::close_with(1011_u16, "judger is already connected");
                let _ = session.sender.send(close_msg).await;
            }

            let prev_state = mem::replace(&mut *state, JudgerState::Online(session.clone()));

            if let JudgerState::Registered { remove_task } = prev_state {
                remove_task.abort();
            } else {
                unreachable!()
            }
        }

        task::spawn(self.run_session(session, ws_stream));
    }

    async fn run_session(self: Arc<Self>, session: Arc<WsSession>, mut ws: SplitStream<WebSocket>) {
        {
            let this = self.clone();
            task::spawn(this.__test_benchmark());
        }

        while let Some(msg) = ws.next().await {
            let msg = match msg {
                Ok(m) => m,
                Err(err) => {
                    error!(%err, "ws run error");
                    break;
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
                                Err(_) => warn!(?seq, "the callback is timeouted"),
                            },
                        }
                    });
                }
            }
        }

        self.set_offline().await
    }

    async fn set_offline(&self) {
        let mut state = self.state.write().await;
        *state = JudgerState::Offline;
        // TODO: notify scheduler, re-dispatch all running tasks in the judger
    }

    async fn wsrpc(&self, req: RpcRequest) -> Result<RpcResponse> {
        let session = match *self.state.read().await {
            JudgerState::Online(ref s) => Arc::clone(&s),
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
            session.sender.send(ws_msg).await.unwrap(); // FIXME: what if disconnected?
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

    async fn handle_rpc_request(self: Arc<Self>, req: RpcRequest) -> RpcResponse {
        drop(req);
        RpcResponse::Output(None)
        // RpcResponse::Error(ErrorInfo {
        //     code: ErrorCode::NotSupported as u32,
        //     message: None,
        // })
    }

    pub async fn create_judge(&self, args: CreateJudgeArgs) -> Result<()> {
        let res = self.wsrpc(RpcRequest::CreateJudge(args)).await?;
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

    async fn __test_benchmark(self: Arc<Self>) {
        tracing::info!("starting benchmark");
        let instant = time::Instant::now();
        let mut tasks = Vec::new();
        for _ in 0..1000 {
            let this = self.clone();
            let benchmark = async move {
                for _ in 0..1000_u32 {
                    let args = CreateJudgeArgs {
                        id: "0000".to_owned(),
                    };
                    if let Err(err) = this.create_judge(args).await {
                        error!(%err);
                        break;
                    }
                }
            };

            tasks.push(task::spawn(benchmark));
        }
        futures::future::join_all(tasks).await;
        tracing::info!(duration = ?instant.elapsed(), "benchmark finished");
    }
}
