mod config;
mod error_code;
mod errors;
mod judger;
mod redis;
mod routes;

pub use self::config::Config;
use self::judger::JudgerModule;
use self::redis::RedisModule;

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;

pub struct App {
    redis_module: RedisModule,
    judger_module: Arc<JudgerModule>,
}

impl App {
    pub async fn new() -> Result<Arc<Self>> {
        let redis_module = RedisModule::new()?;
        let judger_module = JudgerModule::new()?;
        let app = Self {
            redis_module,
            judger_module,
        };
        Ok(Arc::new(app))
    }

    pub async fn run(self: Arc<Self>) -> Result<()> {
        let config = Config::global();
        let addr = config.server.address.parse::<SocketAddr>()?;
        let server = warp::serve(routes::routes(self));
        server.bind(addr).await;
        Ok(())
    }
}
