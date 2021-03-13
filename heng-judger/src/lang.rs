pub mod c_cpp;
pub mod java;
pub mod javascript;
pub mod python;
pub mod rust;

use crate::Config;

use heng_utils::container::inject;
use heng_utils::math::roundup_div;
use heng_utils::os_cmd::OsCmd;

use carapace::{SandboxConfig, SandboxOutput};

use std::path::PathBuf;

use anyhow::Result;
use nix::unistd::{self, Gid, Uid};
use tracing::debug;

pub trait Language {
    fn lang_name(&self) -> &str;

    fn needs_compile(&self) -> bool;
    fn src_name(&self) -> &str;
    fn msg_name(&self) -> &str;

    fn compile(&self, workspace: PathBuf, hard_limit: &Limit) -> Result<SandboxOutput>;

    fn run(
        &self,
        workspace: PathBuf,
        stdin: PathBuf,
        stdout: PathBuf,
        stderr: PathBuf,
        hard_limit: &Limit,
    ) -> Result<SandboxOutput>;
}

pub struct Limit {
    pub cpu_time: u64, // milliseconds
    pub memory: u64,   // bytes
    pub output: u64,   // bytes
    pub pids: u32,     // number
}

#[allow(clippy::too_many_arguments)]
fn sandbox_exec(
    workspace: PathBuf,
    cmd: OsCmd,
    stdin: PathBuf,
    stdout: PathBuf,
    stderr: PathBuf,
    hard_limit: &Limit,
) -> Result<SandboxOutput> {
    let config = inject::<Config>();
    let cfg_hard_limit = &config.executor.hard_limit;

    let cpu_time = cfg_hard_limit.cpu_time.min(hard_limit.cpu_time);
    let memory = cfg_hard_limit.memory.as_u64().min(hard_limit.memory);
    let output = cfg_hard_limit.output.as_u64().min(hard_limit.output);
    let pids = cfg_hard_limit.pids.min(hard_limit.pids);

    let uid = Uid::from_raw(config.executor.uid);
    let gid = Gid::from_raw(config.executor.gid);

    let bind_rw = ["/dev/null", "/dev/zero", "/dev/random", "/dev/urandom"];

    let bind_ro = [
        "/bin", "/sbin", "/etc", "/usr", "/lib", "/lib64", "/var", "/run",
    ];

    let to_bind_mnts = |s: &[&str]| -> Vec<carapace::BindMount> {
        s.iter()
            .map(|s| carapace::BindMount::new_same(s.into()))
            .collect()
    };

    let sandbox_config = SandboxConfig {
        bin: cmd.bin,
        args: cmd.args,
        env: cmd.env,
        chroot: Some(workspace.clone()),
        uid: Some(uid.as_raw()),
        gid: Some(gid.as_raw()),
        stdin: Some(stdin),
        stdout: Some(stdout),
        stderr: Some(stderr),
        stdin_fd: None,
        stdout_fd: None,
        stderr_fd: None,
        real_time_limit: Some(cpu_time),
        rlimit_cpu: Some(roundup_div(cpu_time, 1000) as u32),
        rlimit_as: None,
        rlimit_data: None,
        rlimit_fsize: Some(output),
        cg_limit_memory: Some(memory),
        cg_limit_max_pids: Some(pids),
        bindmount_rw: to_bind_mnts(&bind_rw),
        bindmount_ro: to_bind_mnts(&bind_ro),
        mount_proc: Some("/proc".into()),
        mount_tmpfs: Some("/tmp".into()),
        priority: Some(-20),
    };

    unistd::chown(&workspace, Some(uid), Some(gid))?;

    debug!("run embedded carapace:\n{:?}", sandbox_config.to_cmd());
    let result = carapace::run(&sandbox_config);

    unistd::chown(&workspace, Some(unistd::getuid()), Some(unistd::getgid()))?;

    result
}
