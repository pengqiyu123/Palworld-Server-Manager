//! 应用数据路径统一策略：通过 EXE 同级 `portable.flag` 启用便携模式。
//!
//! - 便携模式（存在 `portable.flag`）：settings/logs/config-backups/world-backups
//!   全部落到 EXE 同级目录；根目录不可写时直接报错，绝不静默回退 LocalAppData。
//! - 安装模式（无 `portable.flag`）：逐字保留 HEAD 既有路径与行为，不迁移现有用户数据。
//!
//! `main.rs` 在 `run()` 开头调用 [`init`] 做一次性 fail-fast 校验；各路径 helper 通过
//! [`current`] 读取解析结果。`current` 在未初始化时（如单元测试）会现场回退到
//! [`AppPaths::detect`]，因此不依赖 `main` 是否运行。

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AppStorageMode {
    Installed,
    Portable,
}

/// 写探针：在指定目录写一个临时文件后删除，验证可写性。可注入以便测试模拟失败。
pub type WriteProbe = fn(&Path) -> Result<(), String>;

#[derive(Clone, Debug)]
#[allow(dead_code)] // mode/app_root 等字段当前仅 ensure_writable 与测试直接读取
pub struct AppPaths {
    mode: AppStorageMode,
    app_root: PathBuf,
    data_dir: PathBuf,
    config_backups_dir: PathBuf,
    world_backups_dir: PathBuf,
    logs_dir: PathBuf, // app_log::log_path 消费
}

#[allow(dead_code)] // mode/app_root/logs_dir/workflow_dir 当前仅测试或预留使用
impl AppPaths {
    /// 生产入口：用 `current_exe()` 与真实写探针解析。
    pub fn detect() -> Result<Self, String> {
        let exe = std::env::current_exe().map_err(|error| format!("无法定位程序目录: {error}"))?;
        Self::detect_with_probe(&exe, real_write_probe)
    }

    /// 可注入入口：测试用。`exe` 为模拟的 EXE 路径，`probe` 为可写性探针。
    pub fn detect_with_probe(exe: &Path, probe: WriteProbe) -> Result<Self, String> {
        let parent = exe
            .parent()
            .ok_or_else(|| "无法定位程序所在目录".to_string())?;

        if parent.join("portable.flag").exists() {
            // 便携模式：先确保根目录可写，再解析所有路径；不可写直接失败，不回退 AppData。
            probe(parent).map_err(|_| {
                "便携文件夹无写入权限，请解压到可写目录后再运行（不要从压缩包内直接运行）"
                    .to_string()
            })?;
            Ok(Self {
                mode: AppStorageMode::Portable,
                app_root: parent.to_path_buf(),
                data_dir: parent.join("data"),
                config_backups_dir: parent.join("data").join("config-backups"),
                world_backups_dir: parent.join("backups"),
                logs_dir: parent.join("data").join("logs"),
            })
        } else {
            // 安装模式：逐字复刻 HEAD 既有路径，不新增校验、不迁移。
            let local = dirs::data_local_dir().ok_or_else(|| "无法获取本地数据目录".to_string())?;
            let roaming = dirs::data_dir().ok_or_else(|| "无法定位 AppData 目录".to_string())?;
            Ok(Self {
                mode: AppStorageMode::Installed,
                app_root: parent.to_path_buf(),
                data_dir: local.join("PalworldServerManager"),
                config_backups_dir: roaming.join("PalworldServerManager").join("config-backups"),
                world_backups_dir: installed_world_backups_dir(parent),
                logs_dir: local.join("PalworldServerManager").join("logs"),
            })
        }
    }

