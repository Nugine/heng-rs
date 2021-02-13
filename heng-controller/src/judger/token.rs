use super::{JudgerModule, JudgerState, JudgerStatus};

use crate::config::Config;

use heng_protocol::internal::http::{AcquireTokenOutput, AcquireTokenRequest};

use std::time::Duration;

use actix_web::{web, HttpResponse, Responder};
use chrono::Utc;
use tokio::{task, time};
use tracing::debug;
use uuid::Uuid;
use validator::Validate;

#[actix_web::post("/token")]
async fn acquire_token(
    module: web::Data<JudgerModule>,
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
        system_info: body.software,
        created_at: Utc::now(),
        state: JudgerState::Registered,
    };

    let _ = module.status_map.insert(ws_id.clone(), ability);
    {
        let ws_id = ws_id.clone();
        let ttl = Config::global().judger.token_ttl;
        task::spawn(async move {
            time::sleep(Duration::from_millis(ttl)).await;
            let check_remove = |status: &JudgerStatus| status.state == JudgerState::Registered;
            let item = module
                .status_map
                .remove_if(&ws_id, |_, status| check_remove(status));
            if let Some((k, v)) = item {
                debug!(ws_id=?k, status = ?v, "remove judger status");
            }
        });
    }

    let output = AcquireTokenOutput { token: ws_id };
    HttpResponse::Ok().json(&output)
}
