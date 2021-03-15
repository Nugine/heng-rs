pub mod c_cpp;
pub mod java;
pub mod javascript;
pub mod python;
pub mod rust;

use crate::Config;

use heng_utils::container::inject;
use heng_utils::math::roundup_div;

use carapace::SandboxOutput;
use tracing::debug;

use std::path::PathBuf;

use anyhow::Result;

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
    pub real_time: u64, // milliseconds
    pub cpu_time: u64,  // milliseconds
    pub memory: u64,    // bytes
    pub output: u64,    // bytes
    pub pids: u32,      // number
}

/// set chroot, uid, gid
///
/// set real_time_limit, rlimits, cg_limits
///
/// set priority
///
/// set mount_proc, mount_tmpfs
///
/// set seccomp_forbid_ipc
///
/// add env PATH
///
/// add some bindmount_rw and bindmount_ro;
fn sandbox_run(
    mut cmd: carapace::Command,
    config: &Config,
    workspace: PathBuf,
    hard_limit: &Limit,
) -> Result<SandboxOutput> {
    let cfg_hard_limit = &config.executor.hard_limit;

    let real_time = cfg_hard_limit.real_time.min(hard_limit.real_time);
    let cpu_time = cfg_hard_limit.cpu_time.min(hard_limit.cpu_time);
    let memory = cfg_hard_limit.memory.as_u64().min(hard_limit.memory);
    let output = cfg_hard_limit.output.as_u64().min(hard_limit.output);
    let pids = cfg_hard_limit.pids.min(hard_limit.pids);

    let uid = config.executor.uid;
    let gid = config.executor.gid;

    let bind_rw = ["/dev/null", "/dev/zero", "/dev/random", "/dev/urandom"];
    let bind_ro = ["/lib", "/lib64"];
    let set_bindmount = |s: &[&str], b: Vec<carapace::BindMount>| {
        let iter1 = s
            .iter()
            .map(|src| carapace::BindMount::new_same(src.into()));
        let iter2 = b.into_iter();
        iter1.chain(iter2).collect::<Vec<_>>()
    };

    let c = &mut cmd.config;
    c.chroot = Some(workspace);
    c.uid = Some(uid);
    c.gid = Some(gid);
    c.real_time_limit = Some(real_time);
    c.rlimit_cpu = Some(roundup_div(cpu_time, 1000) as u32);
    c.rlimit_fsize = Some(output);
    c.cg_limit_memory = Some(memory);
    c.cg_limit_max_pids = Some(pids);
    c.priority = Some(-20);
    c.mount_proc = Some("/proc".into());
    c.mount_tmpfs = Some("/tmp".into());
    c.seccomp_forbid_ipc = true;

    c.env.insert(
        0,
        "PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:.".into(),
    );

    cmd.config.bindmount_rw = set_bindmount(&bind_rw, cmd.config.bindmount_rw);
    cmd.config.bindmount_ro = set_bindmount(&bind_ro, cmd.config.bindmount_ro);

    debug!("run embedded carapace:\n{:?}", cmd.config.to_cli_cmd());
    cmd.run()
}
