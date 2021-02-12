mod cgroup;
mod child;
mod parent;

use self::cgroup::Cgroup;

use std::ffi::CString;
use std::fs::File;
use std::io;
use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context as _, Result};
use nix::unistd;
use serde::{Deserialize, Serialize};
use structopt::StructOpt;

#[derive(Debug, Serialize, Deserialize, StructOpt)]
pub struct Args {
    #[structopt(parse(try_from_str = CString::new))]
    pub bin: CString,

    #[structopt(parse(try_from_str = CString::new))]
    pub args: Vec<CString>,

    #[structopt(long, parse(try_from_str = CString::new))]
    pub stdin: Option<CString>,

    #[structopt(long, parse(try_from_str = CString::new))]
    pub stdout: Option<CString>,

    #[structopt(long, parse(try_from_str = CString::new))]
    pub stderr: Option<CString>,

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
pub struct Output {
    pub code: i32,
    pub signal: i32,
    pub status: i32,

    pub real_time: u64, // milliseconds
    pub sys_time: u64,  // milliseconds
    pub user_time: u64, // milliseconds
    pub cpu_time: u64,  // milliseconds
    pub memory: u64,    // KiB
}

pub fn run(args: Args) -> Result<()> {
    let cgroup = Cgroup::new(rand::random()).context("failed to create cgroup")?;

    let t0 = Instant::now();
    match unsafe { unistd::fork() }.context("failed to fork")? {
        unistd::ForkResult::Parent { child } => {
            env_logger::init();
            let output = parent::run_parent(&args, child, t0, &cgroup)?;
            write_output(&args, &output)
        }
        unistd::ForkResult::Child => {
            child::run_child(&args, &cgroup)?;
            unreachable!() // after evecvp
        }
    }
}

fn write_output(args: &Args, output: &Output) -> Result<()> {
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
