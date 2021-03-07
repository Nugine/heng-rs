use crate::cgroup::Cgroup;
use crate::pipe::PipeRx;
use crate::signal::async_kill;
use crate::{libc_call, SandboxArgs, SandboxOutput};

use std::io;
use std::mem::MaybeUninit;
use std::time::Instant;

use anyhow::{Context, Result};
use log::debug;
use nix::unistd::Pid;
use tokio::task::JoinHandle;

pub fn run_parent(
    args: &SandboxArgs,
    child_pid: Pid,
    t0: Instant,
    cgroup: &Cgroup,
    pipe_rx: PipeRx,
) -> Result<SandboxOutput> {
    debug!("child_pid = {}", child_pid);

    let killer: Option<JoinHandle<()>> = if let Some(real_time_limit) = args.real_time_limit {
        Some(async_kill(child_pid, real_time_limit))
    } else {
        None
    };

    let child_result = pipe_rx
        .read_result()
        .context("failed to read child result")?;

    child_result.context("child process failed")?;

    let (status, rusage) = wait4(child_pid).context("failed to wait4")?;
    let real_duration = t0.elapsed();
    let real_time: u64 = real_duration.as_millis() as u64;

    debug!("status = {:?}", status);
    debug!("rusage = {:?}", rusage);
    debug!("real_duration = {:?}", real_duration);

    if let Some(handle) = killer {
        handle.abort();
    }

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

    Ok(SandboxOutput {
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

    loop {
        let ret = libc_call(|| unsafe {
            libc::wait4(pid, &mut status, libc::WUNTRACED, rusage.as_mut_ptr())
        })?;

        debug!("wait4 ret = {}, status = {}", ret, status);

        if ret > 0 {
            break;
        }
    }

    unsafe { Ok((status, rusage.assume_init())) }
}
