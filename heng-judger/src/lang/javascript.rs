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
        let js = &config.executor.javascript;

        let mut cmd = carapace::Command::new(&js.node);
        cmd.arg(self.src_name());
        cmd.stdio(stdin, stdout, stderr);

        cmd.bindmount_ro(&js.node, &js.node);
        for mnt in &js.mount {
            cmd.bindmount_ro(mnt, mnt);
        }

        sandbox_run(cmd, &config, workspace, hard_limit)
    }
}
