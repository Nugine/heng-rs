#![deny(clippy::all)]

pub mod config;
pub mod redis;
pub mod test;

// -------------------------------------------------------------------------

use crate::config::Config;

use actix_web::{web, App, HttpServer};
use anyhow::Result;
use tracing::info;

const GLOBAL_PREFIX: &str = "/v1";

pub async fn run() -> Result<()> {
    let config = Config::global();

    let test = self::test::register()?;
    let redis = self::redis::register()?;

    // build server
    let server: _ = HttpServer::new(move || {
        App::new().service(
            web::scope(GLOBAL_PREFIX)
                .configure(test.clone())
                .configure(redis.clone()),
        )
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
