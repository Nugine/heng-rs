#[macro_use]
mod utils;
mod config;
mod redis;

pub use self::config::Config;
use self::redis::Redis;

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use warp::reply::{self, Response};
use warp::{Filter, Rejection, Reply};

pub struct App {
    config: Config,
    redis: Redis,
}

impl App {
    pub async fn new(config: Config) -> Result<Arc<Self>> {
        let redis = Redis::new(&config)?;
        let app = Self { config, redis };
        Ok(Arc::new(app))
    }

    fn routes(self: Arc<Self>) -> impl_filter!() {
        warp::any()
            .map(move || self.clone())
            .and(warp::path!("v1" / "test"))
            .and(warp::get())
            .map(|app: Arc<Self>| reply::json(&app.config).into_response())
    }

    pub async fn run(self: Arc<Self>) -> Result<()> {
        let addr = self.config.server.address.parse::<SocketAddr>()?;
        let server = warp::serve(Self::routes(self));
        server.bind(addr).await;
        Ok(())
    }
}
