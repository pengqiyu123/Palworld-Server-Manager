use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tauri::command;

// ==================== 配置备份 ====================

/// 备份信息（list_config_backups 返回结构）
#[derive(Serialize, Deserialize, Clone)]
pub struct BackupInfo {
    pub name: String,
    pub timestamp: String,
    pub size_bytes: u64,
}

/// 获取备份目录：安装模式为 %AppData%/PalworldServerManager/config-backups/（HEAD 既有），
/// 便携模式为 EXE 同级 /data/config-backups/。
fn backups_dir() -> Result<PathBuf, String> {
    let dir = crate::app_paths::current()?
        .config_backups_dir()
        .to_path_buf();
    if !dir.exists() {
        std::fs::create_dir_all(&dir).map_err(|e| format!("创建备份目录失败: {}", e))?;
    }
    Ok(dir)
}

/// 将时间戳格式化为 YYYYMMDD-HHmmss
fn format_timestamp(time: SystemTime) -> String {
    let secs = time
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // 简单实现：使用 chrono 不便引入，按本地时区 +8 小时计算（Asia/Shanghai）
    let secs = secs + 8 * 3600;
    let days = secs / 86400;
    let rem = secs % 86400;
    let h = rem / 3600;
    let m = (rem % 3600) / 60;
    let s = rem % 60;
    // 1970-01-01 起算
    let (y, mo, d) = days_to_ymd(days as i64);
    format!("{:04}{:02}{:02}-{:02}{:02}{:02}", y, mo, d, h, m, s)
}

/// 将 1970-01-01 起的天数转换为年月日（Gregorian）
fn days_to_ymd(days: i64) -> (i64, u32, u32) {
    let mut y = 1970i64;
    let mut d = days;
    loop {
        let leap = (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0);
        let yd = if leap { 366 } else { 365 };
        if d < yd {
            break;
        }
        d -= yd;
        y += 1;
    }
    let leap = (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0);
    let mdays = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut mo = 0u32;
    let mut rem = d as u32;
    while (mo as usize) < mdays.len() && rem >= mdays[mo as usize] {
        rem -= mdays[mo as usize];
        mo += 1;
    }
    (y, mo + 1, rem + 1)
}

/// 清理过期备份：保留最近 20 个，按 mtime 升序删除最旧的
fn prune_old_backups(dir: &Path) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    let mut files: Vec<(PathBuf, SystemTime, u64)> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("ini") {
            let mtime = entry
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            files.push((path, mtime, size));
        }
    }
    if files.len() <= 20 {
        return;
    }
    // 按 mtime 升序，删除最旧的（保留最近 20 个）
    files.sort_by_key(|(_, mtime, _)| *mtime);
    let to_remove = files.len().saturating_sub(20);
    for (path, _, _) in files.iter().take(to_remove) {
        let _ = std::fs::remove_file(path);
    }
}

/// 在 write_config 调用时自动备份现有配置
fn backup_existing_config(path: &str) {
    let src = Path::new(path);
    if !src.exists() {
        return;
    }
    let dir = match backups_dir() {
        Ok(d) => d,
        Err(_) => return,
    };
    let ts = format_timestamp(SystemTime::now());
    let backup_name = format!("PalWorldSettings-{}.ini", ts);
    let dst = dir.join(&backup_name);
    if std::fs::copy(src, &dst).is_ok() {
        prune_old_backups(&dir);
    }
}

#[command]
pub fn list_config_backups() -> Result<Vec<BackupInfo>, String> {
    let dir = backups_dir()?;
    let mut backups = Vec::new();
    let entries = std::fs::read_dir(&dir).map_err(|e| format!("读取备份目录失败: {}", e))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("ini") {
            let name = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            let mtime = entry
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            let timestamp = format_timestamp(mtime);
            let size_bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
            backups.push(BackupInfo {
                name,
                timestamp,
                size_bytes,
            });
        }
    }
    // 按时间降序（最新在前）
    backups.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    Ok(backups)
}

#[command]
pub fn restore_config_backup(name: String, server_path: String) -> Result<String, String> {
    let dir = backups_dir()?;
    // 防止路径穿越：仅使用文件名
    let safe_name = Path::new(&name)
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| "备份文件名非法".to_string())?
        .to_string();
    let backup_path = dir.join(&safe_name);
    if !backup_path.exists() {
        return Err(format!("备份文件不存在: {}", safe_name));
    }
    let target = PathBuf::from(&server_path)
        .join("Pal")
        .join("Saved")
        .join("Config")
        .join("WindowsServer")
        .join("PalWorldSettings.ini");
    if let Some(parent) = target.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent).map_err(|e| format!("创建目标目录失败: {}", e))?;
        }
    }
    std::fs::copy(&backup_path, &target).map_err(|e| format!("恢复备份失败: {}", e))?;
    Ok(format!("已从备份 {} 恢复配置", safe_name))
}

// ==================== 配置管理 ====================

#[derive(Serialize, Deserialize, Clone)]
pub struct ConfigValue {
    pub name: String,
    pub value: String,
    pub description: String,
    pub field_type: String,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub step: Option<f64>,
}

// ==================== 配置管理 ====================

