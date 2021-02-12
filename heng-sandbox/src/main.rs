use std::env;
use std::io;

use anyhow::Result;
use heng_sandbox::sandbox::SandboxArgs;
use io::Read;
use structopt::StructOpt;

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

fn main() -> Result<()> {
    let args = load_args()?;
    heng_sandbox::sandbox::run(args)?;
    Ok(())
}
