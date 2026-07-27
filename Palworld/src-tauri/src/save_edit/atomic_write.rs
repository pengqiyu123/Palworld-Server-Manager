use std::io::Write;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone)]
pub struct FileMutation {
    pub relative_path: PathBuf,
    pub content: Option<Vec<u8>>,
}

pub fn commit_file_set(world_root: &Path, mutations: &[FileMutation]) -> Result<(), String> {
    commit_file_set_with_fault(world_root, mutations, None)
}

fn commit_file_set_with_fault(
    world_root: &Path,
    mutations: &[FileMutation],
    fail_after_commits: Option<usize>,
) -> Result<(), String> {
    if !world_root.is_dir() {
        return Err("世界目录不存在".to_string());
    }
    if mutations.is_empty() {
        return Err("写入文件集合不能为空".to_string());
    }

    let mut seen = std::collections::HashSet::new();
    for mutation in mutations {
        validate_relative_path(&mutation.relative_path)?;
        if !seen.insert(mutation.relative_path.clone()) {
            return Err(format!(
                "重复的写入路径: {}",
                mutation.relative_path.display()
            ));
        }
    }

    let transaction_id = new_transaction_id();
    let mut prepared = Vec::with_capacity(mutations.len());
    for mutation in mutations {
        let destination = world_root.join(&mutation.relative_path);
        let parent = destination
            .parent()
            .ok_or_else(|| "写入路径缺少父目录".to_string())?;
        if let Err(error) = std::fs::create_dir_all(parent) {
            cleanup_prepared(&mut prepared);
            return Err(format!("创建写入目录失败: {error}"));
        }
        let file_name = destination
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| "写入文件名无效".to_string())?;
        let temporary = parent.join(format!(".{file_name}.txn-{transaction_id}.tmp"));
        let previous = parent.join(format!(".{file_name}.txn-{transaction_id}.old"));
        if temporary.exists() || previous.exists() {
            cleanup_prepared(&mut prepared);
            return Err("事务暂存文件冲突".to_string());
        }
        if let Some(content) = &mutation.content {
            let prepared_file = (|| {
                let mut file = std::fs::OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .open(&temporary)
                    .map_err(|error| format!("创建事务暂存文件失败: {error}"))?;
                file.write_all(content)
                    .map_err(|error| format!("写入事务暂存文件失败: {error}"))?;
                file.sync_all()
                    .map_err(|error| format!("同步事务暂存文件失败: {error}"))?;
                Ok::<(), String>(())
            })();
            if let Err(error) = prepared_file {
                let _ = std::fs::remove_file(&temporary);
                cleanup_prepared(&mut prepared);
                return Err(error);
            }
        }
        prepared.push(PreparedMutation {
            destination,
            temporary,
            previous,
            has_content: mutation.content.is_some(),
            existed_before: world_root.join(&mutation.relative_path).is_file(),
            applied: false,
        });
    }

    for index in 0..prepared.len() {
        if fail_after_commits == Some(index) {
            let error = "故障注入：中断多文件提交".to_string();
            rollback_prepared(&mut prepared)?;
            return Err(error);
        }
        let item = &mut prepared[index];
        let apply_result = (|| {
            if item.destination.exists() {
                if !item.destination.is_file() {
                    return Err(format!(
                        "目标写入路径不是文件: {}",
                        item.destination.display()
                    ));
                }
                std::fs::rename(&item.destination, &item.previous)
                    .map_err(|error| format!("暂存原文件失败: {error}"))?;
            }
            if item.has_content {
                std::fs::rename(&item.temporary, &item.destination)
                    .map_err(|error| format!("提交新文件失败: {error}"))?;
            }
            item.applied = true;
            Ok::<(), String>(())
        })();
        if let Err(error) = apply_result {
            let rollback = rollback_prepared(&mut prepared);
            return match rollback {
                Ok(()) => Err(format!("{error}；已恢复全部原文件")),
                Err(rollback_error) => Err(format!("{error}；恢复失败: {rollback_error}")),
            };
        }
    }

    for item in &prepared {
        if item.previous.is_file() {
            std::fs::remove_file(&item.previous)
                .map_err(|error| format!("清理事务旧文件失败: {error}"))?;
        }
        if item.temporary.is_file() {
            std::fs::remove_file(&item.temporary)
                .map_err(|error| format!("清理事务暂存文件失败: {error}"))?;
        }
    }
    Ok(())
}

struct PreparedMutation {
    destination: PathBuf,
    temporary: PathBuf,
    previous: PathBuf,
    has_content: bool,
    existed_before: bool,
    applied: bool,
}

fn rollback_prepared(prepared: &mut [PreparedMutation]) -> Result<(), String> {
    let mut failures = Vec::new();
    for item in prepared.iter_mut().rev() {
        if item.previous.is_file() {
            if item.destination.is_file() {
                if let Err(error) = std::fs::remove_file(&item.destination) {
                    failures.push(format!("删除新文件失败: {error}"));
                    continue;
                }
            }
            if let Err(error) = std::fs::rename(&item.previous, &item.destination) {
                failures.push(format!("恢复原文件失败: {error}"));
            }
        } else if item.applied && !item.existed_before && item.destination.is_file() {
            if let Err(error) = std::fs::remove_file(&item.destination) {
                failures.push(format!("删除新增文件失败: {error}"));
            }
        }
        if item.temporary.is_file() {
            if let Err(error) = std::fs::remove_file(&item.temporary) {
                failures.push(format!("清理暂存文件失败: {error}"));
            }
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("; "))
    }
}

