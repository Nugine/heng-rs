use super::cgroup::Cgroup;
use super::Args;

use std::convert::Infallible as Never;
use std::ffi::{CStr, CString};
use std::io;
use std::ptr;

use anyhow::{Context, Result};
use fcntl::OFlag;
use nix::fcntl;
use nix::sys::stat::Mode;
use nix::unistd::{self, Gid,  Uid};

pub fn run_child(args: &Args, cgroup: &Cgroup) -> Result<Never> {
    let child_pid = unistd::getpid();

    if let Some(ref stdin) = args.stdin {
        redirect_stdin(stdin).context("failed to redirect stdin")?;
    }

    if let Some(ref stdout) = args.stdout {
        redirect_stdout(stdout).context("failed to redirect stdout")?;
    }

    if let Some(ref stderr) = args.stderr {
        redirect_stderr(stderr).context("failed to redirect stderr")?;
    }

    cgroup
        .child_setup(args, child_pid)
        .context("failed to setup cgroup")?;

    cgroup
        .reset_statistics()
        .context("failed to reset cgroup statistics")?;

    if let Some(gid) = args.gid.map(Gid::from_raw) {
        unistd::setgroups(&[gid]).context("failed to set groups")?;
        unistd::setgid(gid).context("failed to set gid")?;
    }

    if let Some(uid) = args.uid.map(Uid::from_raw) {
        unistd::setuid(uid).context("failed to set uid")?;
    }

    execvp(args.bin.as_c_str(), args.args.as_slice())
}

fn redirect_stdin(stdin: &CStr) -> nix::Result<()> {
    let newfd = fcntl::open(stdin, OFlag::O_RDONLY | OFlag::O_CLOEXEC, Mode::empty())?;
    unistd::dup2(libc::STDIN_FILENO, newfd)?;
    unistd::close(newfd)?;
    Ok(())
}

fn redirect_stdout(stdout: &CStr) -> nix::Result<()> {
    let newfd = fcntl::open(
        stdout,
        OFlag::O_WRONLY | OFlag::O_TRUNC | OFlag::O_CLOEXEC,
        Mode::from_bits_truncate(0o644),
    )?;
    unistd::dup2(libc::STDOUT_FILENO, newfd)?;
    unistd::close(newfd)?;
    Ok(())
}

fn redirect_stderr(stderr: &CStr) -> nix::Result<()> {
    let newfd = fcntl::open(
        stderr,
        OFlag::O_WRONLY | OFlag::O_TRUNC | OFlag::O_CLOEXEC,
        Mode::from_bits_truncate(0o644),
    )?;
    unistd::dup2(libc::STDERR_FILENO, newfd)?;
    unistd::close(newfd)?;
    Ok(())
}

fn execvp(bin: &CStr, args: &[CString]) -> Result<Never> {
    let execvp_args: Vec<*const libc::c_char> = {
        let mut argv = Vec::with_capacity(args.len() + 2);
        argv.push(bin.as_ptr());
        argv.extend(args.iter().map(|a| a.as_ptr()));
        argv.push(ptr::null());
        argv
    };

    unsafe { libc::execvp(bin.as_ptr(), execvp_args.as_ptr()) };

    Err(io::Error::last_os_error()).context("failed to execvp")
}
