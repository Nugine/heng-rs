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
        let java = &config.executor.java;

        let mut cmd = carapace::Command::new(&java.javac);

        cmd.arg("-J-Xms64m");
        cmd.arg("-J-Xmx512m");
        cmd.arg("-encoding").arg("UTF-8");
        cmd.arg("-sourcepath").arg(".");
        cmd.arg(self.src_name());

        // javac's compile error message is writed to stdout
        cmd.stdio("/dev/null", self.msg_name(), "/dev/null");

        cmd.bindmount_ro(&java.javac, &java.javac);
        for mnt in &java.mount {
            cmd.bindmount_ro(mnt, mnt);
        }

        sandbox_run(cmd, &config, workspace, hard_limit)
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
        let java = &config.executor.java;

        let mut cmd = carapace::Command::new(&java.java);
        cmd.arg("-cp").arg(".");
        cmd.arg("-Xms64m");
        cmd.arg("-Xmx512m");
        cmd.arg("Main");
        cmd.stdio(stdin, stdout, stderr);

        cmd.bindmount_ro(&java.javac, &java.javac);
        for mnt in &java.mount {
            cmd.bindmount_ro(mnt, mnt);
        }

        sandbox_run(cmd, &config, workspace, hard_limit)
    }
}
