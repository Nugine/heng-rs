use crate::data::DataModule;

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use heng_protocol::common::{DynamicFile, File, Judge, JudgeResult, Test};
use heng_protocol::error::ErrorCode;
use heng_protocol::internal::ErrorInfo;
use heng_utils::auto_join::auto_join;

use anyhow::Result;

pub struct ExecutorModule {
    data_module: Arc<DataModule>,
    workspace_root: PathBuf,
}

impl ExecutorModule {
    pub fn new(data_module: Arc<DataModule>, workspace_root: PathBuf) -> Result<Self> {
        if !workspace_root.exists() {
            fs::create_dir_all(&workspace_root)?;
        }
        Ok(Self {
            data_module,
            workspace_root,
        })
    }

    fn validate_dyn_file_name(s: &str) -> bool {
        if s.len() > 64 {
            return false;
        }
        if s.starts_with("__") {
            return false;
        }
        for &b in s.as_bytes() {
            if b.is_ascii_alphabetic() {
                continue;
            }
            if b.is_ascii_digit() {
                continue;
            }
            if matches!(b, b'.' | b'_' | b'-') {
                continue;
            }
            return false;
        }
        true
    }

    fn create_workspace(&self, name: &str) -> Result<PathBuf> {
        let workspace_path = self.workspace_root.join(name);
        if workspace_path.exists() {
            fs::remove_dir_all(&workspace_path)?;
        }
        fs::create_dir(&workspace_path)?;
        Ok(workspace_path)
    }

    async fn load_dyn_files(&self, workspace_path: &Path, dyn_files: &[DynamicFile]) -> Result<()> {
        auto_join(|j| {
            let mut name_set: HashSet<&str> = HashSet::new();
            for dyn_file in dyn_files {
                match dyn_file {
                    DynamicFile::BuiltIn { name } => {
                        match name.as_str() {
                            "__user_code" => {
                                if name_set.contains(&**name) {
                                    anyhow::bail!("duplicate dynamic file name")
                                }
                                name_set.insert(name.as_str());
                            }
                            _ => reject_error!(
                                ErrorCode::NotSupported,
                                Some("unsupported dynamic file name".to_owned())
                            ),
                        };
                    }
                    DynamicFile::Remote { name, file } => {
                        if !Self::validate_dyn_file_name(name) {
                            reject_error!(
                                ErrorCode::InvalidRequest,
                                Some("invalid dynamic file name".to_owned())
                            )
                        }

                        if name_set.contains(&**name) {
                            anyhow::bail!("duplicate dynamic file name")
                        }
                        name_set.insert(name.as_str());

                        let file_path = workspace_path.join(name);
                        j.spawn(
                            async move { self.data_module.download_file(file, &file_path).await },
                        );
                    }
                };
            }

            Ok(())
        })
        .await
    }

    pub async fn load_sources(&self, workspace_path: &Path, judge: &Judge) -> Result<()> {
        let (user, spj, interactor) = match judge {
            Judge::Normal { ref user } => (user, None, None),
            Judge::Special { ref user, ref spj } => (user, Some(spj), None),
            Judge::Interactive {
                ref user,
                interactor,
            } => (user, None, Some(interactor)),
        };

        auto_join(|j| {
            let d = &self.data_module;
            {
                let user_path = workspace_path.join("__user_code");
                j.spawn(async move { d.download_file(&user.source, &user_path).await });
            }
            if let Some(spj) = spj {
                let spj_path = workspace_path.join("__spj_code");
                j.spawn(async move { d.download_file(&spj.source, &spj_path).await })
            }
            if let Some(interactor) = interactor {
                let interactor_path = workspace_path.join("__interactor_code");
                j.spawn(async move { d.download_file(&interactor.source, &interactor_path).await })
            }
            Ok(())
        })
        .await?;

        Ok(())
    }

    pub async fn exec_judge(
        &self,
        id: Arc<str>,
        data: Option<File>,
        dynamic_files: Option<Vec<DynamicFile>>,
        judge: Judge,
        test: Test,
    ) -> Result<JudgeResult> {
        // create workspace
        let workspace_path = self.create_workspace(&*id)?;

        // load data
        let data_dir_path = match data {
            Some(file) => Some(self.data_module.load_data(&file).await?),
            None => None,
        };

        // load dynamic files
        if let Some(ref dyn_files) = dynamic_files {
            self.load_dyn_files(&workspace_path, dyn_files).await?;
        }

        // load sources
        self.load_sources(&workspace_path, &judge).await?;

        let run_path = workspace_path.join("__run");
        fs::create_dir(&run_path)?;

        todo!()
    }
}
