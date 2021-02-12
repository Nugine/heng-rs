use std::env;

use heng_controller::config::Config;

use anyhow::Result;
use dotenv::dotenv;
use tracing::info;

fn setup_tracing() {
    use tracing_error::ErrorSubscriber;
    use tracing_subscriber::{
        subscribe::CollectExt,
        util::SubscriberInitExt,
        {fmt, EnvFilter},
    };

    tracing_subscriber::fmt()
        .event_format(fmt::format::Format::default().pretty())
        .with_env_filter(EnvFilter::from_default_env())
        .with_timer(fmt::time::ChronoLocal::rfc3339())
        .finish()
        .with(ErrorSubscriber::default())
        .init();
}

const CONFIG_PATH: &str = "heng-controller.toml";

#[tracing::instrument(err)]
fn load_config() -> Result<()> {
    let path = env::current_dir()?.join(CONFIG_PATH);

    info!("loading config from {}", path.display());
    let config = Config::init_from_file(&path)?;
    info!("config is loaded:\n{:#?}", config);

    Ok(())
}

#[actix_web::main]
async fn main() -> Result<()> {
    dotenv().ok();
    setup_tracing();

    load_config()?;
    heng_controller::run().await
}
