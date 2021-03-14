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
        let rust = &config.executor.rust;

        let mut cmd = carapace::Command::new(&rust.rustc);
        cmd.arg_if(self.o2, "-O");
        cmd.arg("-o").arg(self.exe_name());
        cmd.arg(self.src_name());
        cmd.stdio("/dev/null", "/dev/null", self.msg_name());

        cmd.bindmount_ro(&rust.rustc, &rust.rustc);
        for mnt in &rust.mount {
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
        let mut cmd = carapace::Command::new(self.exe_name());
        cmd.stdio(stdin, stdout, stderr);
        sandbox_run(cmd, &config, workspace, hard_limit)
    }
}
