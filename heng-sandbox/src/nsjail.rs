use crate::{SandboxArgs, SandboxOutput};

use std::path::PathBuf;
use std::process::{Command, Stdio};

use anyhow::{format_err, Context, Result};
use log::{debug, error};

pub struct NsjailArgs {
    pub config: PathBuf,
    pub workspace: PathBuf,
    pub time_limit: Option<u32>, // seconds
}

pub fn exec(nsjail: &NsjailArgs, sandbox: &SandboxArgs) -> Result<SandboxOutput> {
    if sandbox.stdin.is_none() || sandbox.stdout.is_none() || sandbox.stderr.is_none() {
        panic!("sandbox std streams must be specified");
    }

    let mut cmd = Command::new("nsjail");

    cmd.arg("-C").arg(&nsjail.config);

    cmd.arg("-D").arg(&nsjail.workspace);
    cmd.arg("-B").arg(&nsjail.workspace);

    if let Some(time) = nsjail.time_limit {
        cmd.arg("-t").arg(time.to_string());
    }

    cmd.arg("--");
    cmd.arg("/usr/local/bin/heng-sandbox");

    sandbox.serialize_into_cmd(&mut cmd);

    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    debug!("executing command\n{:?}\n", cmd);

    let child = cmd
        .spawn()
        .context("failed to spawn child process")?
        .wait_with_output()
        .context("failed to wait child process")?;

    if child.status.success() {
        let output: SandboxOutput =
            serde_json::from_slice(&child.stdout).context("failed to parse sandbox json output")?;
        Ok(output)
    } else {
        let err = String::from_utf8(child.stderr).unwrap();
        error!("child process failed:\n{}\n", err);
        Err(format_err!("child process failed"))
    }
}
