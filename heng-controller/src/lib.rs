#![deny(clippy::all)]

pub mod config;
pub mod redis;
pub mod test;

use crate::config::Config;
use crate::redis::Redis;

use actix_web::{web, App, HttpServer};
use anyhow::Result;
use tracing::info;

fn register(cfg: &mut web::ServiceConfig) {
    crate::test::register(cfg);
}

const GLOBAL_PREFIX: &str = "/v1";

pub async fn run() -> Result<()> {
    let config = Config::global();

    // init redis
    let redis = {
        info!("initializing redis module");
        let redis = Redis::new()?;
        info!("redis module is initialized");
        web::Data::new(redis)
    };

    // build server
    let server: _ = HttpServer::new(move || {
        App::new()
            .app_data(redis.clone())
            .service(web::scope(GLOBAL_PREFIX).configure(register))
    });

    // bind address
    let host = &config.server.host;
    let port = config.server.port;
    let server: _ = server.bind((host.as_str(), port))?;
    info!("server is listening {}:{}", host, port);

    // run server
    server.run().await?;

    Ok(())
}
