use crate::config::Config;

use actix_web::{get, web, HttpResponse, Responder};

pub fn register(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/test").service(show_config));
}

#[get("/config")]
async fn show_config(config: web::Data<Config>) -> impl Responder {
    HttpResponse::Ok().json(config.as_ref())
}
