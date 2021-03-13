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

        let mut cmd = OsCmd::new(&config.executor.compilers.rustc);
        cmd.arg_if(self.o2, "-O");
        cmd.arg("-o").arg(self.exe_name());
        cmd.arg(self.src_name());

        cmd.env
            .push("PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:.".into());

        sandbox_exec(
            workspace,
            cmd,
            "/dev/null".into(),
            "/dev/null".into(),
            self.msg_name().into(),
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
        let cmd = OsCmd::new(self.exe_name());

        sandbox_exec(workspace, cmd, stdin, stdout, stderr, hard_limit)
    }
}
