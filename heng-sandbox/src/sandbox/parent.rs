use super::cgroup::Cgroup;
use super::{Args, Output};

use std::io;
use std::mem::MaybeUninit;
use std::time::Instant;

use anyhow::{Context, Result};
use log::debug;
use nix::unistd::Pid;

pub fn run_parent(_args: &Args, child_pid: Pid, t0: Instant, cgroup: &Cgroup) -> Result<Output> {
    let (status, rusage) = wait4(child_pid).context("failed to wait4")?;

    debug!("status = {:?}", status);
    debug!("rusage = {:?}", rusage);

    let real_duration = t0.elapsed();
    let real_time: u64 = real_duration.as_millis() as u64;
    debug!("real_duration = {:?}", real_duration);

    let code = libc::WEXITSTATUS(status);
    let signal = libc::WTERMSIG(status);

    debug!("code   = {}", code);
    debug!("signal = {}", signal);

    let s = {
        let ret1 = cgroup
            .collect_statistics()
            .context("failed to collect statistics from cgroup");
        let ret2 = cgroup.parent_cleanup().context("failed to cleanup cgroup");
        ret2.and(ret1)?
    };

    debug!("statistics = {:?}", s);

    Ok(Output {
        code,
        signal,
        status,
        real_time,
        sys_time: s.sys_time,
        user_time: s.user_time,
        cpu_time: s.cpu_time,
        memory: s.memory,
    })
}

fn wait4(child_pid: Pid) -> io::Result<(i32, libc::rusage)> {
    let pid = child_pid.as_raw();
    let mut status: i32 = 0;
    let mut rusage: MaybeUninit<libc::rusage> = MaybeUninit::zeroed();

    unsafe {
        let ret = libc::wait4(pid, &mut status, libc::WUNTRACED, rusage.as_mut_ptr());
        if ret < 0 {
            return Err(io::Error::last_os_error());
        }

        let rusage = rusage.assume_init();
        Ok((status, rusage))
    }
}