/// 返回一份完整的默认配置（首次启动、配置文件尚不存在时使用）。
/// 同时被 `read_config`（文件不存在兜底）与 `get_default_config` 复用，避免默认值重复定义。
fn default_config_map() -> HashMap<String, String> {
    // 从实际的 DefaultPalWorldSettings.ini 提取的最新默认配置
    let defaults = vec![
        ("Difficulty", "None"),
        ("RandomizerType", "None"),
        ("RandomizerSeed", "\"\""),
        ("bIsRandomizerPalLevelRandom", "False"),
        ("DayTimeSpeedRate", "1.000000"),
        ("NightTimeSpeedRate", "1.000000"),
        ("ExpRate", "1.000000"),
        ("PalCaptureRate", "1.000000"),
        ("PalSpawnNumRate", "1.000000"),
        ("PalDamageRateAttack", "1.000000"),
        ("PalDamageRateDefense", "1.000000"),
        ("PlayerDamageRateAttack", "1.000000"),
        ("PlayerDamageRateDefense", "1.000000"),
        ("PlayerStomachDecreaceRate", "1.000000"),
        ("PlayerStaminaDecreaceRate", "1.000000"),
        ("PlayerAutoHPRegeneRate", "1.000000"),
        ("PlayerAutoHpRegeneRateInSleep", "1.000000"),
        ("PalStomachDecreaceRate", "1.000000"),
        ("PalStaminaDecreaceRate", "1.000000"),
        ("PalAutoHPRegeneRate", "1.000000"),
        ("PalAutoHpRegeneRateInSleep", "1.000000"),
        ("BuildObjectHpRate", "1.000000"),
        ("BuildObjectDamageRate", "1.000000"),
        ("BuildObjectDeteriorationDamageRate", "1.000000"),
        ("CollectionDropRate", "1.000000"),
        ("CollectionObjectHpRate", "1.000000"),
        ("CollectionObjectRespawnSpeedRate", "1.000000"),
        ("EnemyDropItemRate", "1.000000"),
        ("DeathPenalty", "Item"),
        ("bEnablePlayerToPlayerDamage", "False"),
        ("bEnableFriendlyFire", "False"),
        ("bEnableInvaderEnemy", "True"),
        ("bActiveUNKO", "False"),
        ("bEnableAimAssistPad", "True"),
        ("bEnableAimAssistKeyboard", "False"),
        ("DropItemMaxNum", "3000"),
        ("PhysicsActiveDropItemMaxNum", "-1"),
        ("DropItemMaxNum_UNKO", "100"),
        ("BaseCampMaxNum", "128"),
        ("BaseCampWorkerMaxNum", "15"),
        ("DropItemAliveMaxHours", "1.000000"),
        ("bAutoResetGuildNoOnlinePlayers", "False"),
        ("AutoResetGuildTimeNoOnlinePlayers", "72.000000"),
        ("GuildPlayerMaxNum", "20"),
        ("BaseCampMaxNumInGuild", "4"),
        ("PalEggDefaultHatchingTime", "1.000000"),
        ("WorkSpeedRate", "1.000000"),
        ("AutoSaveSpan", "30.000000"),
        ("bIsMultiplay", "False"),
        ("bIsPvP", "False"),
        ("bHardcore", "False"),
        ("bPalLost", "False"),
        ("bCharacterRecreateInHardcore", "False"),
        ("bCanPickupOtherGuildDeathPenaltyDrop", "False"),
        ("bEnableNonLoginPenalty", "True"),
        ("bEnableFastTravel", "True"),
        ("bEnableFastTravelOnlyBaseCamp", "False"),
        ("bIsStartLocationSelectByMap", "False"),
        ("bExistPlayerAfterLogout", "False"),
        ("bEnableDefenseOtherGuildPlayer", "False"),
        ("bInvisibleOtherGuildBaseCampAreaFX", "False"),
        ("bBuildAreaLimit", "False"),
        ("ItemWeightRate", "1.000000"),
        ("CoopPlayerMaxNum", "4"),
        ("ServerPlayerMaxNum", "32"),
        ("ServerName", "\"Default Palworld Server\""),
        ("ServerDescription", "\"\""),
        ("AdminPassword", "\"\""),
        ("ServerPassword", "\"\""),
        ("bAllowClientMod", "True"),
        ("PublicPort", "8211"),
        ("PublicIP", "\"\""),
        ("RCONEnabled", "False"),
        ("RCONPort", "25575"),
        ("Region", "\"\""),
        ("bUseAuth", "True"),
        (
            "BanListURL",
            "\"https://b.palworldgame.com/api/banlist.txt\"",
        ),
        ("RESTAPIEnabled", "False"),
        ("RESTAPIPort", "8212"),
        ("bShowPlayerList", "False"),
        ("ChatPostLimitPerMinute", "30"),
        ("CrossplayPlatforms", "(Steam,Xbox,PS5,Mac)"),
        ("bIsUseBackupSaveData", "True"),
        ("LogFormatType", "Text"),
        ("bIsShowJoinLeftMessage", "True"),
        ("SupplyDropSpan", "180"),
        ("EnablePredatorBossPal", "True"),
        ("MaxBuildingLimitNum", "0"),
        ("ServerReplicatePawnCullDistance", "15000.000000"),
        ("bAllowGlobalPalboxExport", "True"),
        ("bAllowGlobalPalboxImport", "False"),
        ("EquipmentDurabilityDamageRate", "1.000000"),
        ("ItemContainerForceMarkDirtyInterval", "1.000000"),
        ("PlayerDataPalStorageUpdateCheckTickInterval", "1.000000"),
        ("ItemCorruptionMultiplier", "1.000000"),
        ("MonsterFarmActionSpeedRate", "1.000000"),
        ("DenyTechnologyList", ""),
        ("GuildRejoinCooldownMinutes", "0"),
        ("AutoTransferMasterCheckIntervalSeconds", "3600.000000"),
        ("AutoTransferMasterThresholdDays", "14"),
        ("MaxGuildsPerFrame", "10"),
        ("BlockRespawnTime", "5.000000"),
        ("RespawnPenaltyDurationThreshold", "0.000000"),
        ("RespawnPenaltyTimeScale", "2.000000"),
        ("bDisplayPvPItemNumOnWorldMap_BaseCamp", "False"),
        ("bDisplayPvPItemNumOnWorldMap_Player", "False"),
        (
            "AdditionalDropItemWhenPlayerKillingInPvPMode",
            "\"PlayerDropItem\"",
        ),
        ("AdditionalDropItemNumWhenPlayerKillingInPvPMode", "1"),
        ("bAdditionalDropItemWhenPlayerKillingInPvPMode", "False"),
        ("bEnableVoiceChat", "False"),
        ("VoiceChatMaxVolumeDistance", "3000.000000"),
        ("VoiceChatZeroVolumeDistance", "15000.000000"),
        ("bAllowEnhanceStat_Health", "True"),
        ("bAllowEnhanceStat_Attack", "True"),
        ("bAllowEnhanceStat_Stamina", "True"),
        ("bAllowEnhanceStat_Weight", "True"),
        ("bAllowEnhanceStat_WorkSpeed", "True"),
        ("bEnableBuildingPlayerUIdDisplay", "False"),
        ("BuildingNameDisplayCacheTTLSeconds", "60"),
    ];

    let mut map = HashMap::new();
    for (k, v) in defaults {
        map.insert(k.to_string(), v.to_string());
    }
    map
}

