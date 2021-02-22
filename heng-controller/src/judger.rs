use crate::container::inject;
use crate::queue::Queue;
use crate::Config;

use heng_protocol::common as hp_common;
use heng_protocol::error::ErrorCode;
use heng_protocol::internal::ws_json::{
    CreateJudgeArgs, Message as RpcMessage, Request as RpcRequest, Response as RpcResponse,
};
use heng_protocol::internal::ErrorInfo;

use std::collections::{HashMap, HashSet};
use std::mem;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::{Arc, Weak};
use std::time::Duration;

use anyhow::{format_err, Result};
use chrono::Utc;
use dashmap::DashMap;
use futures::stream::SplitStream;
use futures::{StreamExt, TryFutureExt};
use tokio::sync::{mpsc, oneshot, Mutex, RwLock};
use tokio::task::{self, JoinHandle};
use tokio::time;
use tokio_stream::wrappers::ReceiverStream;
use tracing::{debug, error, info, warn};
use uuid::Uuid;
use warp::ws::{self, WebSocket};

pub struct JudgerModule {
    judger_map: RwLock<HashMap<Arc<str>, Arc<Judger>>>,
    available_queue: Queue<Weak<Judger>>,
}

pub struct Judger {
    module: Weak<JudgerModule>,
    ws_id: Arc<str>,
    info: JudgerInfo,
    state: RwLock<JudgerState>,
    rpc_timeout: u64,
    tasks: DashMap<Arc<str>, (UpdateCallbackSender, FinishCallbackSender)>,
}

#[derive(Debug)]
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

pub struct JudgeTask {
    pub id: Arc<str>,
    pub data: Option<hp_common::File>,
    pub dynamic_files: Option<Vec<hp_common::DynamicFile>>,
    pub judge: hp_common::Judge,
    pub test: hp_common::Test,
    pub update_callback: UpdateCallbackSender,
    pub finish_callback: FinishCallbackSender,
}

type UpdateCallbackSender = async_channel::Sender<(Arc<str>, hp_common::JudgeState)>;
type FinishCallbackSender = async_channel::Sender<(Arc<str>, hp_common::JudgeResult)>;

impl JudgerModule {
    pub fn new() -> Self {
        Self {
            judger_map: RwLock::new(HashMap::new()),
            available_queue: Queue::unbounded(),
        }
    }

