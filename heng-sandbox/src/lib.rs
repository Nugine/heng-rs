#![deny(clippy::all)]

mod cgroup;
mod child;
mod parent;
mod pipe;
mod signal;

pub mod nsjail;

use self::cgroup::Cgroup;

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;
use std::{io, process};

use anyhow::{Context as _, Result};
use nix::unistd;
use serde::{Deserialize, Serialize};
use structopt::StructOpt;

#[derive(Debug, Default, Serialize, Deserialize, StructOpt)]
pub struct SandboxArgs {
    pub bin: PathBuf,

    pub args: Vec<OsString>,

    #[structopt(long)]
    pub env: Vec<OsString>,

    #[structopt(long)]
    pub stdin: Option<PathBuf>,

    #[structopt(long)]
    pub stdout: Option<PathBuf>,

    #[structopt(long)]
    pub stderr: Option<PathBuf>,

    #[structopt(long)]
    pub uid: Option<u32>,

    #[structopt(long)]
    pub gid: Option<u32>,

    #[structopt(long)]
    pub real_time_limit: Option<u64>, // milliseconds

    #[structopt(long)]
    pub rlimit_cpu: Option<u32>, // seconds

    #[structopt(long)]
    pub rlimit_as: Option<u64>, // bytes

    #[structopt(long)]
    pub rlimit_data: Option<u64>, // bytes

    #[structopt(long)]
    pub rlimit_fsize: Option<u64>, // bytes

    #[structopt(long)]
    pub cg_limit_memory: Option<u64>, // bytes

    #[structopt(long)]
    pub cg_limit_max_pids: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SandboxOutput {
    pub code: i32,
    pub signal: i32,
    pub status: i32,

    pub real_time: u64, // milliseconds
    pub sys_time: u64,  // milliseconds
    pub user_time: u64, // milliseconds
    pub cpu_time: u64,  // milliseconds
    pub memory: u64,    // KiB
}

impl SandboxArgs {
    pub fn serialize_into_cmd(&self, cmd: &mut Command) {
        if let Some(ref stdin) = self.stdin {
            cmd.arg("--stdin").arg(stdin);
        }

        if let Some(ref stdout) = self.stdout {
            cmd.arg("--stdout").arg(stdout);
        }

        if let Some(ref stderr) = self.stderr {
            cmd.arg("--stderr").arg(stderr);
        }

        if let Some(uid) = self.uid {
            cmd.arg("--uid").arg(uid.to_string());
        }

        if let Some(gid) = self.gid {
            cmd.arg("--gid").arg(gid.to_string());
        }

        if let Some(real_time_limit) = self.real_time_limit {
            cmd.arg("--real-time-limit")
                .arg(real_time_limit.to_string());
        }

        if let Some(rlimit_cpu) = self.rlimit_cpu {
            cmd.arg("--rlimit-cpu").arg(rlimit_cpu.to_string());
        }

        if let Some(rlimit_as) = self.rlimit_as {
            cmd.arg("--rlimit-as").arg(rlimit_as.to_string());
        }

        if let Some(rlimit_data) = self.rlimit_data {
            cmd.arg("--rlimit-data").arg(rlimit_data.to_string());
        }
        if let Some(rlimit_fsize) = self.rlimit_fsize {
            cmd.arg("--rlimit-fsize").arg(rlimit_fsize.to_string());
        }

        if let Some(cg_limit_memory) = self.cg_limit_memory {
            cmd.arg("--cg-limit-memory")
                .arg(cg_limit_memory.to_string());
        }

        if let Some(cg_limit_max_pids) = self.cg_limit_max_pids {
            cmd.arg("--cg-limit-max-pids")
                .arg(cg_limit_max_pids.to_string());
        }

        for e in &self.env {
            cmd.arg("--env").arg(e);
        }

        cmd.arg("--");
        cmd.arg(&self.bin);
        cmd.args(&self.args);
    }
}

impl SandboxOutput {
    pub fn is_success(&self) -> bool {
        let exited = libc::WIFEXITED(self.status);
        exited && self.code == 0
    }
}

fn libc_call(f: impl FnOnce() -> i32) -> io::Result<u32> {
    let ret = f();
    if ret < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(ret as u32)
}

pub fn run(args: &SandboxArgs) -> Result<SandboxOutput> {
    if !args.bin.exists() {
        anyhow::bail!("binary file does not exist: path = {}", args.bin.display());
    }

    let cgroup = Cgroup::new(rand::random()).context("failed to create cgroup")?;
    let (pipe_tx, pipe_rx) = pipe::create().context("failed to create pipe")?;

    let t0 = Instant::now();
    match unsafe { unistd::fork() }.context("failed to fork")? {
        unistd::ForkResult::Parent { child } => {
            drop(pipe_tx);
            let output = parent::run_parent(&args, child, t0, &cgroup, pipe_rx)?;
            Ok(output)
        }
        unistd::ForkResult::Child => {
            drop(pipe_rx);
            let result = child::run_child(&args, &cgroup);
            if let Err(err) = result {
                let _ = pipe_tx.write_error(err);
            }
            process::exit(101);
        }
    }
}
