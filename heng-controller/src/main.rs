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

    load_config()?;
    let app = App::new().await?;
    app.run().await
}

#[tracing::instrument(err)]
fn load_config() -> Result<()> {
    let path = env::current_dir()?.join(CONFIG_PATH);

    info!("loading config from {}", path.display());
    let config = Config::init_from_file(&path)?;
    info!("config is loaded:\n{:#?}", config);

    Ok(())
}

fn setup_tracing() {
    use tracing_error::ErrorLayer;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;
    use tracing_subscriber::{fmt, EnvFilter};

    tracing_subscriber::fmt()
        .event_format(fmt::format::Format::default().pretty())
        .with_env_filter(EnvFilter::from_default_env())
        .with_timer(fmt::time::ChronoLocal::rfc3339())
        .finish()
        .with(ErrorLayer::default())
        .init();
}
