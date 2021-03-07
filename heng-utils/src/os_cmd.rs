use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;

pub struct OsCmd {
    pub bin: PathBuf,
    pub args: Vec<OsString>,
    pub env: Vec<OsString>,
}

impl OsCmd {
    pub fn new(bin: impl Into<PathBuf>) -> Self {
        Self {
            bin: bin.into(),
            args: Vec::new(),
            env: Vec::new(),
        }
    }

    pub fn arg(&mut self, a: impl Into<OsString>) -> &mut Self {
        self.args.push(a.into());
        self
    }

    pub fn arg_if(&mut self, cond: bool, a: impl Into<OsString>) -> &mut Self {
        if cond {
            self.arg(a)
        } else {
            self
        }
    }

    pub fn inherit_env(&mut self, k: impl Into<OsString>) -> &mut Self {
        self.env.push(k.into()); // TODO: check b'=' and b'\0' ?
        self
    }

    pub fn add_env(&mut self, k: impl Into<OsString>, v: impl AsRef<OsStr>) -> &mut Self {
        let mut e: OsString = k.into();
        e.push(OsStr::from_bytes(b"="));
        e.push(v.as_ref());
        self.env.push(e); // TODO: check b'=' and b'\0' ?
        self
    }
}
