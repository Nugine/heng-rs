use super::*;

pub struct JavaScript {}

impl Language for JavaScript {
    fn lang_name(&self) -> &str {
        "javascript"
    }

    fn needs_compile(&self) -> bool {
        false
    }

    fn src_name(&self) -> &str {
        "src.js"
    }

    fn msg_name(&self) -> &str {
        unimplemented!()
    }

    fn compile(&self, _: PathBuf, _: &Limit) -> Result<SandboxOutput> {
        unimplemented!()
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
        let src_path = workspace.join(self.src_name());
        let mut cmd = OsCmd::new(&config.executor.runtimes.node);
        cmd.arg(src_path);

        sandbox_exec(workspace, cmd, stdin, stdout, stderr, hard_limit)
    }
}