    pub fn mode(&self) -> AppStorageMode {
        self.mode
    }
    pub fn app_root(&self) -> &Path {
        &self.app_root
    }
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }
    pub fn config_backups_dir(&self) -> &Path {
        &self.config_backups_dir
    }
    pub fn world_backups_dir(&self) -> &Path {
        &self.world_backups_dir
    }
    pub fn logs_dir(&self) -> &Path {
        &self.logs_dir
    }
    pub fn workflow_dir(&self) -> PathBuf {
        self.world_backups_dir.join("_system").join("workflows")
    }

    /// 便携模式下确保根目录可写；安装模式直接放行（保留现状）。
    pub fn ensure_writable(&self) -> Result<(), String> {
        match self.mode {
            AppStorageMode::Installed => Ok(()),
            AppStorageMode::Portable => real_write_probe(&self.app_root).map_err(|_| {
                "便携文件夹无写入权限，请解压到可写目录后再运行（不要从压缩包内直接运行）"
                    .to_string()
            }),
        }
    }
}

/// HEAD 既有 world backup 默认目录：debug 用 `CARGO_MANIFEST_DIR/.. /backups`，
/// release 用 EXE 同级 `/backups`。逐字保留以守护既有回归测试
/// `debug_default_backup_root_is_repository_palworld_backups`。
fn installed_world_backups_dir(exe_parent: &Path) -> PathBuf {
    #[cfg(debug_assertions)]
    {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .map(|parent| parent.join("backups"))
            .unwrap_or_else(|| exe_parent.join("backups"))
    }
    #[cfg(not(debug_assertions))]
    {
        exe_parent.join("backups")
    }
}

fn real_write_probe(dir: &Path) -> Result<(), String> {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let probe = dir.join(format!(".palworld-write-probe-{nonce}"));
    std::fs::write(&probe, b"probe").map_err(|e| format!("写入探针失败: {e}"))?;
    std::fs::remove_file(&probe).map_err(|e| format!("删除探针失败: {e}"))?;
    Ok(())
}

static CURRENT: OnceLock<AppPaths> = OnceLock::new();

/// 在 `run()` 启动时调用一次：解析路径并对便携模式做 fail-fast 可写性校验。
/// 重复调用返回首次结果（`OnceLock` 仅首次写入成功）。
pub fn init() -> Result<(), String> {
    if CURRENT.get().is_some() {
        return Ok(());
    }
    let paths = AppPaths::detect()?;
    paths.ensure_writable()?;
    let _ = CURRENT.set(paths);
    Ok(())
}