/// 从文件读取并解析 PalWorldSettings.ini（非 #[command]，供 rest_proxy 调用复用）。
/// 配置文件尚不存在时返回默认配置，使配置页/REST 代理在首次启动前也能工作。
pub fn read_config_from_file(path: &str) -> Result<HashMap<String, String>, String> {
    // 配置文件尚不存在（典型场景：用户还未启动过服务器，PalWorldSettings.ini 未生成）时，
    // 直接返回默认配置，使配置页在首次启动前也能正常展示、可编辑后保存。
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(default_config_map());
        }
        Err(e) => return Err(format!("读取配置文件失败: {}", e)),
    };

    let mut config = HashMap::new();

    if let Some(start) = content.find('(') {
        if let Some(end) = content.rfind(')') {
            let inner = &content[start + 1..end];
            // 解析: key=value 对，处理引号内和括号内的逗号
            let mut current_key = String::new();
            let mut current_value = String::new();
            let mut in_quotes = false;
            let mut in_parens = 0i32;
            let mut parsing_key = true;

            for ch in inner.chars() {
                match ch {
                    '"' => {
                        in_quotes = !in_quotes;
                        if !parsing_key {
                            current_value.push(ch);
                        }
                    }
                    '(' if !in_quotes && !parsing_key => {
                        in_parens += 1;
                        current_value.push(ch);
                    }
                    ')' if !in_quotes && !parsing_key => {
                        in_parens -= 1;
                        current_value.push(ch);
                    }
                    '=' if !in_quotes && parsing_key => {
                        parsing_key = false;
                    }
                    ',' if !in_quotes && !parsing_key && in_parens == 0 => {
                        if !current_key.is_empty() {
                            config.insert(
                                current_key.trim().to_string(),
                                current_value.trim().to_string(),
                            );
                        }
                        current_key.clear();
                        current_value.clear();
                        parsing_key = true;
                    }
                    _ => {
                        if parsing_key {
                            current_key.push(ch);
                        } else {
                            current_value.push(ch);
                        }
                    }
                }
            }

            if !current_key.is_empty() {
                config.insert(
                    current_key.trim().to_string(),
                    current_value.trim().to_string(),
                );
            }
        }
    }

    Ok(config)
}

/// 从配置 HashMap 中提取 AdminPassword（去引号 + 去单引号），供 rest_proxy 构建 Basic Auth。
/// Q3 遗留清理：模板可能保留 `"pass"` 或 `'pass'`，统一吃掉引号避免认证失败。
pub fn extract_admin_password(config: &HashMap<String, String>) -> String {
    config
        .get("AdminPassword")
        .map(|s| {
            s.trim()
                .trim_matches('"')
                .trim_matches('\'')
                .trim()
                .to_string()
        })
        .unwrap_or_default()
}

/// 从配置 HashMap 中提取 RCONPort（默认 25575），供 rcon.rs 连接使用（Q2）。
#[cfg(test)]
pub fn extract_rcon_port(config: &HashMap<String, String>) -> u16 {
    config
        .get("RCONPort")
        .and_then(|s| {
            let trimmed = s.trim().trim_matches('"').trim_matches('\'').trim();
            trimmed.parse::<u16>().ok()
        })
        .unwrap_or(25575)
}

/// 读取 RCON 连接凭据（AdminPassword + RCONPort），供 rcon_connect_using_config 使用（Q2）。
/// 路径：{server_path}/Pal/Saved/Config/WindowsServer/PalWorldSettings.ini
#[cfg(test)]
pub fn read_rcon_credentials(server_path: &str) -> Result<(String, u16), String> {
    let config_path = Path::new(server_path)
        .join("Pal")
        .join("Saved")
        .join("Config")
        .join("WindowsServer")
        .join("PalWorldSettings.ini");
    let config_map = read_config_from_file(config_path.to_str().ok_or("服务器路径包含非法字符")?)?;
    let password = extract_admin_password(&config_map);
    let port = extract_rcon_port(&config_map);
    Ok((password, port))
}

/// 从配置 HashMap 中提取 RESTAPIPort（默认 8212），供 rest_proxy 拼接 Base URL。
pub fn extract_rest_port(config: &HashMap<String, String>) -> u16 {
    config
        .get("RESTAPIPort")
        .and_then(|s| {
            let trimmed = s.trim().trim_matches('"');
            trimmed.parse::<u16>().ok()
        })
        .unwrap_or(8212)
}

#[command]
pub async fn read_config(path: String) -> Result<HashMap<String, String>, String> {
    // 委托给内部函数，供 rest_proxy.rs 复用同一套解析逻辑
    read_config_from_file(&path)
}

#[command]
pub async fn write_config(path: String, config: HashMap<String, String>) -> Result<String, String> {
    // 失败边界记录项目日志：配置写入是高频破坏性操作，失败必须落盘以便用户反馈
    // 「哪里错了」；成功也记一条 INFO，便于追溯写入时间线。整个业务体包进 IIFE，
    // 在边界统一记录，不污染每个 ? 的错误信息。
    let result = (|| -> Result<String, String> {
        // 写入前备份现有配置（若文件存在）
        backup_existing_config(&path);

        let mut options: Vec<(String, String)> = config.into_iter().collect();
        options.sort_by(|a, b| a.0.cmp(&b.0));

        let lines: Vec<String> = options
            .into_iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect();

        let content = format!(
            "[/Script/Pal.PalGameWorldSettings]\nOptionSettings=({})\n",
            lines.join(",")
        );

        // 首次启动前 WindowsServer 目录可能尚未被服务器创建，写入前先确保父目录存在
        if let Some(parent) = Path::new(&path).parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                std::fs::create_dir_all(parent).map_err(|e| format!("创建配置目录失败: {}", e))?;
            }
        }

        std::fs::write(&path, content).map_err(|e| format!("写入配置文件失败: {}", e))?;

        Ok("配置文件已保存".to_string())
    })();
    match &result {
        Ok(_) => {
            crate::app_log::record("INFO", "config.write", "配置文件已保存", &[("path", &path)])
        }
        Err(error) => crate::app_log::record("ERROR", "config.write", error, &[("path", &path)]),
    }
    result
}

#[command]
pub async fn get_default_config() -> Result<HashMap<String, String>, String> {
    // 复用默认配置定义，避免与 read_config 重复
    Ok(default_config_map())
}

