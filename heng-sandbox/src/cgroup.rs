use crate::signal::killall;
use crate::SandboxArgs;

use std::fs::File;
use std::io::Write as _;
use std::str::FromStr;
use std::{fmt, fs, io};

use anyhow::{Context, Result};
use log::{debug, warn};
use nix::sys::stat::Mode;
use nix::unistd::{self, AccessFlags, Pid};

macro_rules! cgname {
    ($controller:literal) => {
        concat!("/sys/fs/cgroup", "/", $controller, "/", "heng-sandbox")
    };

    ($controller:literal, $nonce: expr) => {
        format!("{}/{}", cgname!($controller), $nonce)
    };

    (@ensure $controller:literal, $nonce: expr) => {{
        ensure_cgroup(cgname!($controller))?;
        let cg_name = cgname!($controller, $nonce);
        ensure_cgroup(&cg_name)?;
        cg_name
    }};
}

fn ensure_cgroup(cg_dir: &str) -> Result<()> {
    if unistd::access(cg_dir, AccessFlags::F_OK).is_ok() {
        return Ok(());
    }

    unistd::mkdir(cg_dir, Mode::from_bits_truncate(0o755))
        .with_context(|| format!("fail to create cgroup directory: {}", cg_dir))?;

    Ok(())
}

fn add_pid_to_cgroup(cg_dir: &str, pid: Pid) -> Result<()> {
    let path = format!("{}/tasks", cg_dir);
    let mut file = fs::OpenOptions::new().append(true).open(path)?;
    write!(file, "{}", pid)?;
    Ok(())
}

fn write_cgroup(cg_dir: &str, file: &str, content: impl fmt::Display) -> io::Result<()> {
    let path = format!("{}/{}", cg_dir, file);
    let mut file = File::create(&path)?;
    write!(file, "{}", content)?;
    Ok(())
}

fn read_cgroup<T>(cg_dir: &str, file: &str) -> Result<T>
where
    T: FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
{
    let path = format!("{}/{}", cg_dir, file);
    let content = fs::read_to_string(path)?;
    Ok(content.trim_end().parse::<T>()?)
}

fn remove_cgroup(cg_dir: &str) {
    if let Err(err) = fs::remove_dir(cg_dir) {
        warn!("failed to remove cgroup: {}, path = {}", err, cg_dir);
    }
}

pub struct Cgroup {
    cg_cpu: String,
    cg_memory: String,
    cg_pids: String,
}

#[derive(Debug)]
pub struct Statistics {
    pub sys_time: u64,
    pub user_time: u64,
    pub cpu_time: u64,
    pub memory: u64,
}

impl Cgroup {
    pub fn new(nonce: u32) -> Result<Self> {
        debug!("cgroup nonce = {}", nonce);
        Ok(Self {
            cg_cpu: cgname!(@ensure "cpu", nonce),
            cg_memory: cgname!(@ensure "memory", nonce),
            cg_pids: cgname!(@ensure "pids",nonce),
        })
    }

    pub fn child_setup(&self, args: &SandboxArgs, child_pid: Pid) -> Result<()> {
        add_pid_to_cgroup(&self.cg_cpu, child_pid).context("failed to add pid to cgroup")?;
        add_pid_to_cgroup(&self.cg_memory, child_pid).context("failed to add pid to cgroup")?;

        if let Some(memory_limit) = args.cg_limit_memory {
            write_cgroup(&self.cg_memory, "memory.limit_in_bytes", memory_limit)
                .context("failed to set memory limit")?;
        }

        if let Some(pids_max) = args.cg_limit_max_pids {
            write_cgroup(&self.cg_pids, "pids.max", pids_max)
                .context("failed to set max pids limit")?;
            add_pid_to_cgroup(&&self.cg_pids, child_pid).context("failed to add pid to cgroup")?;
        }

        Ok(())
    }

    pub fn reset_statistics(&self) -> Result<()> {
        write_cgroup(&self.cg_cpu, "cpuacct.usage", 0)?;
        write_cgroup(&self.cg_memory, "memory.max_usage_in_bytes", 0)?;
        Ok(())
    }

    pub fn killall(&self) -> Result<()> {
        let content = fs::read_to_string(format!("{}/cgroup.procs", &self.cg_cpu))
            .context("failed to read cgroup procs")?;

        if content.is_empty() {
            return Ok(());
        }

        let mut pids = Vec::new();
        for t in content.split('\n') {
            if !t.is_empty() {
                let pid = t.parse::<i32>().unwrap();
                pids.push(Pid::from_raw(pid))
            }
        }

        killall(&pids);

        Ok(())
    }

    pub fn parent_cleanup(&self) -> Result<()> {
        self.killall()?;

        remove_cgroup(&self.cg_cpu);
        remove_cgroup(&self.cg_memory);
        remove_cgroup(&self.cg_pids);

        Ok(())
    }

    pub fn collect_statistics(&self) -> Result<Statistics> {
        let sys_time = read_cgroup::<u64>(&self.cg_cpu, "cpuacct.usage_sys")?;
        let user_time = read_cgroup::<u64>(&self.cg_cpu, "cpuacct.usage_user")?;
        let memory = read_cgroup::<u64>(&self.cg_memory, "memory.max_usage_in_bytes")?;
        let cpu_time = sys_time + user_time;

        debug!("sys_time  = {:?} ns", sys_time);
        debug!("user_time = {:?} ns", user_time);
        debug!("cpu_time  = {:?} ns", cpu_time);
        debug!("memory    = {:?} bytes", memory);

        Ok(Statistics {
            sys_time: sys_time / 1_000_000,   // ns => ms
            user_time: user_time / 1_000_000, // ns => ms
            memory: memory / 1024,            // bytes => KiB
            cpu_time: cpu_time / 1_000_000,   // ns => ms
        })
    }
}
