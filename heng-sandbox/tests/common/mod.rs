use std::sync::Once;

use heng_sandbox::{SandboxArgs, SandboxOutput};

use anyhow::Result;
use log::{debug, error};

pub fn init() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        dotenv::dotenv().ok();
        env_logger::init();
    });
}

pub fn run(args: &SandboxArgs) -> Result<SandboxOutput> {
    debug!("sandbox args = {:?}", args);
    match heng_sandbox::run(&args) {
        Ok(output) => {
            debug!("sandbox output = {:?}", output);
            Ok(output)
        }
        Err(err) => {
            error!("sandbox error:\n{:?}", err);
            Err(err)
        }
    }
}
