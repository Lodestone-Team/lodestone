use std::path::{Path, PathBuf};

use async_trait::async_trait;
use color_eyre::eyre::{eyre, Context};

use crate::{
    error::Error,
    events::CausedBy,
    macro_executor::{DefaultWorkerOptionGenerator, MacroPID, SpawnResult},
    traits::t_macro::{HistoryEntry, MacroEntry, TMacro, TaskEntry},
};

use super::MinecraftInstance;

pub fn resolve_macro_invocation(path_to_macro: &Path, macro_name: &str) -> Option<PathBuf> {
    let ts_macro = path_to_macro.join(macro_name).with_extension("ts");
    let js_macro = path_to_macro.join(macro_name).with_extension("js");

    let macro_folder = path_to_macro.join(macro_name);

    if ts_macro.is_file() {
        return Some(ts_macro);
    } else if js_macro.is_file() {
        return Some(js_macro);
    } else if macro_folder.is_dir() {
        // check if index.ts exists
        let index_ts = macro_folder.join("index.ts");
        let index_js = macro_folder.join("index.js");
        if index_ts.exists() {
            return Some(index_ts);
        } else if index_js.exists() {
            return Some(index_js);
        }
    }
    None
}

#[async_trait]
impl TMacro for MinecraftInstance {
    async fn get_macro_list(&self) -> Result<Vec<MacroEntry>, Error> {
        let mut ret = Vec::new();
        for entry in
            (std::fs::read_dir(&self.path_to_macros).context("Failed to read macro dir")?).flatten()
        {
            // if the entry is a file, check if it has the .ts or .js extension
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext == "ts" || ext == "js" {
                        ret.push(MacroEntry {
                            last_run: self.macro_name_to_last_run.lock().await.get(&name).cloned(),
                            name,
                            path,
                        })
                    }
                }
            } else if path.is_dir() {
                // check if index.ts or index.js exists
                let index_ts = path.join("index.ts");
                let index_js = path.join("index.js");
                if index_ts.exists() || index_js.exists() {
                    ret.push(MacroEntry {
                        last_run: self.macro_name_to_last_run.lock().await.get(&name).cloned(),
                        name,
                        path,
                    })
                }
            }
        }
        ret.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(ret)
    }

    async fn get_task_list(&self) -> Result<Vec<TaskEntry>, Error> {
        let mut ret = Vec::new();
        for (pid, task_entry) in self.pid_to_task_entry.lock().await.iter() {
            if self.macro_executor.get_macro_status(*pid).await.is_none() {
                ret.push(task_entry.clone());
            }
        }
        ret.sort_by(|a, b| a.creation_time.cmp(&b.creation_time));
        Ok(ret)
    }

    async fn get_history_list(&self) -> Result<Vec<HistoryEntry>, Error> {
        let mut ret = Vec::new();
        for (pid, task_entry) in self.pid_to_task_entry.lock().await.iter() {
            if let Some(exit_status) = self.macro_executor.get_macro_status(*pid).await {
                ret.push(HistoryEntry {
                    task: task_entry.clone(),
                    exit_status,
                });
            }
        }
        ret.sort_by(|a, b| b.exit_status.time().cmp(&a.exit_status.time()));
        Ok(ret)
    }

    async fn delete_macro(&mut self, name: &str) -> Result<(), Error> {
        crate::util::fs::remove_file(self.path_to_macros.join(name)).await?;
        Ok(())
    }

    async fn create_macro(&mut self, name: &str, content: &str) -> Result<(), Error> {
        crate::util::fs::write_all(self.path_to_macros.join(name), content.as_bytes().to_vec())
            .await
    }

    async fn run_macro(
        &mut self,
        name: &str,
        args: Vec<String>,
        caused_by: CausedBy,
    ) -> Result<TaskEntry, Error> {
        let path_to_macro = resolve_macro_invocation(&self.path_to_macros, name)
            .ok_or_else(|| eyre!("Failed to resolve macro invocation for {}", name))?;

        let SpawnResult { macro_pid: pid, .. } = self
            .macro_executor
            .spawn(
                path_to_macro,
                args,
                caused_by,
                Box::new(DefaultWorkerOptionGenerator),
                None,
                Some(self.uuid.clone()),
            )
            .await?;
        let entry = TaskEntry {
            pid,
            name: name.to_string(),
            creation_time: chrono::Utc::now().timestamp(),
        };
        self.pid_to_task_entry
            .lock()
            .await
            .insert(pid, entry.clone());
        self.macro_name_to_last_run
            .lock()
            .await
            .insert(name.to_string(), chrono::Utc::now().timestamp());

        Ok(entry)
    }

    async fn kill_macro(&mut self, pid: MacroPID) -> Result<(), Error> {
        self.macro_executor.abort_macro(pid)?;
        Ok(())
    }
}
