use super::{JudgerModule, JudgerState};

use chrono::Utc;
use heng_protocol::internal::ws_json::Message as WsMessage;

use std::collections::HashMap;
use std::sync::Arc;

use actix::{Actor, AsyncContext, Handler, StreamHandler};
use actix_web::{web, HttpRequest, HttpResponse, Responder};
use actix_web_actors::ws;
use serde::Deserialize;
use tokio::sync::oneshot;
use tracing::{error, info, warn};

#[derive(Deserialize)]
struct WebsocketQuery {
    token: String,
}

#[actix_web::get("/websocket")]
async fn websocket(
    module: web::Data<JudgerModule>,
    query: web::Query<WebsocketQuery>,
    req: HttpRequest,
    stream: web::Payload,
) -> impl Responder {
    let ws_id = query.into_inner().token;

    match module.status_map.get_mut(&ws_id) {
        Some(mut status) if status.state == JudgerState::Registered => {
            status.state = JudgerState::Online;
        }
        _ => return Ok(HttpResponse::Forbidden().finish()),
    };

    info!(?ws_id, "new ws session");

    let session = WsSession::new(ws_id, module.into_inner());
    ws::start(session, &req, stream)
}

pub struct WsSession {
    ws_id: String,
    seq: u32,
    callbacks: HashMap<u32, oneshot::Sender<WsActorResponse>>,
    module: Arc<JudgerModule>,
}

impl WsSession {
    fn new(ws_id: String, module: Arc<JudgerModule>) -> Self {
        WsSession {
            ws_id,
            seq: 0,
            callbacks: HashMap::new(),
            module,
        }
    }
}

pub struct WsActorRequest(pub heng_protocol::internal::ws_json::Request);

pub struct WsActorResponse(pub heng_protocol::internal::ws_json::Response);

impl actix::Message for WsActorRequest {
    type Result = oneshot::Receiver<WsActorResponse>;
}

impl Actor for WsSession {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let addr = ctx.address();
        let session_map = &self.module.session_map;
        session_map.insert(self.ws_id.clone(), addr.downgrade());

        // let ws_id = self.ws_id.clone();
        // let module = self.module.clone();
        // task::spawn(async move {
        //     let instant = time::Instant::now();
        //     let mut tasks = Vec::new();
        //     for _ in 0..1000 {
        //         use heng_protocol::internal::ws_json::JudgeArgs;
        //         let ws_id = ws_id.clone();
        //         let module = module.clone();
        //         let benchmark = async move {
        //             for _ in 0..1000_u32 {
        //                 let args = JudgeArgs { placeholder: 0 };
        //                 if let Err(err) = module.create_judge(&ws_id, args).await {
        //                     error!(%err);
        //                     break;
        //                 }
        //             }
        //         };

        //         tasks.push(task::spawn(benchmark));
        //     }
        //     futures::future::join_all(tasks).await;
        //     info!(duration = ?instant.elapsed(), "benchmark finished");
        // });
    }

    fn stopped(&mut self, _: &mut Self::Context) {
        info!(ws_id = ?self.ws_id, "ws session stopped");

        self.module.session_map.remove(&self.ws_id);
        self.module.status_map.remove(&self.ws_id);
    }
}

impl Handler<WsActorRequest> for WsSession {
    type Result = actix::MessageResult<WsActorRequest>;

    fn handle(&mut self, msg: WsActorRequest, ctx: &mut Self::Context) -> Self::Result {
        actix::MessageResult(self.wsrpc(msg, ctx))
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsSession {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Text(text)) => match serde_json::from_str::<WsMessage>(&text) {
                Ok(msg) => self.handle_judger_message(msg, ctx),
                Err(err) => {
                    error!(%err, "close judger ws session");
                    ctx.close(Some(ws::CloseReason {
                        code: ws::CloseCode::Abnormal,
                        description: Some("internal protocol: message format error".to_owned()),
                    }));
                }
            },
            Err(err) => {
                error!(%err, "close judger ws session");
                ctx.close(None)
            }
            Ok(_) => {
                warn!("drop ws message");
                drop(msg);
            }
        }
    }
}

impl WsSession {
    fn handle_judger_message(&mut self, msg: WsMessage, ctx: &mut ws::WebsocketContext<Self>) {
        match msg {
            WsMessage::Request { seq, .. } => {
                warn!("TODO handle judger request")
            }
            WsMessage::Response {
                seq,
                ref time,
                body,
            } => match self.callbacks.remove(&seq) {
                Some(cb) => {
                    if cb.send(WsActorResponse(body)).is_err() {
                        warn!(?seq, ?time, "the callback has been cancelled")
                    }
                }
                None => {
                    warn!(
                        ?seq,
                        ?time,
                        "can not find a callback waiting for the response"
                    );
                }
            },
        }
    }

    fn wsrpc(
        &mut self,
        msg: WsActorRequest,
        ctx: &mut ws::WebsocketContext<Self>,
    ) -> oneshot::Receiver<WsActorResponse> {
        self.seq = self.seq.wrapping_add(1);
        let seq = self.seq;

        let ws_msg = WsMessage::Request {
            seq,
            time: Utc::now(),
            body: msg.0,
        };

        let (tx, rx) = oneshot::channel();
        self.callbacks.insert(seq, tx);

        ctx.text(serde_json::to_string(&ws_msg).unwrap());

        rx
    }
}
