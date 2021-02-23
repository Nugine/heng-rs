#![deny(clippy::all)]

mod auth;
mod config;
mod errors;
mod external;
mod judger;
mod redis;
mod routes;

use self::auth::AuthModule;
pub use self::config::Config;
use self::external::ExternalModule;
use self::judger::JudgerModule;
use self::redis::RedisModule;

use heng_utils::container::{inject, Container};

use std::net::SocketAddr;
use std::sync::Arc;

pub use anyhow::Result;

pub fn init(config: Config) -> Result<()> {
    let config = Arc::new(config);

    let redis_module = Arc::new(RedisModule::new(&config)?);
    let judger_module = Arc::new(JudgerModule::new());
    let external_module = Arc::new(ExternalModule::new(redis_module.clone()));
    let auth_module = Arc::new(AuthModule::new(&config, redis_module.clone()));

    let mut container = Container::new();

    container.register(config);
    container.register(redis_module);
    container.register(judger_module);
    container.register(external_module);
    container.register(auth_module);

    container.install_global();
    Ok(())
}

pub async fn run() -> Result<()> {
    {
        let module = inject::<JudgerModule>();
        tokio::task::spawn(module.__test_schedule());
    }

    let config: Arc<Config> = inject();
    let addr = config.server.address.parse::<SocketAddr>()?;
    let server = warp::serve(routes::routes());
    server.bind(addr).await;
    Ok(())
}
