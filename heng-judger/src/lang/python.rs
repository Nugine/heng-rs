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
        let python = &config.executor.python;

        let mut cmd = carapace::Command::new(&python.python);
        cmd.arg(self.src_name());
        cmd.stdio(stdin, stdout, stderr);

        cmd.bindmount_ro(&python.python, &python.python);
        for mnt in &python.mount {
            cmd.bindmount_ro(mnt, mnt);
        }

        sandbox_run(cmd, &config, workspace, hard_limit)
    }
}
