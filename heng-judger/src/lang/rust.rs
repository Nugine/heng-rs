use heng_utils::os_cmd::OsCmd;

use super::*;

pub struct Rust {
    pub o2: bool,
}

impl Rust {
    fn exe_name(&self) -> &str {
        "src"
    }
}

impl Language for Rust {
    fn lang_name(&self) -> &str {
        "rust"
    }

    fn needs_compile(&self) -> bool {
        true
    }

    fn src_name(&self) -> &str {
        "src.rs"
    }

    fn msg_name(&self) -> &str {
        "msg"
    }

    fn compile(&self, workspace: PathBuf, hard_limit: &Limit) -> Result<SandboxOutput> {
        let config = inject::<Config>();

        let src_path = workspace.join(self.src_name());
        let exe_path = workspace.join(self.exe_name());
        let msg_path = workspace.join(self.msg_name());

        let mut cmd = OsCmd::new(&config.executor.compilers.rustc);
        cmd.arg_if(self.o2, "-O");
        cmd.arg("-o").arg(exe_path);
        cmd.arg(src_path);

        cmd.inherit_env("PATH");
        cmd.add_env("TMPDIR", &workspace);

        sandbox_exec(
            workspace,
            cmd,
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
        let exe_path = workspace.join(self.exe_name());
        let cmd = OsCmd::new(exe_path);

        sandbox_exec(workspace, cmd, stdin, stdout, stderr, hard_limit)
    }
}
