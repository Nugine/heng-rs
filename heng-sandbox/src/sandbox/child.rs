use super::cgroup::Cgroup;
use super::SandboxArgs;

use std::convert::Infallible as Never;
use std::ffi::CString;
use std::io;
use std::path::Path;
use std::ptr;

use anyhow::{Context, Result};
use fcntl::OFlag;
use nix::fcntl;
use nix::sys::stat::Mode;
use nix::unistd::{self, Gid, Uid};

pub fn run_child(args: &SandboxArgs, cgroup: &Cgroup) -> Result<Never> {
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

    execvp(&args.bin, &args.args)
}

fn redirect_stdin(stdin: &Path) -> nix::Result<()> {
    let file_fd = fcntl::open(stdin, OFlag::O_RDONLY | OFlag::O_CLOEXEC, Mode::empty())?;
    unistd::dup2(file_fd, libc::STDIN_FILENO)?;
    unistd::close(file_fd)?;
    Ok(())
}

fn redirect_stdout(stdout: &Path) -> nix::Result<()> {
    let file_fd = fcntl::open(
        stdout,
        OFlag::O_WRONLY | OFlag::O_CREAT | OFlag::O_TRUNC | OFlag::O_CLOEXEC,
        Mode::from_bits_truncate(0o644),
    )?;
    unistd::dup2(file_fd, libc::STDOUT_FILENO)?;
    unistd::close(file_fd)?;
    Ok(())
}

fn redirect_stderr(stderr: &Path) -> nix::Result<()> {
    let file_fd = fcntl::open(
        stderr,
        OFlag::O_WRONLY | OFlag::O_CREAT | OFlag::O_TRUNC | OFlag::O_CLOEXEC,
        Mode::from_bits_truncate(0o644),
    )?;
    unistd::dup2(file_fd, libc::STDERR_FILENO)?;
    unistd::close(file_fd)?;
    Ok(())
}

fn execvp(bin: &str, args: &[String]) -> Result<Never> {
    let bin = CString::new(bin)?;

    let mut c_args = Vec::new();
    let mut argv: Vec<*const libc::c_char> = Vec::with_capacity(args.len() + 2);

    argv.push(bin.as_ptr());
    for a in args {
        let c = CString::new(a.as_str())?;
        argv.push(c.as_ptr());
        c_args.push(c);
    }
    argv.push(ptr::null());

    unsafe { libc::execvp(bin.as_ptr(), argv.as_ptr()) };

    Err(io::Error::last_os_error()).context("failed to execvp")
}
