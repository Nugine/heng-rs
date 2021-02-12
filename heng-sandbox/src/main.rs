use std::io::{self, Write as _};

use anyhow::Result;
use heng_sandbox::sandbox::Args;
use structopt::StructOpt;

fn main() -> Result<()> {
    let args = Args::from_args();
    let output = heng_sandbox::sandbox::run(args)?;
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    serde_json::to_writer(&mut stdout, &output)?;
    writeln!(stdout)?;
    Ok(())
}
