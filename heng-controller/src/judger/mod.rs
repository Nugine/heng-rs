mod token;
mod ws;

use actix::WeakAddr;
use actix_web::web;
use anyhow::format_err;
use anyhow::Result;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use heng_protocol::internal::ws_json::{self, CreateJudgeArgs, Request, Response};
use tracing::{error, info, warn};

use crate::utils::ResultExt;

use self::ws::WsActorRequest;

pub fn register() -> Result<impl Fn(&mut web::ServiceConfig) + Clone> {
    info!("initializing judger module");
    let state = web::Data::new(JudgerModule::default());
    info!("judger module is initialized");

    Ok(move |cfg: &mut web::ServiceConfig| {
        cfg.app_data(state.clone());
        cfg.service(
            web::scope("/judger")
                .service(token::acquire_token)
                .service(ws::websocket),
        );
    })
}

#[derive(Default)]
pub struct JudgerModule {
    status_map: DashMap<String, JudgerStatus>,
    session_map: DashMap<String, WeakAddr<ws::WsSession>>,
}

#[derive(Debug)]
struct JudgerStatus {
    max_task_count: u32,
    name: Option<String>,
    core_count: Option<u32>,
    system_info: Option<String>,
    created_at: DateTime<Utc>,
    state: JudgerState,
}

#[derive(Debug, PartialEq, Eq)]
enum JudgerState {
    Registered,
    Online,
    Disabled,
    Offline,
}

impl JudgerModule {
    async fn call(&self, ws_id: &str, req: Request) -> Result<Response> {
        let addr = match self.session_map.get(ws_id) {
            Some(weak) => match weak.upgrade() {
                Some(a) => a,
                None => {
                    error!(?ws_id, "ws actor not found");
                    return Err(format_err!("ws actor not found"));
                }
            },
            None => {
                error!(?ws_id, "ws session not found");
                return Err(format_err!("ws session not found"));
            }
        };

        let rx = addr.send(WsActorRequest(req)).await.inspect_err(|err| {
            error!(%err,"can not send request to ws actor");
        })?;

        let res = rx
            .await
            .inspect_err(|err| error!(%err,"failed to receive a response"))?;

        Ok(res.0)
    }

    #[tracing::instrument(err, skip(self, args))]
    pub async fn create_judge(&self, ws_id: &str, args: CreateJudgeArgs) -> Result<()> {
        let res = self.call(ws_id, Request::CreateJudge(args)).await?;
        match res {
            ws_json::Response::Output(output) => {
                if output.is_some() {
                    warn!("expected null response");
                }
            }
            ws_json::Response::Error(err) => return Err(anyhow::Error::new(err)),
        }
        Ok(())
    }

    // fn report_status(&self, ws_id: &str, args:)
}