fn cleanup_prepared(prepared: &mut [PreparedMutation]) {
    for item in prepared {
        let _ = std::fs::remove_file(&item.temporary);
        if item.previous.is_file() && !item.destination.exists() {
            let _ = std::fs::rename(&item.previous, &item.destination);
        }
    }
}

fn validate_relative_path(path: &Path) -> Result<(), String> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("写入路径必须是世界目录内的安全相对文件路径".to_string());
    }
    Ok(())
}

fn new_transaction_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    format!(
        "{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fault_after_first_commit_restores_originals_and_removes_new_file() {
        let root = test_root("rollback");
        std::fs::create_dir_all(root.join("Players")).unwrap();
        std::fs::write(root.join("Level.sav"), b"level-before").unwrap();
        std::fs::write(root.join("Players/player.sav"), b"player-before").unwrap();
        let mutations = vec![
            FileMutation {
                relative_path: PathBuf::from("Level.sav"),
                content: Some(b"level-after".to_vec()),
            },
            FileMutation {
                relative_path: PathBuf::from("Players/player.sav"),
                content: Some(b"player-after".to_vec()),
            },
            FileMutation {
                relative_path: PathBuf::from("Players/new_dps.sav"),
                content: Some(b"new-dps".to_vec()),
            },
        ];

        let error = commit_file_set_with_fault(&root, &mutations, Some(1))
            .expect_err("故障注入应使提交失败");

        assert!(error.contains("故障注入"));
        assert_eq!(
            std::fs::read(root.join("Level.sav")).unwrap(),
            b"level-before"
        );
        assert_eq!(
            std::fs::read(root.join("Players/player.sav")).unwrap(),
            b"player-before"
        );
        assert!(!root.join("Players/new_dps.sav").exists());
        assert_no_transaction_files(&root);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn successful_commit_replaces_deletes_and_creates_as_one_set() {
        let root = test_root("success");
        std::fs::create_dir_all(root.join("Players")).unwrap();
        std::fs::write(root.join("Level.sav"), b"before").unwrap();
        std::fs::write(root.join("Players/remove.sav"), b"remove").unwrap();
        let mutations = vec![
            FileMutation {
                relative_path: PathBuf::from("Level.sav"),
                content: Some(b"after".to_vec()),
            },
            FileMutation {
                relative_path: PathBuf::from("Players/remove.sav"),
                content: None,
            },
            FileMutation {
                relative_path: PathBuf::from("Players/new.sav"),
                content: Some(b"new".to_vec()),
            },
        ];

        commit_file_set(&root, &mutations).expect("事务提交应成功");

        assert_eq!(std::fs::read(root.join("Level.sav")).unwrap(), b"after");
        assert!(!root.join("Players/remove.sav").exists());
        assert_eq!(std::fs::read(root.join("Players/new.sav")).unwrap(), b"new");
        assert_no_transaction_files(&root);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_parent_path_before_writing_any_file() {
        let root = test_root("traversal");
        std::fs::write(root.join("Level.sav"), b"before").unwrap();
        let mutation = FileMutation {
            relative_path: PathBuf::from("../outside.sav"),
            content: Some(b"bad".to_vec()),
        };

        assert!(commit_file_set(&root, &[mutation]).is_err());
        assert_eq!(std::fs::read(root.join("Level.sav")).unwrap(), b"before");
        assert!(!root.parent().unwrap().join("outside.sav").exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn preparation_failure_removes_all_already_created_transaction_files() {
        let root = test_root("prepare-cleanup");
        std::fs::write(root.join("blocked"), b"not-a-directory").unwrap();
        let mutations = vec![
            FileMutation {
                relative_path: PathBuf::from("Level.sav"),
                content: Some(b"prepared".to_vec()),
            },
            FileMutation {
                relative_path: PathBuf::from("blocked/player.sav"),
                content: Some(b"must-fail".to_vec()),
            },
        ];

        commit_file_set(&root, &mutations).expect_err("第二个父目录不可创建时准备阶段必须失败");

        assert!(!root.join("Level.sav").exists());
        assert_no_transaction_files(&root);
        let _ = std::fs::remove_dir_all(root);
    }

    fn test_root(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "palworld-atomic-write-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn assert_no_transaction_files(root: &Path) {
        let mut pending = vec![root.to_path_buf()];
        while let Some(directory) = pending.pop() {
            for entry in std::fs::read_dir(directory).unwrap() {
                let entry = entry.unwrap();
                if entry.path().is_dir() {
                    pending.push(entry.path());
                } else {
                    let name = entry.file_name().to_string_lossy().to_string();
                    assert!(!name.contains(".txn-"), "遗留事务文件: {name}");
                }
            }
        }
    }
}
