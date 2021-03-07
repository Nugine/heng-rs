use heng_utils::os_cmd::OsCmd;

use super::*;

pub struct Rust {
    pub o2: bool,
}

impl Language for Rust {
    fn needs_compile(&self) -> bool {
        true
    }

    fn src_name(&self) -> &str {
        "src.rs"
    }

    fn exe_name(&self) -> &str {
        "src"
    }

    fn msg_name(&self) -> &str {
        "msg"
    }

    fn compile(&self, workspace: PathBuf, hard_limit: &Limit) -> Result<SandboxOutput> {
        let config = inject::<Config>();

        let src_path = workspace.join(self.src_name());
        let exe_path = workspace.join(self.exe_name());
        let msg_path = workspace.join(self.msg_name());

        let mut cmd = OsCmd::new(&config.executor.compilers.rust);
        cmd.arg_if(self.o2, "-O");
        cmd.arg("-o").arg(exe_path);
        cmd.arg(src_path);

        cmd.inherit_env("PATH");
        cmd.add_env("TMPDIR", &workspace);

        sandbox_exec(
            workspace,
            cmd.bin,
            cmd.args,
            cmd.env,
            "/dev/null".into(),
            "/dev/null".into(),
            msg_path,
            hard_limit,
        )
    }

    fn run(
        &self,
        workspace: PathBuf,
        stdin: PathBuf,
        stdout: PathBuf,
        stderr: PathBuf,
        hard_limit: &Limit,
    ) -> Result<SandboxOutput> {
        let cmd = OsCmd::new(workspace.join(self.exe_name()));

        sandbox_exec(
            workspace, cmd.bin, cmd.args, cmd.env, stdin, stdout, stderr, hard_limit,
        )
    }
}
