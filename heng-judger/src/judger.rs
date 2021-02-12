use crate::redis::RedisModule;

use heng_protocol::internal::ws_json::Message as WsMessage;

use std::collections::HashMap;
use std::sync::atomic::AtomicU32;

use chrono::Utc;
use tokio::sync::{mpsc, oneshot, Mutex};
use tracing::{debug, error, warn};

pub struct Judger {
    sender: mpsc::Sender<WsMessage>,
    seq: AtomicU32,
    callbacks: Mutex<HashMap<u32, oneshot::Sender<WsMessage>>>,
    redis: RedisModule,
}

impl Judger {
    #[allow(clippy::new_without_default)]
    pub fn new(sender: mpsc::Sender<WsMessage>, redis: RedisModule) -> Self {
        Self {
            sender,
            seq: AtomicU32::new(0),
            callbacks: Mutex::new(HashMap::new()),
            redis,
        }
    }

    pub async fn handle_controller_message(&self, msg: WsMessage) {
        match msg {
            WsMessage::Request { seq, time, .. } => {
                // let now = Utc::now();
                // debug!(send_time=?time,recv_time=?now,"duration from controller to judger: {}",now-time);

                let res = WsMessage::Response {
                    seq,
                    time: Utc::now(),
                    body: None,
                };

                if let Err(err) = self.sender.send(res).await {
                    error!(%err,"ws send failed");
                }
            }
            WsMessage::Response { seq, ref time, .. } => {
                let mut callbacks = self.callbacks.lock().await;
                match callbacks.remove(&seq) {
                    Some(cb) => {
                        if let Err(msg) = cb.send(msg) {
                            if let WsMessage::Response { seq, time, .. } = msg {
                                warn!(?seq, ?time, "the callback has been cancelled");
                            }
                        }
                    }
                    None => {
                        warn!(
                            ?seq,
                            ?time,
                            "can not find a callback waiting for the response"
                        );
                        drop(msg);
                    }
                }
            }
        }
    }
}
