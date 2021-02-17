use crate::{SandboxArgs, SandboxOutput};

use std::path::PathBuf;
use std::process::{Command, Stdio};

use anyhow::{format_err, Context, Result};
use log::{debug, error};

pub struct NsjailArgs {
    config: PathBuf,
    workspace: PathBuf,
    time_limit: Option<u32>,   // seconds
    rlimit_as: Option<u32>,    // MB
    rlimit_fsize: Option<u32>, // MB
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

    if let Some(mem) = nsjail.rlimit_as {
        cmd.arg("--rlimit_as").arg(mem.to_string());
    }

    if let Some(fsize) = nsjail.rlimit_fsize {
        cmd.arg("--rlimit_fsize").arg(fsize.to_string());
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

// #[test]
// fn test_exec() {
//     use std::env;

//     env::set_var("RUST_LOG", "debug");
//     env_logger::init();

//     let nsjail_args = NsjailArgs {
//         config: "sandbox.cfg".into(),
//         workspace: "/tmp/heng-sandbox".into(),
//         time_limit: None,
//         rlimit_as: None,
//         rlimit_fsize: None,
//     };

//     let sandbox_args = SandboxArgs {
//         bin: "ls".into(),
//         args: Vec::new(),
//         stdin: Some("/dev/null".into()),
//         stdout: Some("/tmp/heng-sandbox/testout".into()),
//         stderr: Some("/tmp/heng-sandbox/testerr".into()),
//         uid: None,
//         gid: None,
//         memory_limit: None,
//         max_pids_limit: None,
//     };
//     let output = exec(&nsjail_args, &sandbox_args).unwrap();
//     dbg!(output);
// }
