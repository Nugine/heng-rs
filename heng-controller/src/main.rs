use heng_controller::{App, Config};

use std::env;

use anyhow::Result;
use dotenv::dotenv;
use tracing::info;

const CONFIG_PATH: &str = "heng-controller.toml";

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    setup_tracing();

    let config = load_config()?;
    let app = App::new(config).await?;
    app.run().await
}

#[tracing::instrument(err)]
fn load_config() -> Result<Config> {
    let path = env::current_dir()?.join(CONFIG_PATH);

    info!("loading config from {}", path.display());
    let config = Config::new_from_file(&path)?;
    info!("config is loaded:\n{:#?}", config);

    Ok(config)
}

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
