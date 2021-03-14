use super::*;

pub struct CCpp {
    pub std: CCppStd,
    pub o2: bool,
}

pub enum CCppStd {
    C89,
    C99,
    C11,
    Cpp11,
    Cpp14,
    Cpp17,
}

impl CCppStd {
    fn from_str(s: &str) -> Result<Self> {
        match s {
            "c89" => Ok(CCppStd::C89),
            "c99" => Ok(CCppStd::C99),
            "c11" => Ok(CCppStd::C11),
            "cpp11" => Ok(CCppStd::Cpp11),
            "cpp14" => Ok(CCppStd::Cpp14),
            "cpp17" => Ok(CCppStd::Cpp17),
            _ => Err(anyhow::format_err!("invalid c/cpp std")),
        }
    }

    fn as_str_gnu(&self) -> &str {
        match self {
            CCppStd::C89 => "gnu89",
            CCppStd::C99 => "gnu99",
            CCppStd::C11 => "gnu11",
            CCppStd::Cpp11 => "gnu++11",
            CCppStd::Cpp14 => "gnu++14",
            CCppStd::Cpp17 => "gnu++17",
        }
    }

    fn is_cpp(&self) -> bool {
        matches!(self, CCppStd::Cpp11 | CCppStd::Cpp14 | CCppStd::Cpp17)
    }
}

impl CCpp {
    fn exe_name(&self) -> &str {
        "src"
    }
}

impl Language for CCpp {
    fn lang_name(&self) -> &str {
        if self.std.is_cpp() {
            "cpp"
        } else {
            "c"
        }
    }

    fn needs_compile(&self) -> bool {
        true
    }

    fn src_name(&self) -> &str {
        if self.std.is_cpp() {
            "src.cpp"
        } else {
            "src.c"
        }
    }

    fn msg_name(&self) -> &str {
        "msg"
    }

    fn compile(&self, workspace: PathBuf, hard_limit: &Limit) -> Result<SandboxOutput> {
        let config = inject::<Config>();
        let c_cpp = &config.executor.c_cpp;

        let is_cpp = self.std.is_cpp();

        let cc = if is_cpp {
            c_cpp.gxx.as_os_str()
        } else {
            c_cpp.gcc.as_os_str()
        };

        let mut cmd = carapace::Command::new(cc);

        cmd.arg("--std").arg(self.std.as_str_gnu());
        cmd.arg("-static");
        cmd.arg_if(self.o2, "-O2");

        // https://stackoverflow.com/questions/5419366/why-do-i-have-to-explicitly-link-with-libm
        cmd.arg_if(!is_cpp, "-lm");

        cmd.arg("-o").arg(self.exe_name());
        cmd.arg(self.src_name());

        cmd.stdio("/dev/null", "/dev/null", self.msg_name());

        cmd.bindmount_ro(cc, cc);
        for mnt in &c_cpp.mount {
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
