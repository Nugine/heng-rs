use crate::redis::RedisModule;
use crate::utils::ResultExt;

use heng_protocol::internal::ws_json::{
    CreateJudgeArgs, FinishJudgeArgs, Message as WsMessage, ReportStatusArgs, Request, Response,
    UpdateJudgeArgs,
};
use heng_protocol::internal::{
    ConnectionSettings, ErrorInfo, JudgeState, PartialConnectionSettings,
};

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering::Relaxed};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use chrono::Utc;
use dashmap::DashMap;
use serde::Serialize;
use serde_json::value::RawValue;
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::{task, time};
use tracing::{debug, error, warn};

pub struct Judger {
    sender: mpsc::Sender<WsMessage>,
    seq: AtomicU32,
    callbacks: DashMap<u32, oneshot::Sender<Response>>,
    redis: RedisModule,
    settings: Settings,
    counter: Mutex<Counter>,
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
    pub fn new(sender: mpsc::Sender<WsMessage>, redis: RedisModule) -> Self {
        Self {
            sender,
            seq: AtomicU32::new(0),
            callbacks: DashMap::new(),
            redis,
            settings: Settings {
                status_report_interval: AtomicU64::new(1000),
            },
            counter: Mutex::new(Counter {
                pending: 0,
                judging: 0,
                finished: 0,
            }),
        }
    }

    pub async fn wsrpc(&self, body: Request) -> Result<Response> {
        let seq = self.seq.fetch_add(1, Relaxed).wrapping_add(1);

        let ws_msg = WsMessage::Request {
            seq,
            time: Utc::now(),
            body,
        };

        let (tx, rx) = oneshot::channel();
        self.callbacks.insert(seq, tx);

        self.sender.send(ws_msg).await.unwrap();

        let res = rx
            .await
            .inspect_err(|err| error!(%err,"failed to receive a response"))?;

        Ok(res)
    }

    pub async fn report_status_loop(&self) -> Result<()> {
        loop {
            let delay = self.settings.status_report_interval.load(Relaxed);
            time::sleep(Duration::from_millis(delay)).await;

            let result = self
                .wsrpc(Request::ReportStatus(ReportStatusArgs {
                    collect_time: Utc::now(),
                    next_report_time: Utc::now() + chrono::Duration::milliseconds(delay as i64),
                    report: None, // FIXME
                }))
                .await;

            let cnt = self.count(|cnt| cnt.clone()).await;

            match result {
                Ok(Response::Output(None)) => debug!(interval=?delay, count=?cnt, "report status"),
                Ok(Response::Output(Some(value))) => warn!(?value, "unexpected response"),
                Ok(Response::Error(err)) => warn!(%err, "report status"),
                Err(_) => warn!("the request failed"),
            }
        }
    }

    pub async fn handle_controller_message(self: Arc<Self>, msg: WsMessage) {
        match msg {
            WsMessage::Request {
                seq,
                time: _time,
                body,
            } => {
                let res_body = match body {
                    Request::CreateJudge(args) => {
                        to_null_response(self.clone().create_judge(args).await)
                    }
                    Request::Control(args) => to_response(self.control(args).await),
                    _ => {
                        warn!(?body, "unexpected ws request from controller");
                        drop(body);
                        return;
                    }
                };

                let res = WsMessage::Response {
                    seq,
                    time: Utc::now(),
                    body: res_body,
                };

                if let Err(err) = self.sender.send(res).await {
                    error!(%err,"ws send failed");
                }
            }
            WsMessage::Response { seq, time, body } => match self.callbacks.remove(&seq) {
                Some((_, cb)) => {
                    if cb.send(body).is_err() {
                        warn!(?seq, ?time, "the callback has been cancelled");
                    }
                }
                None => {
                    warn!(
                        ?seq,
                        ?time,
                        "can not find a callback waiting for the response"
                    );
                    drop(body);
                }
            },
        }
    }

    async fn count<T>(&self, f: impl FnOnce(&mut Counter) -> T) -> T {
        let mut counter = self.counter.lock().await;
        f(&mut counter)
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
        let res = self.wsrpc(Request::UpdateJudges(vec![update])).await?;
        let output = to_anyhow(res)?;
        if output.is_some() {
            warn!(?output, "unexpected output")
        }
        Ok(())
    }

    async fn finish_judge(&self, finish: FinishJudgeArgs) -> Result<()> {
        let res = self.wsrpc(Request::FinishJudges(vec![finish])).await?;
        let output = to_anyhow(res)?;
        if output.is_some() {
            warn!(?output, "unexpected output")
        }
        Ok(())
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
}

fn to_response<T: Serialize>(result: Result<T>) -> Response {
    match result {
        Ok(value) => {
            let raw_value = RawValue::from_string(serde_json::to_string(&value).unwrap()).unwrap();
            Response::Output(Some(raw_value))
        }
        Err(err) => {
            Response::Error(ErrorInfo {
                code: 1000, // unknown
                message: Some(err.to_string()),
            })
        }
    }
}

fn to_null_response(result: Result<()>) -> Response {
    match result {
        Ok(()) => Response::Output(None),
        Err(err) => {
            Response::Error(ErrorInfo {
                code: 1000, // unknown
                message: Some(err.to_string()),
            })
        }
    }
}

fn to_anyhow(response: Response) -> Result<Option<Box<RawValue>>> {
    match response {
        Response::Output(output) => Ok(output),
        Response::Error(err) => Err(anyhow::Error::from(err)),
    }
}