/// 一键填充默认配置的结果（前端 Toast 直接使用 message 字段）
#[derive(Serialize, Clone)]
pub struct FillConfigResult {
    /// "already_filled" | "filled_from_template" | "filled_from_defaults"
    pub status: String,
    /// 实际命中/写入的来源路径或来源标识
    pub source: String,
    /// 面向用户的中文提示
    pub message: String,
}

fn has_option_settings_assignment(content: &str) -> bool {
    content.lines().any(|line| {
        let line = line.trim_start_matches('\u{feff}').trim();
        let Some((name, value)) = line.split_once('=') else {
            return false;
        };
        name.trim().eq_ignore_ascii_case("OptionSettings") && value.trim_start().starts_with('(')
    })
}

/// 一键填充默认配置（仅手动按钮触发，绝不接入 start_server 自动守卫）。
///
/// 四级策略：
/// ① live 已含 `OptionSettings=(` → 跳过（already_filled）
/// ② 模板 `DefaultPalWorldSettings.ini` 存在 → 先备份再复制（filled_from_template）
/// ③ 模板缺失 → 用内置 `default_config_map()` 物化兜底（filled_from_defaults）
/// ④ 两者皆无 → 明确报错，绝不静默（Err）
///
/// 路径仅 Windows 专用服：`{server_path}/Pal/.../WindowsServer/`
#[command]
pub async fn fill_default_config(server_path: String) -> Result<FillConfigResult, String> {
    if server_path.trim().is_empty() {
        return Err("未设置服务器路径（server_path 为空）。请到【设置】填写 PalServer.exe 所在根目录后再试。".into());
    }
    let live = PathBuf::from(&server_path)
        .join("Pal")
        .join("Saved")
        .join("Config")
        .join("WindowsServer")
        .join("PalWorldSettings.ini");
    let template = PathBuf::from(&server_path)
        .join("Pal")
        .join("Config")
        .join("WindowsServer")
        .join("DefaultPalWorldSettings.ini");

    // ① 已填好（含 OptionSettings=( ）→ 跳过
    if live.exists() {
        if let Ok(content) = std::fs::read_to_string(&live) {
            if has_option_settings_assignment(&content) {
                return Ok(FillConfigResult {
                    status: "already_filled".into(),
                    source: live.to_string_lossy().into(),
                    message: "PalWorldSettings.ini 已含配置，无需填充。".into(),
                });
            }
        }
    }

    // ② 模板存在 → 先备份 live（若有），再复制模板
    if template.exists() {
        backup_existing_config(live.to_str().unwrap_or("")); // 复用 config.rs 既有备份逻辑
                                                             // 确保父目录存在（Pal/Saved/Config/WindowsServer/ 可能尚未创建），与三级分支一致
        if let Some(parent) = live.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                std::fs::create_dir_all(parent).map_err(|e| format!("创建配置目录失败: {}", e))?;
            }
        }
        std::fs::copy(&template, &live).map_err(|e| format!("复制默认模板失败: {}", e))?;
        return Ok(FillConfigResult {
            status: "filled_from_template".into(),
            source: template.to_string_lossy().into(),
            message: "已从 DefaultPalWorldSettings.ini 填充 PalWorldSettings.ini，可正常开服。"
                .into(),
        });
    }

    // ③ 模板缺失 → 用内置默认值物化兜底
    let map = default_config_map();
    if !map.is_empty() {
        let mut options: Vec<(String, String)> = map.into_iter().collect();
        options.sort_by(|a, b| a.0.cmp(&b.0));
        let lines: Vec<String> = options
            .into_iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect();
        let content = format!(
            "[ /Script/Pal.PalGameWorldSettings ]\nOptionSettings=({})\n",
            lines.join(",")
        );
        if let Some(parent) = live.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                std::fs::create_dir_all(parent).map_err(|e| format!("创建配置目录失败: {}", e))?;
            }
        }
        std::fs::write(&live, content).map_err(|e| format!("写入默认配置失败: {}", e))?;
        return Ok(FillConfigResult {
            status: "filled_from_defaults".into(),
            source: "builtin default_config_map()".into(),
            message: "未找到默认模板，已用内置默认值初始化 PalWorldSettings.ini。".into(),
        });
    }

    // ④ 两者皆无 → 明确报错，绝不静默
    Err("未找到默认配置模板 DefaultPalWorldSettings.ini，且内置默认值不可用。请确认 server_path 指向 PalServer 根目录。".into())
}

/// 只读探测：live 配置文件是否已初始化（含 `OptionSettings=(`）。
/// 供前端「空白横幅」判断显隐，不写盘。
#[command]
pub async fn is_config_initialized(server_path: String) -> Result<bool, String> {
    if server_path.trim().is_empty() {
        return Ok(false);
    }
    let live = PathBuf::from(&server_path)
        .join("Pal")
        .join("Saved")
        .join("Config")
        .join("WindowsServer")
        .join("PalWorldSettings.ini");
    if !live.exists() {
        return Ok(false);
    }
    let content = std::fs::read_to_string(&live).map_err(|e| format!("读取配置文件失败: {}", e))?;
    Ok(has_option_settings_assignment(&content))
}

