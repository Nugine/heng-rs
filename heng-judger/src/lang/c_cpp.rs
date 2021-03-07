use super::*;
use crate::config::Config;

use heng_utils::container::inject;
use heng_utils::os_cmd::OsCmd;

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
        let is_cpp = self.std.is_cpp();

        let src_path = workspace.join(self.src_name());
        let exe_path = workspace.join(self.exe_name());
        let msg_path = workspace.join(self.msg_name());

        let config = inject::<Config>();

        let mut cmd = OsCmd::new(if is_cpp {
            config.executor.compilers.gxx.as_os_str()
        } else {
            config.executor.compilers.gcc.as_os_str()
        });

        cmd.arg("--std").arg(self.std.as_str_gnu());
        cmd.arg("-static");
        cmd.arg_if(self.o2, "-O2");

        // https://stackoverflow.com/questions/5419366/why-do-i-have-to-explicitly-link-with-libm
        cmd.arg_if(!is_cpp, "-lm");

        cmd.arg("-o").arg(exe_path);
        cmd.arg(src_path);

        cmd.inherit_env("PATH");

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
        let cmd = OsCmd::new(workspace.join(self.exe_name()));

        sandbox_exec(workspace, cmd, stdin, stdout, stderr, hard_limit)
    }
}