    pub async fn register_judger(self: Arc<Self>, info: JudgerInfo) -> Result<Arc<str>> {
        let ws_id: Arc<str> = Uuid::new_v4().to_string().into();
        let config = inject::<Config>();

        let remove_task = {
            let ws_id = ws_id.clone();
            let token_ttl = config.judger.token_ttl;
            let module = self.clone();
            task::spawn(async move {
                time::sleep(Duration::from_millis(token_ttl)).await;
                let mut judger_map: _ = module.judger_map.write().await;
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
            module: Arc::downgrade(&self),
            ws_id: ws_id.clone(),
            info,
            state: RwLock::new(JudgerState::Registered { remove_task }),
            rpc_timeout: config.judger.rpc_timeout,
            // tasks: RwLock::new(HashMap::new()),
            tasks: DashMap::new(),
        });

        let mut judger_map: _ = self.judger_map.write().await;
        let _ = judger_map.insert(ws_id.clone(), judger);

        Ok(ws_id)
    }

    pub async fn find_judger(&self, ws_id: &str) -> Option<Arc<Judger>> {
        self.judger_map.read().await.get(ws_id).map(Arc::clone)
    }

    pub async fn schedule(self: Arc<Self>, task: JudgeTask) -> Result<()> {
        task::spawn(async move {
            loop {
                let judger = loop {
                    let weak_judger = self.available_queue.pop().await;
                    if let Some(judger) = weak_judger.upgrade() {
                        break judger;
                    }
                };

                judger.tasks.insert(
                    task.id.clone(),
                    (task.update_callback.clone(), task.finish_callback.clone()),
                );

                let args = CreateJudgeArgs {
                    id: task.id.to_string(),
                };

                if let Err(err) = judger.create_judge(args).await {
                    error!(?judger.ws_id, ?judger.info, %err, "failed to create judge");
                    judger.tasks.remove(&task.id);
                    continue;
                }

                // info!(?judger.ws_id,?task.id, "create a judge task on the judger");
                break;
            }
        });
        Ok(())
    }

    pub(crate) async fn __test_schedule(self: Arc<Self>) {
        time::sleep(Duration::from_secs(5)).await;
        let tasks_count: usize = 50_0000;
        tracing::info!(?tasks_count, "starting scheduler test");

        let instant = time::Instant::now();

        let (update_tx, _) = async_channel::bounded(4096);
        let (finish_tx, finish_rx) = async_channel::bounded(4096);

        let mut task_ids: HashSet<Arc<str>> = HashSet::new();
        for _ in 0..tasks_count {
            let id: Arc<str> = Uuid::new_v4().to_string().into();
            task_ids.insert(id.clone());
            let judge_task = JudgeTask {
                id,
                data: None,
                dynamic_files: None,
                judge: hp_common::Judge::Normal {
                    user: hp_common::Executable {
                        source: hp_common::File::Direct {
                            content: "".to_owned(),
                            hashsum: None,
                        },
                        environment: Default::default(),
                        limit: Default::default(),
                    },
                },
                test: hp_common::Test {
                    cases: Vec::new(),
                    policy: hp_common::TestPolicy::All,
                },
                update_callback: update_tx.clone(),
                finish_callback: finish_tx.clone(),
            };

            self.clone().schedule(judge_task).await.unwrap();
        }
        for i in 1..=tasks_count {
            let (task_id, result) = finish_rx.recv().await.unwrap();
            assert!(task_ids.contains(&task_id));
            // dbg!(result);

            if i % (tasks_count / 1000) == 0 {
                let progress = 100.0 * i as f64 / tasks_count as f64;
                tracing::info!(duration = ?instant.elapsed(), "scheduler test progress: {:.2} %",progress);
            }
        }
        tracing::info!(duration = ?instant.elapsed(), "scheduler test finished");
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

        {
            let module = inject::<JudgerModule>();
            let weak_judger = Arc::downgrade(&self);
            let max_task_count = self.info.max_task_count;

            for _ in 0..max_task_count {
                module.available_queue.push(weak_judger.clone()).await
            }
        }

        task::spawn(self.run_session(session, ws_stream));
    }

    async fn run_session(self: Arc<Self>, session: Arc<WsSession>, mut ws: SplitStream<WebSocket>) {
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
            if session.sender.send(ws_msg).await.is_err() {
                return Err(format_err!("judger has disconnected"));
            }
        }

        match time::timeout(Duration::from_millis(self.rpc_timeout), rx).await {
            Ok(res) => Ok(res.unwrap()),
            Err(err) => {
                let mut callbacks = session.callbacks.lock().await;
                let _ = callbacks.remove(&seq);
                return Err(anyhow::Error::new(err));
            }
        }
    }

    // judger => controller
    async fn handle_rpc_request(self: Arc<Self>, req: RpcRequest) -> RpcResponse {
        match req {
            RpcRequest::ReportStatus(status) => {
                // dbg!(status);
                RpcResponse::Output(None)
            }
            RpcRequest::UpdateJudges(update) => {
                // dbg!(update);
                RpcResponse::Output(None)
            }
            RpcRequest::FinishJudges(args) => {
                let module = self.module.upgrade().unwrap();
                // let mut tasks = self.tasks.write().await;
                for finish in args {
                    if let Some((id, (_, finish_tx))) = self.tasks.remove(&*finish.id) {
                        let _ = finish_tx.send((id, finish.result)).await;
                        module.available_queue.push(Arc::downgrade(&self)).await;
                    }
                }
                RpcResponse::Output(None)
            }
            _ => RpcResponse::Error(ErrorInfo {
                code: ErrorCode::NotSupported,
                message: None,
            }),
        }
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

    // async fn __test_benchmark(self: Arc<Self>) {
    //     tracing::info!("starting benchmark");
    //     let instant = time::Instant::now();
    //     let mut tasks = Vec::new();
    //     for _ in 0..1000 {
    //         let this = self.clone();
    //         let benchmark = async move {
    //             for _ in 0..1000_u32 {
    //                 let args = CreateJudgeArgs {
    //                     id: "0000".to_owned(),
    //                 };
    //                 if let Err(err) = this.create_judge(args).await {
    //                     error!(%err);
    //                     break;
    //                 }
    //             }
    //         };

    //         tasks.push(task::spawn(benchmark));
    //     }
    //     futures::future::join_all(tasks).await;
    //     tracing::info!(duration = ?instant.elapsed(), "benchmark finished");
    // }
}
