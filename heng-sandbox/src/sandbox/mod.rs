mod cgroup;
mod child;
mod parent;

use self::cgroup::Cgroup;

use std::fs::File;
use std::io;
use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

use anyhow::{Context as _, Result};
use nix::unistd;
use serde::{Deserialize, Serialize};
use structopt::StructOpt;

#[derive(Debug, Serialize, Deserialize, StructOpt)]
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
    pub limit_max_pids: Option<u32>,

    #[structopt(short = "o", long)]
    pub sandbox_output: Option<PathBuf>,
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

pub fn run(args: SandboxArgs) -> Result<()> {
    let cgroup = Cgroup::new(rand::random()).context("failed to create cgroup")?;

    let t0 = Instant::now();
    match unsafe { unistd::fork() }.context("failed to fork")? {
        unistd::ForkResult::Parent { child } => {
            env_logger::init();
            let output = parent::run_parent(&args, child, t0, &cgroup)?;
            write_output(&args, &output)
        }
        unistd::ForkResult::Child => {
            if let Err(err) = child::run_child(&args, &cgroup) {
                eprintln!("{:?}", err);
                panic!("failed to prepare child");
            }
            unreachable!() // after evecvp
        }
    }
}

fn write_output(args: &SandboxArgs, output: &SandboxOutput) -> Result<()> {
    let stdout;
    let mut stdout_lock;
    let mut out_file;

    let out: &mut dyn io::Write = match args.sandbox_output {
        Some(ref path) => {
            out_file = File::create(&path)?;
            &mut out_file
        }
        None => {
            stdout = io::stdout();
            stdout_lock = stdout.lock();
            &mut stdout_lock
        }
    };

    serde_json::to_writer(&mut *out, &output)?;
    writeln!(out)?;
    Ok(())
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

        if let Some(limit_max_pids) = self.limit_max_pids {
            cmd.arg("--limit_max_pids").arg(limit_max_pids.to_string());
        }

        if let Some(ref sandbox_output) = self.sandbox_output {
            cmd.arg("-o").arg(sandbox_output);
        }

        cmd.arg("--");
        cmd.arg(&self.bin);
        cmd.args(&self.args);
    }
}
