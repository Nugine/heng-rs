macro_rules! reject_error {
    ($code: expr, $msg: expr) => {
        return Err(ErrorInfo {
            code: $code,
            message: $msg,
        }
        .into())
    };
}

mod config;
mod data;
mod exec;
mod judger;
pub mod lang;
mod login;

pub use self::config::Config;
use self::data::DataModule;
use self::judger::Judger;

use heng_utils::container::{inject, Container};

use std::sync::Arc;

use anyhow::Result;

type WsStream = tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>;
type WsMessage = tokio_tungstenite::tungstenite::Message;

pub fn init(config: Config) -> Result<()> {
    let data_module = Arc::new(DataModule::new(&config)?);

    let mut container = Container::new();

    container.register(Arc::new(config));
    container.register(data_module);

    container.install_global();
    Ok(())
}

pub async fn run() -> Result<()> {
    let config = inject::<Config>();
    let remote_domain = &*config.judger.remote_domain;
    let access_key = &config.judger.access_key;
    let secret_key = &config.judger.secret_key;

    let token = login::get_token(remote_domain, access_key, secret_key).await?;
    let ws_stream = login::connect_ws(remote_domain, access_key, secret_key, &*token).await?;

    Judger::run(ws_stream).await
}