#[command]
pub async fn get_config_descriptions() -> Result<Vec<ConfigValue>, String> {
    Ok(vec![
        ConfigValue {
            name: "Difficulty".to_string(),
            value: "None".to_string(),
            description: "游戏难度".to_string(),
            field_type: "select".to_string(),
            min: None,
            max: None,
            step: None,
        },
        ConfigValue {
            name: "RandomizerType".to_string(),
            value: "None".to_string(),
            description: "随机化模式: None/Pal/Item/PalAndItem".to_string(),
            field_type: "select".to_string(),
            min: None,
            max: None,
            step: None,
        },
        ConfigValue {
            name: "DayTimeSpeedRate".to_string(),
            value: "1.0".to_string(),
            description: "白天时间速率".to_string(),
            field_type: "range".to_string(),
            min: Some(0.1),
            max: Some(5.0),
            step: Some(0.1),
        },
        ConfigValue {
            name: "NightTimeSpeedRate".to_string(),
            value: "1.0".to_string(),
            description: "夜间时间速率".to_string(),
            field_type: "range".to_string(),
            min: Some(0.1),
            max: Some(5.0),
            step: Some(0.1),
        },
        ConfigValue {
            name: "ExpRate".to_string(),
            value: "1.0".to_string(),
            description: "经验值倍率".to_string(),
            field_type: "range".to_string(),
            min: Some(0.1),
            max: Some(20.0),
            step: Some(0.1),
        },
        ConfigValue {
            name: "PalCaptureRate".to_string(),
            value: "1.0".to_string(),
            description: "帕鲁捕获率".to_string(),
            field_type: "range".to_string(),
            min: Some(0.1),
            max: Some(10.0),
            step: Some(0.1),
        },
        ConfigValue {
            name: "PalSpawnNumRate".to_string(),
            value: "1.0".to_string(),
            description: "帕鲁出现率（过高影响性能）".to_string(),
            field_type: "range".to_string(),
            min: Some(0.1),
            max: Some(5.0),
            step: Some(0.1),
        },
        ConfigValue {
            name: "PalDamageRateAttack".to_string(),
            value: "1.0".to_string(),
            description: "帕鲁攻击伤害倍率".to_string(),
            field_type: "range".to_string(),
            min: Some(0.1),
            max: Some(5.0),
            step: Some(0.1),
        },
        ConfigValue {
            name: "PalDamageRateDefense".to_string(),
            value: "1.0".to_string(),
            description: "对帕鲁的防御伤害倍率".to_string(),
            field_type: "range".to_string(),
            min: Some(0.1),
            max: Some(5.0),
            step: Some(0.1),
        },
        ConfigValue {
            name: "PlayerDamageRateAttack".to_string(),
            value: "1.0".to_string(),
            description: "玩家攻击伤害倍率".to_string(),
            field_type: "range".to_string(),
            min: Some(0.1),
            max: Some(5.0),
            step: Some(0.1),
        },
        ConfigValue {
            name: "PlayerDamageRateDefense".to_string(),
            value: "1.0".to_string(),
            description: "对玩家的防御伤害倍率".to_string(),
            field_type: "range".to_string(),
            min: Some(0.1),
            max: Some(5.0),
            step: Some(0.1),
        },
        ConfigValue {
            name: "PlayerStomachDecreaceRate".to_string(),
            value: "1.0".to_string(),
            description: "玩家饥饿消耗率".to_string(),
            field_type: "range".to_string(),
            min: Some(0.1),
            max: Some(5.0),
            step: Some(0.1),
        },
        ConfigValue {
            name: "PlayerStaminaDecreaceRate".to_string(),
            value: "1.0".to_string(),
            description: "玩家耐力消耗率".to_string(),
            field_type: "range".to_string(),
            min: Some(0.1),
            max: Some(5.0),
            step: Some(0.1),
        },
        ConfigValue {
            name: "PlayerAutoHPRegeneRate".to_string(),
            value: "1.0".to_string(),
            description: "玩家自动HP恢复率".to_string(),
            field_type: "range".to_string(),
            min: Some(0.1),
            max: Some(5.0),
            step: Some(0.1),
        },
        ConfigValue {
            name: "PlayerAutoHpRegeneRateInSleep".to_string(),
            value: "1.0".to_string(),
            description: "玩家睡眠HP恢复率".to_string(),
            field_type: "range".to_string(),
            min: Some(0.1),
            max: Some(5.0),
            step: Some(0.1),
        },
        ConfigValue {
            name: "PalStomachDecreaceRate".to_string(),
            value: "1.0".to_string(),
            description: "帕鲁饥饿消耗率".to_string(),
            field_type: "range".to_string(),
            min: Some(0.1),
            max: Some(5.0),
            step: Some(0.1),
        },
        ConfigValue {
            name: "PalStaminaDecreaceRate".to_string(),
            value: "1.0".to_string(),
            description: "帕鲁耐力消耗率".to_string(),
            field_type: "range".to_string(),
            min: Some(0.1),
            max: Some(5.0),
            step: Some(0.1),
        },
        ConfigValue {
            name: "PalAutoHPRegeneRate".to_string(),
            value: "1.0".to_string(),
            description: "帕鲁自动HP恢复率".to_string(),
            field_type: "range".to_string(),
            min: Some(0.1),
            max: Some(5.0),
            step: Some(0.1),
        },
        ConfigValue {
            name: "PalAutoHpRegeneRateInSleep".to_string(),
            value: "1.0".to_string(),
            description: "帕鲁睡眠HP恢复率".to_string(),
            field_type: "range".to_string(),
            min: Some(0.1),
            max: Some(5.0),
            step: Some(0.1),
        },
        ConfigValue {
            name: "BuildObjectHpRate".to_string(),
            value: "1.0".to_string(),
            description: "建筑物HP倍率".to_string(),
            field_type: "range".to_string(),
            min: Some(0.1),
            max: Some(10.0),
            step: Some(0.1),
        },
        ConfigValue {
            name: "BuildObjectDamageRate".to_string(),
            value: "1.0".to_string(),
            description: "建筑物伤害倍率（0=无敌）".to_string(),
            field_type: "range".to_string(),
            min: Some(0.0),
            max: Some(5.0),
            step: Some(0.1),
        },
        ConfigValue {
            name: "BuildObjectDeteriorationDamageRate".to_string(),
            value: "1.0".to_string(),
            description: "建筑物损耗率（0=不劣化）".to_string(),
            field_type: "range".to_string(),
            min: Some(0.0),
            max: Some(5.0),
            step: Some(0.1),
        },
        ConfigValue {
            name: "CollectionDropRate".to_string(),
            value: "1.0".to_string(),
            description: "采集物品掉落倍率".to_string(),
            field_type: "range".to_string(),
            min: Some(0.1),
            max: Some(10.0),
            step: Some(0.1),
        },
        ConfigValue {
            name: "EnemyDropItemRate".to_string(),
            value: "1.0".to_string(),
            description: "敌人掉落物品倍率".to_string(),
            field_type: "range".to_string(),
            min: Some(0.1),
            max: Some(10.0),
            step: Some(0.1),
        },
        ConfigValue {
            name: "DeathPenalty".to_string(),
            value: "Item".to_string(),
            description: "死亡惩罚: None/Item/ItemAndEquipment/All".to_string(),
            field_type: "select".to_string(),
            min: None,
            max: None,
            step: None,
        },
        ConfigValue {
            name: "WorkSpeedRate".to_string(),
            value: "1.0".to_string(),
            description: "工作速度倍率".to_string(),
            field_type: "range".to_string(),
            min: Some(0.1),
            max: Some(10.0),
            step: Some(0.1),
        },
        ConfigValue {
            name: "PalEggDefaultHatchingTime".to_string(),
            value: "1.0".to_string(),
            description: "帕鲁蛋孵化时间（小时，0=即时）".to_string(),
            field_type: "range".to_string(),
            min: Some(0.0),
            max: Some(240.0),
            step: Some(0.1),
        },
        ConfigValue {
            name: "AutoSaveSpan".to_string(),
            value: "30.0".to_string(),
            description: "自动保存间隔（秒）".to_string(),
            field_type: "range".to_string(),
            min: Some(10.0),
            max: Some(3600.0),
            step: Some(10.0),
        },
        ConfigValue {
            name: "ItemWeightRate".to_string(),
            value: "1.0".to_string(),
            description: "物品重量倍率（越低背包越轻松）".to_string(),
            field_type: "range".to_string(),
            min: Some(0.1),
            max: Some(5.0),
            step: Some(0.1),
        },
        ConfigValue {
            name: "EquipmentDurabilityDamageRate".to_string(),
            value: "1.0".to_string(),
            description: "装备耐久损耗倍率（0=不掉耐久）".to_string(),
            field_type: "range".to_string(),
            min: Some(0.0),
            max: Some(5.0),
            step: Some(0.1),
        },
        ConfigValue {
            name: "DropItemMaxNum".to_string(),
            value: "3000".to_string(),
            description: "掉落物品最大数量".to_string(),
            field_type: "number".to_string(),
            min: Some(0.0),
            max: Some(10000.0),
            step: Some(100.0),
        },
        ConfigValue {
            name: "BaseCampWorkerMaxNum".to_string(),
            value: "15".to_string(),
            description: "单个基地帕鲁数（最高50）".to_string(),
            field_type: "number".to_string(),
            min: Some(1.0),
            max: Some(50.0),
            step: Some(1.0),
        },
        ConfigValue {
            name: "SupplyDropSpan".to_string(),
            value: "180".to_string(),
            description: "空投间隔（分钟）".to_string(),
            field_type: "number".to_string(),
            min: Some(0.0),
            max: Some(1440.0),
            step: Some(10.0),
        },
        ConfigValue {
            name: "MaxBuildingLimitNum".to_string(),
            value: "0".to_string(),
            description: "建筑上限（0=无限）".to_string(),
            field_type: "number".to_string(),
            min: Some(0.0),
            max: Some(100000.0),
            step: Some(100.0),
        },
        ConfigValue {
            name: "ServerPlayerMaxNum".to_string(),
            value: "32".to_string(),
            description: "服务器最大玩家数".to_string(),
            field_type: "number".to_string(),
            min: Some(1.0),
            max: Some(32.0),
            step: Some(1.0),
        },
        ConfigValue {
            name: "ServerName".to_string(),
            value: "Default Palworld Server".to_string(),
            description: "服务器名称".to_string(),
            field_type: "text".to_string(),
            min: None,
            max: None,
            step: None,
        },
        ConfigValue {
            name: "ServerPassword".to_string(),
            value: "".to_string(),
            description: "服务器密码（留空无密码）".to_string(),
            field_type: "text".to_string(),
            min: None,
            max: None,
            step: None,
        },
        ConfigValue {
            name: "AdminPassword".to_string(),
            value: "".to_string(),
            description: "管理员密码".to_string(),
            field_type: "text".to_string(),
            min: None,
            max: None,
            step: None,
        },
        ConfigValue {
            name: "bIsPvP".to_string(),
            value: "False".to_string(),
            description: "PvP模式".to_string(),
            field_type: "toggle".to_string(),
            min: None,
            max: None,
            step: None,
        },
        ConfigValue {
            name: "bHardcore".to_string(),
            value: "False".to_string(),
            description: "硬核模式（死亡不可复活）".to_string(),
            field_type: "toggle".to_string(),
            min: None,
            max: None,
            step: None,
        },
        ConfigValue {
            name: "bEnableInvaderEnemy".to_string(),
            value: "True".to_string(),
            description: "启用入侵敌人（袭击事件）".to_string(),
            field_type: "toggle".to_string(),
            min: None,
            max: None,
            step: None,
        },
        ConfigValue {
            name: "bEnableFastTravel".to_string(),
            value: "True".to_string(),
            description: "启用快速旅行".to_string(),
            field_type: "toggle".to_string(),
            min: None,
            max: None,
            step: None,
        },
        ConfigValue {
            name: "bShowPlayerList".to_string(),
            value: "False".to_string(),
            description: "显示玩家列表".to_string(),
            field_type: "toggle".to_string(),
            min: None,
            max: None,
            step: None,
        },
        ConfigValue {
            name: "bIsUseBackupSaveData".to_string(),
            value: "True".to_string(),
            description: "启用自动备份存档".to_string(),
            field_type: "toggle".to_string(),
            min: None,
            max: None,
            step: None,
        },
        ConfigValue {
            name: "bEnableVoiceChat".to_string(),
            value: "False".to_string(),
            description: "启用语音聊天".to_string(),
            field_type: "toggle".to_string(),
            min: None,
            max: None,
            step: None,
        },
        ConfigValue {
            name: "bIsShowJoinLeftMessage".to_string(),
            value: "True".to_string(),
            description: "显示加入/退出消息".to_string(),
            field_type: "toggle".to_string(),
            min: None,
            max: None,
            step: None,
        },
        ConfigValue {
            name: "EnablePredatorBossPal".to_string(),
            value: "True".to_string(),
            description: "启用捕食者BOSS帕鲁".to_string(),
            field_type: "toggle".to_string(),
            min: None,
            max: None,
            step: None,
        },
        ConfigValue {
            name: "RCONEnabled".to_string(),
            value: "False".to_string(),
            description: "启用RCON远程管理".to_string(),
            field_type: "toggle".to_string(),
            min: None,
            max: None,
            step: None,
        },
        ConfigValue {
            name: "RESTAPIEnabled".to_string(),
            value: "False".to_string(),
            description: "启用REST API".to_string(),
            field_type: "toggle".to_string(),
            min: None,
            max: None,
            step: None,
        },
    ])
}

