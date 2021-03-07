pub mod c_cpp;
pub mod java;
pub mod rust;

use crate::Config;

use heng_sandbox::nsjail::NsjailArgs;
use heng_sandbox::{SandboxArgs, SandboxOutput};
use heng_utils::container::inject;
use heng_utils::math::roundup_div;

use std::ffi::OsString;
use std::path::PathBuf;

use anyhow::Result;
use nix::unistd::{self, Gid, Uid};

pub trait Language {
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
    bin: PathBuf,
    args: Vec<OsString>,
    env: Vec<OsString>,
    stdin: PathBuf,
    stdout: PathBuf,
    stderr: PathBuf,
    hard_limit: &Limit,
) -> Result<SandboxOutput> {
    let config = inject::<Config>();
    let cfg_hard_limit = &config.executor.hard_limit;

    let nsjail_args = NsjailArgs {
        config: config.executor.nsjail_config.clone(),
        workspace: workspace.clone(),
        time_limit: Some(roundup_div(cfg_hard_limit.cpu_time, 1000) as u32),
    };

    let cpu_time = cfg_hard_limit.cpu_time.min(hard_limit.cpu_time);
    let memory = cfg_hard_limit.memory.as_u64().min(hard_limit.memory);
    let output = cfg_hard_limit.output.as_u64().min(hard_limit.output);
    let pids = cfg_hard_limit.pids.min(hard_limit.pids);

    let uid = Uid::from_raw(config.executor.uid);
    let gid = Gid::from_raw(config.executor.gid);

    let sandbox_args = SandboxArgs {
        bin,
        args,
        env,
        stdin: Some(stdin),
        stdout: Some(stdout),
        stderr: Some(stderr),
        uid: Some(uid.as_raw()),
        gid: Some(gid.as_raw()),
        real_time_limit: Some(cpu_time),
        rlimit_cpu: Some(roundup_div(cpu_time, 1000) as u32),
        rlimit_as: None,
        rlimit_data: None,
        rlimit_fsize: Some(output),
        cg_limit_memory: Some(memory),
        cg_limit_max_pids: Some(pids),
    };

    unistd::chown(&workspace, Some(uid), Some(gid))?;

    let result = heng_sandbox::nsjail::exec(&nsjail_args, &sandbox_args);

    unistd::chown(&workspace, Some(unistd::getuid()), Some(unistd::getgid()))?;

    result
}
