use std::env;
use std::fs;
use std::io::{self, Read};

use anyhow::Result;
use heng_sandbox::{SandboxArgs, SandboxOutput};
use structopt::StructOpt;
use tokio::runtime;

fn load_args() -> Result<SandboxArgs> {
    let mut stdin_content = Vec::new();
    match env::var("HENG_SANDBOX_ARGPASS").as_deref() {
        Ok("JSON") => {
            std::io::stdin().read_to_end(&mut stdin_content)?;
            Ok(serde_json::from_slice(&stdin_content)?)
        }
        Ok("BINCODE") => {
            std::io::stdin().read_to_end(&mut stdin_content)?;
            Ok(bincode::deserialize(&stdin_content)?)
        }
        _ => Ok(SandboxArgs::from_args()),
    }
}

fn write_output(output: &SandboxOutput) -> Result<()> {
    let stdout;
    let mut stdout_lock;
    let mut out_file;

    let out: &mut dyn io::Write = match env::var("HENG_SANDBOX_OUTPATH") {
        Ok(path) => {
            out_file = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)?;
            &mut out_file
        }
        Err(_) => {
            stdout = io::stdout();
            stdout_lock = stdout.lock();
            &mut stdout_lock
        }
    };

    serde_json::to_writer(&mut *out, &output)?;
    writeln!(out)?;

    Ok(())
}

fn main() -> Result<()> {
    env_logger::init();
    let args = load_args()?;

    let runtime = runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .max_blocking_threads(1)
        .enable_time()
        .build()?;

    let output = {
        let _enter = runtime.enter();
        heng_sandbox::run(&args)?
    };

    write_output(&output)?;
    Ok(())
}