// ==================== 单元测试（QA 收官 · 严过关） ====================
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn extract_admin_password_strips_quotes() {
        // Q3 遗留清理：模板可能保留 "pass" 或 'pass'，统一吃掉引号避免认证失败
        let mut m = HashMap::new();
        m.insert("AdminPassword".to_string(), "\"secret\"".to_string());
        assert_eq!(extract_admin_password(&m), "secret");

        let mut m2 = HashMap::new();
        m2.insert("AdminPassword".to_string(), "'secret'".to_string());
        assert_eq!(extract_admin_password(&m2), "secret");

        let mut m3 = HashMap::new();
        m3.insert("AdminPassword".to_string(), "plain".to_string());
        assert_eq!(extract_admin_password(&m3), "plain");

        let empty = HashMap::new();
        assert_eq!(extract_admin_password(&empty), "");
    }

    #[test]
    fn extract_rcon_port_parses_and_defaults() {
        let mut m = HashMap::new();
        m.insert("RCONPort".to_string(), "25575".to_string());
        assert_eq!(extract_rcon_port(&m), 25575);

        let mut m2 = HashMap::new();
        m2.insert("RCONPort".to_string(), "9999".to_string());
        assert_eq!(extract_rcon_port(&m2), 9999);

        // 引号也应被吃掉
        let mut m3 = HashMap::new();
        m3.insert("RCONPort".to_string(), "\"27015\"".to_string());
        assert_eq!(extract_rcon_port(&m3), 27015);

        // 缺失 → 默认 25575
        let empty = HashMap::new();
        assert_eq!(extract_rcon_port(&empty), 25575);
    }

    #[test]
    fn read_rcon_credentials_parses_sample_ini() {
        // 路径：{server_path}/Pal/Saved/Config/WindowsServer/PalWorldSettings.ini
        let dir = std::env::temp_dir().join(format!("pwsm_test_{}", std::process::id()));
        let cfg_dir = dir
            .join("Pal")
            .join("Saved")
            .join("Config")
            .join("WindowsServer");
        std::fs::create_dir_all(&cfg_dir).unwrap();
        let ini = cfg_dir.join("PalWorldSettings.ini");
        let content = "[/Script/Pal.PalGameWorldSettings]\nOptionSettings=(AdminPassword=\"topsecret\",RCONPort=25575,Difficulty=None)\n";
        let mut f = std::fs::File::create(&ini).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        drop(f);

        let (pw, port) = read_rcon_credentials(dir.to_str().unwrap()).unwrap();
        assert_eq!(pw, "topsecret");
        assert_eq!(port, 25575);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_rcon_credentials_empty_password_when_unset() {
        let dir = std::env::temp_dir().join(format!("pwsm_test2_{}", std::process::id()));
        let cfg_dir = dir
            .join("Pal")
            .join("Saved")
            .join("Config")
            .join("WindowsServer");
        std::fs::create_dir_all(&cfg_dir).unwrap();
        let ini = cfg_dir.join("PalWorldSettings.ini");
        let content = "[/Script/Pal.PalGameWorldSettings]\nOptionSettings=(AdminPassword=\"\",RCONPort=25575)\n";
        let mut f = std::fs::File::create(&ini).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        drop(f);

        let (pw, _port) = read_rcon_credentials(dir.to_str().unwrap()).unwrap();
        // 空密码 → rcon_connect_using_config 内部 if password.is_empty() 应返回明确错误
        assert_eq!(pw, "");

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ====== fill_default_config / is_config_initialized 单元测试（T4 收官 · 严过关）======

    /// 构造临时 server_path 目录起点（含唯一子目录，避免并行测试互相干扰），调用方负责清理。
    fn make_qa_server_path(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("pwqa_{}_{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir); // 确保干净起点
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// 拼接 live 配置文件路径（与 fill_default_config 内部一致）
    fn qa_live_path(sp: &std::path::Path) -> std::path::PathBuf {
        sp.join("Pal")
            .join("Saved")
            .join("Config")
            .join("WindowsServer")
            .join("PalWorldSettings.ini")
    }

    #[test]
    fn fill_level_1_already_filled_does_not_write() {
        // ① live 已含 OptionSettings=( → already_filled，且不写盘
        let sp = make_qa_server_path("lvl1");
        let ws = sp
            .join("Pal")
            .join("Saved")
            .join("Config")
            .join("WindowsServer");
        std::fs::create_dir_all(&ws).unwrap();
        let live = ws.join("PalWorldSettings.ini");
        let preset = "[/Script/Pal.PalGameWorldSettings]\nOptionSettings=(Difficulty=None,ServerName=\"X\")\n";
        std::fs::write(&live, preset).unwrap();
        let mtime_before = std::fs::metadata(&live).unwrap().modified().unwrap();

        let res = tauri::async_runtime::block_on(fill_default_config(sp.to_string_lossy().into()))
            .expect("level1 不应报错");
        assert_eq!(res.status, "already_filled");
        // 关键：不得改写已填充的 live（内容不变 + mtime 不变）
        assert_eq!(
            std::fs::read_to_string(&live).unwrap(),
            preset,
            "live 内容被意外改写"
        );
        assert_eq!(
            std::fs::metadata(&live).unwrap().modified().unwrap(),
            mtime_before,
            "fill_default_config 不应触碰已填充的 live（mtime 变化）"
        );

        let _ = std::fs::remove_dir_all(&sp);
    }

    #[test]
    fn fill_level_2_from_template() {
        // ② 模板存在 → 复制模板 → filled_from_template
        // （备份既有 live 的行为见实现，为避免污染真实 AppData 备份目录，此处不预置旧 live 断言）
        let sp = make_qa_server_path("lvl2");
        let tpl_dir = sp.join("Pal").join("Config").join("WindowsServer");
        std::fs::create_dir_all(&tpl_dir).unwrap();
        let tpl = tpl_dir.join("DefaultPalWorldSettings.ini");
        let tpl_content = "[/Script/Pal.PalGameWorldSettings]\nOptionSettings=(Difficulty=Hardcore,ServerName=\"Tpl\")\n";
        std::fs::write(&tpl, tpl_content).unwrap();

        let res = tauri::async_runtime::block_on(fill_default_config(sp.to_string_lossy().into()))
            .expect("level2 不应报错");
        assert_eq!(res.status, "filled_from_template");

        let live = qa_live_path(&sp);
        assert!(live.exists(), "应从模板复制生成 live");
        assert_eq!(
            std::fs::read_to_string(&live).unwrap(),
            tpl_content,
            "live 内容应与模板一致"
        );

        let _ = std::fs::remove_dir_all(&sp);
    }

    #[test]
    fn fill_level_3_from_defaults() {
        // ③ 模板缺失 → 用内置 default_config_map() 物化兜底 → filled_from_defaults
        // 注意：default_config_map() 为硬编码非空，故「无模板的空目录」会走此分支而非 ④ 的 Err
        // （④ 仅 server_path 为空时可达）。此即 QA 任务中 ④「指向无模板的空目录」的实际正确落点。
        let sp = make_qa_server_path("lvl3");
        // 不创建模板，确保无 DefaultPalWorldSettings.ini
        let res = tauri::async_runtime::block_on(fill_default_config(sp.to_string_lossy().into()))
            .expect("level3 不应报错");
        assert_eq!(res.status, "filled_from_defaults");

        let live = qa_live_path(&sp);
        assert!(live.exists(), "应已物化写入 live");
        let got = std::fs::read_to_string(&live).unwrap();
        assert!(
            got.contains("OptionSettings=("),
            "写入内容应含 OptionSettings=("
        );
        assert!(got.contains("Difficulty=None"), "写入内容应含内置默认值");

        let _ = std::fs::remove_dir_all(&sp);
    }

    #[test]
    fn fill_level_4_empty_server_path_errors() {
        // ④ server_path 为空 → 明确 Err，绝不静默
        let res = tauri::async_runtime::block_on(fill_default_config("".to_string()));
        assert!(res.is_err(), "server_path 为空必须明确报错，不能静默");
        // 避免依赖 FillConfigResult: Debug（生产结构体未 derive Debug），用 match 取 Err 信息
        let msg = match res {
            Err(m) => m,
            Ok(_) => panic!("server_path 为空必须返回 Err，不能静默"),
        };
        assert!(
            msg.contains("server_path"),
            "错误信息应指明 server_path 为空: {}",
            msg
        );
    }

    #[test]
    fn is_config_initialized_empty_path_false() {
        let res = tauri::async_runtime::block_on(is_config_initialized("".to_string())).unwrap();
        assert!(!res, "空 server_path 应返回 false");
    }

    #[test]
    fn is_config_initialized_missing_file_false() {
        let sp = make_qa_server_path("ici_missing");
        let res =
            tauri::async_runtime::block_on(is_config_initialized(sp.to_string_lossy().into()))
                .unwrap();
        assert!(!res, "live 文件不存在应返回 false");
        let _ = std::fs::remove_dir_all(&sp);
    }

    #[test]
    fn is_config_initialized_no_option_settings_false() {
        let sp = make_qa_server_path("ici_noopt");
        let ws = sp
            .join("Pal")
            .join("Saved")
            .join("Config")
            .join("WindowsServer");
        std::fs::create_dir_all(&ws).unwrap();
        std::fs::write(
            ws.join("PalWorldSettings.ini"),
            "[/Script/Pal.PalGameWorldSettings]\nFoo=Bar\n",
        )
        .unwrap();
        let res =
            tauri::async_runtime::block_on(is_config_initialized(sp.to_string_lossy().into()))
                .unwrap();
        assert!(!res, "文件存在但不含 OptionSettings=( 应返回 false");
        let _ = std::fs::remove_dir_all(&sp);
    }

    #[test]
    fn is_config_initialized_with_option_settings_true() {
        let sp = make_qa_server_path("ici_ok");
        let ws = sp
            .join("Pal")
            .join("Saved")
            .join("Config")
            .join("WindowsServer");
        std::fs::create_dir_all(&ws).unwrap();
        std::fs::write(
            ws.join("PalWorldSettings.ini"),
            "[/Script/Pal.PalGameWorldSettings]\nOptionSettings=(Difficulty=None)\n",
        )
        .unwrap();
        let res =
            tauri::async_runtime::block_on(is_config_initialized(sp.to_string_lossy().into()))
                .unwrap();
        assert!(res, "文件存在且含 OptionSettings=( 应返回 true");
        let _ = std::fs::remove_dir_all(&sp);
    }

    #[test]
    fn is_config_initialized_accepts_bom_whitespace_and_case_variants() {
        for (suffix, content) in [
            (
                "ici_whitespace",
                "\u{feff}[/Script/Pal.PalGameWorldSettings]\nOptionSettings = (Difficulty=None)\n",
            ),
            (
                "ici_case",
                "[/Script/Pal.PalGameWorldSettings]\noptionsettings=(Difficulty=None)\n",
            ),
        ] {
            let sp = make_qa_server_path(suffix);
            let ws = sp
                .join("Pal")
                .join("Saved")
                .join("Config")
                .join("WindowsServer");
            std::fs::create_dir_all(&ws).unwrap();
            std::fs::write(ws.join("PalWorldSettings.ini"), content).unwrap();

            let initialized =
                tauri::async_runtime::block_on(is_config_initialized(sp.to_string_lossy().into()))
                    .unwrap();
            assert!(initialized, "配置变体应识别为已初始化: {suffix}");
            let _ = std::fs::remove_dir_all(&sp);
        }
    }

    #[test]
    fn write_config_updates_selected_values_and_round_trips_unknown_options() {
        let dir = make_qa_server_path("write_round_trip");
        let path = dir.join("PalWorldSettings.ini");
        std::fs::write(
            &path,
            "[/Script/Pal.PalGameWorldSettings]\nOptionSettings=(ExpRate=1.000000,UnknownFutureOption=KeepMe,ServerName=\"旧名称\")\n",
        )
        .unwrap();

        let mut config = read_config_from_file(path.to_str().unwrap()).unwrap();
        config.insert("ExpRate".to_string(), "1.300000".to_string());
        config.insert("ServerName".to_string(), "\"新名称\"".to_string());
        tauri::async_runtime::block_on(write_config(path.to_string_lossy().into(), config))
            .expect("副本配置应可写入");

        let reloaded = read_config_from_file(path.to_str().unwrap()).expect("写入结果应可重新解析");
        assert_eq!(reloaded.get("ExpRate"), Some(&"1.300000".to_string()));
        assert_eq!(reloaded.get("ServerName"), Some(&"\"新名称\"".to_string()));
        assert_eq!(
            reloaded.get("UnknownFutureOption"),
            Some(&"KeepMe".to_string())
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
