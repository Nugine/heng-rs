use heng_utils::os_cmd::OsCmd;

use super::*;

pub struct Java {}

impl Language for Java {
    fn lang_name(&self) -> &str {
        "java"
    }

    fn needs_compile(&self) -> bool {
        true
    }

    fn src_name(&self) -> &str {
        "Main.java"
    }

    fn msg_name(&self) -> &str {
        "msg"
    }

    fn compile(&self, workspace: PathBuf, hard_limit: &Limit) -> Result<SandboxOutput> {
        let config = inject::<Config>();

        let mut cmd = OsCmd::new(&config.executor.compilers.javac);

        cmd.arg("-J-Xms64m");
        cmd.arg("-J-Xmx512m");
        cmd.arg("-encoding").arg("UTF-8");
        cmd.arg("-sourcepath").arg(".");
        cmd.arg(self.src_name());

        sandbox_exec(
            workspace,
            cmd,
            "/dev/null".into(),
            self.msg_name().into(), // javac's compile error message is writed to stdout
            "/dev/null".into(),
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
        let config = inject::<Config>();

        let mut cmd = OsCmd::new(&config.executor.runtimes.java);
        cmd.arg("-cp").arg(".");
        cmd.arg("-Xms64m");
        cmd.arg("-Xmx512m");
        cmd.arg("Main");

        sandbox_exec(workspace, cmd, stdin, stdout, stderr, hard_limit)
    }
}