/// 各路径 helper 读取已初始化的解析结果；未初始化时（如单元测试）现场回退到
/// [`AppPaths::detect`]，因此不依赖 `main` 是否运行过 `init`。
pub fn current() -> Result<AppPaths, String> {
    if let Some(existing) = CURRENT.get() {
        return Ok(existing.clone());
    }
    AppPaths::detect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::AppSettings;
    use std::path::PathBuf;

    fn make_stub_exe(dir: &Path) -> PathBuf {
        // 模拟 EXE：detect 只关心 exe.parent()，文件内容无关。
        let exe = dir.join("Palworld Server Manager.exe");
        std::fs::write(&exe, b"stub").unwrap();
        exe
    }

    fn failing_probe(_dir: &Path) -> Result<(), String> {
        Err("simulated unwritable".to_string())
    }

    #[test]
    fn portable_flag_present_resolves_all_under_exe() {
        let root = std::env::temp_dir().join(format!(
            "palworld_app_paths_portable_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let exe = make_stub_exe(&root);
        std::fs::write(root.join("portable.flag"), b"").unwrap();

        let paths =
            AppPaths::detect_with_probe(&exe, real_write_probe).expect("便携模式应解析成功");

        assert_eq!(paths.mode(), AppStorageMode::Portable);
        assert_eq!(paths.app_root(), root);
        assert_eq!(paths.data_dir(), root.join("data"));
        assert_eq!(paths.logs_dir(), root.join("data").join("logs"));
        assert_eq!(
            paths.config_backups_dir(),
            root.join("data").join("config-backups")
        );
        assert_eq!(paths.world_backups_dir(), root.join("backups"));
        assert_eq!(
            paths.workflow_dir(),
            root.join("backups").join("_system").join("workflows")
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn portable_flag_absent_keeps_installed_paths() {
        let root = std::env::temp_dir().join(format!(
            "palworld_app_paths_installed_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let exe = make_stub_exe(&root);
        // 无 portable.flag → 安装模式

        let paths =
            AppPaths::detect_with_probe(&exe, real_write_probe).expect("安装模式应解析成功");

        assert_eq!(paths.mode(), AppStorageMode::Installed);
        // 形态断言（不强制目录真存在）：data_dir 仍是 LocalAppData/PalworldServerManager
        assert!(
            paths
                .data_dir()
                .to_string_lossy()
                .ends_with("PalworldServerManager"),
            "data_dir 应保留 HEAD 形态: {}",
            paths.data_dir().display()
        );
        assert!(
            paths
                .config_backups_dir()
                .to_string_lossy()
                .ends_with("config-backups"),
            "config_backups_dir 应保留 HEAD 形态: {}",
            paths.config_backups_dir().display()
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn portable_root_unwritable_blocks_without_appdata_fallback() {
        let root = std::env::temp_dir().join(format!(
            "palworld_app_paths_unwritable_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let exe = make_stub_exe(&root);
        std::fs::write(root.join("portable.flag"), b"").unwrap();

        let err = AppPaths::detect_with_probe(&exe, failing_probe)
            .expect_err("不可写时应返回错误，不得回退 AppData");
        assert!(
            err.contains("无写入权限"),
            "错误应说明无写入权限，实际: {err}"
        );

        // 便携根目录不可写时，绝不应回退到 LocalAppData 下新建本应用目录。
        // 这里无法可靠地断言“全局 LocalAppData 干净”（其他测试可能写入），
        // 但 detect_with_probe 在便携分支返回 Err 前不会触碰任何 AppData 路径——
        // 该保证由源码结构决定：便携分支只调用 probe(parent)。
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn explicit_backup_root_override_still_wins() {
        // resolve_backup_root 在 backup_service 中：非空 backup_root 必须覆盖 AppPaths 默认。
        let settings = AppSettings {
            backup_root: "D:/custom-portable-backups".to_string(),
            ..AppSettings::default()
        };
        let resolved = crate::backup_service::resolve_backup_root(&settings)
            .expect("显式 backup_root 应直接返回");
        assert_eq!(
            resolved,
            PathBuf::from("D:/custom-portable-backups"),
            "显式 backup_root 必须胜出，不被 AppPaths 默认覆盖"
        );
    }

    #[test]
    fn installed_debug_default_matches_head_cargo_manifest_dir() {
        // 守护 HEAD 既有行为：debug 下默认 world backup 在 CARGO_MANIFEST_DIR/.. /backups。
        let root = std::env::temp_dir().join(format!(
            "palworld_app_paths_debug_default_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let exe = make_stub_exe(&root);

        let paths =
            AppPaths::detect_with_probe(&exe, real_write_probe).expect("安装模式应解析成功");

        #[cfg(debug_assertions)]
        {
            let expected = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .map(|p| p.join("backups"))
                .expect("CARGO_MANIFEST_DIR 应有父目录");
            assert_eq!(
                paths.world_backups_dir(),
                expected,
                "debug 安装模式默认备份目录应逐字等于 HEAD 的 CARGO_MANIFEST_DIR/.. /backups"
            );
        }
        #[cfg(not(debug_assertions))]
        {
            assert_eq!(paths.world_backups_dir(), root.join("backups"));
        }

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn ensure_writable_passes_in_portable_temp_dir() {
        // 端到端真实写探针：便携模式在可写临时目录应通过 ensure_writable。
        let root = std::env::temp_dir().join(format!(
            "palworld_app_paths_ensure_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let exe = make_stub_exe(&root);
        std::fs::write(root.join("portable.flag"), b"").unwrap();

        let paths =
            AppPaths::detect_with_probe(&exe, real_write_probe).expect("便携模式应解析成功");
        paths
            .ensure_writable()
            .expect("可写临时目录的 ensure_writable 应成功");
        // 探针文件应已被清理
        assert!(
            !std::fs::read_dir(&root).unwrap().any(|e| e
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".palworld-write-probe-")),
            "ensure_writable 后探针文件应被清理"
        );

        let _ = std::fs::remove_dir_all(&root);
    }
}
