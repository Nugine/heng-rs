#![deny(clippy::all)]

mod cgroup;
mod child;
mod parent;
mod signal;

pub mod nsjail;

use self::cgroup::Cgroup;

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
    pub bin: String,

    pub args: Vec<String>,

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
    pub rlimit_cpu: Option<u32>, // seconds

    #[structopt(long)]
    pub memory_limit: Option<u64>, // KiB

    #[structopt(long)]
    pub real_time_limit: Option<u64>, // milliseconds

    #[structopt(long)]
    pub max_pids_limit: Option<u32>,
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

        if let Some(memory_limit) = self.memory_limit {
            cmd.arg("--memory-limit").arg(memory_limit.to_string());
        }

        if let Some(max_pids_limit) = self.max_pids_limit {
            cmd.arg("--max-pids-limit").arg(max_pids_limit.to_string());
        }

        cmd.arg("--");
        cmd.arg(&self.bin);
        cmd.args(&self.args);
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
    let cgroup = Cgroup::new(rand::random()).context("failed to create cgroup")?;

    let t0 = Instant::now();
    match unsafe { unistd::fork() }.context("failed to fork")? {
        unistd::ForkResult::Parent { child } => {
            let output = parent::run_parent(&args, child, t0, &cgroup)?;
            Ok(output)
        }
        unistd::ForkResult::Child => {
            if let Err(err) = child::run_child(&args, &cgroup) {
                eprintln!("failed to prepare child: {:?}", err);
                process::exit(101);
            }
            unreachable!() // after evecvp
        }
    }
}
