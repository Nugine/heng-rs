use heng_utils::os_cmd::OsCmd;

use super::*;

pub struct Python {}

impl Language for Python {
    fn lang_name(&self) -> &str {
        "python"
    }

    fn needs_compile(&self) -> bool {
        false
    }

    fn src_name(&self) -> &str {
        "src.py"
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

        let mut cmd = OsCmd::new(&config.executor.runtimes.python);
        cmd.arg(self.src_name());

        sandbox_exec(workspace, cmd, stdin, stdout, stderr, hard_limit)
    }
}