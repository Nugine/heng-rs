use heng_judger::config::Config;
use heng_utils::tracing::setup_tracing;

use std::env;

use anyhow::Result;
use dotenv::dotenv;
use tracing::info;

const CONFIG_PATH: &str = "heng-judger.toml";

#[tracing::instrument(err)]
fn load_config() -> Result<()> {
    let path = env::current_dir()?.join(CONFIG_PATH);

    info!("loading config from {}", path.display());
    let config = Config::init_from_file(&path)?;
    info!("config is loaded:\n{:#?}", config);

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    setup_tracing();

    load_config()?;
    heng_judger::run().await
}
