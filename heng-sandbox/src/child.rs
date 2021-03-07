use crate::cgroup::Cgroup;
use crate::libc_call;
use crate::SandboxArgs;

use std::convert::Infallible as Never;
use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::prelude::OsStringExt;
use std::path::Path;
use std::ptr;
use std::{env, io};

use anyhow::{Context, Result};
use fcntl::OFlag;
use nix::fcntl;
use nix::sys::stat::Mode;
use nix::unistd::{self, Gid, Uid};
use rlimit::{Resource, Rlim};

pub fn run_child(args: &SandboxArgs, cgroup: &Cgroup) -> Result<Never> {
    let child_pid = unistd::getpid();

    libc_call(|| unsafe { libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) })?;

    if let Some(ref stdin) = args.stdin {
        redirect_stdin(stdin).context("failed to redirect stdin")?;
    }

    if let Some(ref stdout) = args.stdout {
        redirect_stdout(stdout).context("failed to redirect stdout")?;
    }

    if let Some(ref stderr) = args.stderr {
        redirect_stderr(stderr).context("failed to redirect stderr")?;
    }

    if let Some(rlimit_cpu) = args.rlimit_cpu.map(|r| Rlim::from_raw(r as _)) {
        Resource::CPU.set(rlimit_cpu, rlimit_cpu)?;
    }

    if let Some(rlimit_as) = args.rlimit_as.map(|r| Rlim::from_raw(r as _)) {
        Resource::AS.set(rlimit_as, rlimit_as)?;
    }

    if let Some(rlimit_data) = args.rlimit_data.map(|r| Rlim::from_raw(r as _)) {
        Resource::DATA.set(rlimit_data, rlimit_data)?;
    }

    if let Some(rlimit_fsize) = args.rlimit_fsize.map(|r| Rlim::from_raw(r as _)) {
        Resource::FSIZE.set(rlimit_fsize, rlimit_fsize)?;
    }

    let execve_bin = CString::new(args.bin.as_os_str().as_bytes())?;

    let mut cstrings = Vec::new();
    let mut execve_argv: Vec<*const libc::c_char> = Vec::with_capacity(args.args.len() + 2);
    let mut execve_env: Vec<*const libc::c_char> = Vec::with_capacity(args.env.len() + 1);

    execve_argv.push(execve_bin.as_ptr());
    for a in &args.args {
        let c = CString::new(a.as_bytes())?;
        execve_argv.push(c.as_ptr());
        cstrings.push(c);
    }
    execve_argv.push(ptr::null());

    for e in &args.env {
        let c = if e.as_bytes().contains(&b'=') {
            CString::new(e.as_bytes())?
        } else if let Some(value) = env::var_os(e) {
            let mut v = Vec::new();
            v.extend_from_slice(e.as_bytes());
            v.push(b'=');
            v.extend(value.into_vec());
            CString::new(v)?
        } else {
            continue;
        };
        execve_env.push(c.as_ptr());
        cstrings.push(c);
    }
    execve_env.push(ptr::null());

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

    unsafe {
        libc::execve(
            execve_bin.as_ptr(),
            execve_argv.as_ptr(),
            execve_env.as_ptr(),
        )
    };

    Err(io::Error::last_os_error())
        .with_context(|| format!("failed to execvp: bin = {:?}", args.bin))
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
