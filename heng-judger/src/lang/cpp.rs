use super::*;
use crate::config::Config;

use heng_utils::container::inject;

use std::fs;

pub struct Cpp {
    pub std: CppStd,
    pub o2: bool,
}

pub enum CppStd {
    C89,
    C99,
    C11,
    Cpp11,
    Cpp14,
    Cpp17,
}

impl CppStd {
    fn from_str(s: &str) -> Result<Self> {
        match s {
            "c89" => Ok(CppStd::C89),
            "c99" => Ok(CppStd::C99),
            "c11" => Ok(CppStd::C11),
            "cpp11" => Ok(CppStd::Cpp11),
            "cpp14" => Ok(CppStd::Cpp14),
            "cpp17" => Ok(CppStd::Cpp17),
            _ => Err(anyhow::format_err!("invalid c/cpp std")),
        }
    }

    fn as_gnu_str(&self) -> &str {
        match self {
            CppStd::C89 => "gnu89",
            CppStd::C99 => "gnu99",
            CppStd::C11 => "gnu11",
            CppStd::Cpp11 => "gnu++11",
            CppStd::Cpp14 => "gnu++14",
            CppStd::Cpp17 => "gnu++17",
        }
    }

    fn is_cpp(&self) -> bool {
        matches!(self, CppStd::Cpp11 | CppStd::Cpp14 | CppStd::Cpp17)
    }
}

impl Language for Cpp {
    fn needs_compile(&self) -> bool {
        true
    }

    fn compile(&self, src_path: &Path, hard_limit: Limit) -> Result<CompileOutput> {
        let is_cpp = self.std.is_cpp();
        let dir = src_path.parent().unwrap();

        let real_src_path = if is_cpp {
            dir.join("src.cpp")
        } else {
            dir.join("src.c")
        };

        fs::rename(src_path, &real_src_path)?;

        let exe_path = dir.join("src");

        let config = inject::<Config>();

        let bin = if is_cpp {
            &config.executor.compilers.cpp
        } else {
            &config.executor.compilers.c
        };

        let mut args: Vec<String> = Vec::new();
        {
            args.push("--std".to_owned());
            args.push(self.std.as_gnu_str().to_owned());
        }
        if self.o2 {
            args.push("-O2".to_owned());
        }
        {
            args.push("-static".to_owned());
        }
        if !is_cpp {
            // https://stackoverflow.com/questions/5419366/why-do-i-have-to-explicitly-link-with-libm
            args.push("-lm".to_owned());
        }
        {
            args.push("-o".to_owned());
            args.push(exe_path.to_string_lossy().into());
        }
        {
            args.push(real_src_path.to_string_lossy().into());
        }

        let ce_path = dir.join("ce_msg");

        let sandbox_output = sandbox_exec(
            dir.to_owned(),
            bin.to_string_lossy().into(),
            args,
            "/dev/null".into(),
            "/dev/null".into(),
            ce_path.clone(),
            hard_limit,
        )?;

        Ok(CompileOutput {
            sandbox_output,
            exe_path,
            ce_path,
        })
    }

    fn run(
        &self,
        exe_path: &Path,
        stdin: PathBuf,
        stdout: PathBuf,
        stderr: PathBuf,
        hard_limit: Limit,
    ) -> Result<SandboxOutput> {
        todo!()
    }
}