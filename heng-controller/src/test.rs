use crate::config::Config;

use actix_web::{get, web, HttpResponse, Responder};

pub fn register(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/test").service(show_config));
}

#[get("/config")]
async fn show_config() -> impl Responder {
    let config = Config::global();
    HttpResponse::Ok().json(config)
}
