use std::time::Duration;

use actix_web::{web, HttpResponse, Responder};
use anyhow::Result;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use tokio::{task, time};
use tracing::{debug, info};
use uuid::Uuid;
use validator::Validate;

use crate::config::Config;

pub fn register() -> Result<impl Fn(&mut web::ServiceConfig) + Clone> {
    info!("initializing judger module");
    let state = web::Data::new(JudgerModule::default());
    info!("judger module is initialized");

    Ok(move |cfg: &mut web::ServiceConfig| {
        cfg.app_data(state.clone());
        cfg.service(web::scope("/judger").service(acquire_token));
    })
}

#[derive(Default)]
pub struct JudgerModule {
    status_map: DashMap<String, JudgerStatus>,
}

#[derive(Debug, Validate, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AcquireTokenRequest {
    #[validate(range(min = 1, max = 64))]
    max_task_count: u32,

    #[validate(length(max = 256))]
    name: Option<String>,

    core_count: Option<u32>,

    #[validate(length(max = 256))]
    system_info: Option<String>,
}

#[derive(Debug, Serialize)]
struct AcquireTokenOutput {
    token: String,
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

#[actix_web::post("/token")]
async fn acquire_token(
    state: web::Data<JudgerModule>,
    body: web::Json<AcquireTokenRequest>,
) -> impl Responder {
    let body = body.0;
    if let Err(err) = body.validate() {
        return HttpResponse::BadRequest().body(err.to_string());
    }

    // TODO: validate AK and SK

    let ws_id = Uuid::new_v4().to_string();

    let ability = JudgerStatus {
        max_task_count: body.max_task_count,
        name: body.name,
        core_count: body.core_count,
        system_info: body.system_info,
        created_at: Utc::now(),
        state: JudgerState::Registered,
    };

    let _ = state.status_map.insert(ws_id.clone(), ability);
    {
        let ws_id = ws_id.clone();
        let ttl = Config::global().judger.token_ttl;
        task::spawn(async move {
            time::sleep(Duration::from_millis(ttl)).await;
            let check_remove = |status: &JudgerStatus| status.state == JudgerState::Registered;
            let item = state
                .status_map
                .remove_if(&ws_id, |_, status| check_remove(status));
            if let Some((k, v)) = item {
                debug!(ws_id=?k, status = ?v, "remove judger status");
            }
        });
    }

    let output = AcquireTokenOutput { token: ws_id };
    HttpResponse::Ok().json(output)
}
