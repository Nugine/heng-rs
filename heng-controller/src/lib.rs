#![deny(clippy::all)]

pub mod config;

pub mod test;

use crate::config::Config;

use actix_web::{web, App, HttpServer};
use anyhow::Result;
use tracing::info;

fn register(cfg: &mut web::ServiceConfig) {
    crate::test::register(cfg);
}

const GLOBAL_PREFIX: &str = "/v1";

pub async fn run(config: Config) -> Result<()> {
    let host = config.server.host.clone();
    let port = config.server.port;
    let config = web::Data::new(config);

    let server: _ = HttpServer::new(move || {
        App::new()
            .app_data(config.clone())
            .service(web::scope(GLOBAL_PREFIX).configure(register))
    });

    let server = server.bind((host.as_str(), port))?;

    info!("server is listening {}:{}", host, port);

    server.run().await?;

    Ok(())
}
