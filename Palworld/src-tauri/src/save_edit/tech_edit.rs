//! 修改器的角色科技点读写。
//!
//! 字段和兼容策略均以 PalworldSaveTools 的玩家资料面板为准：
//! - `SaveData.TechnologyPoint`：普通科技点；
//! - `SaveData.bossTechnologyPoint`：古代科技点。
//!
//! 每次只改写 `Players/<guid>.sav`，候选存档会先重新解析，再经同目录原子替换提交。

use std::path::{Path, PathBuf};

use gvas::properties::int_property::IntProperty;
use gvas::properties::Property;
use gvas::GvasFile;

use crate::save_edit::atomic_write::{self, FileMutation};
use crate::save_edit::models::{
    EditResult, PlayerTechnologyPoints, PlayerTechnologyPointsRequest,
    UpdatePlayerTechnologyPointsRequest,
};
use crate::save_edit::path_util;
use crate::save_edit::sav_io::{self, SavFile};

const MAX_TECHNOLOGY_POINTS: i32 = 9_999_999;

/// 将候选玩家存档验证后以原子事务替换。
fn save_player_gvas_atomic(
    sav: &SavFile,
    gvas: &GvasFile,
    world_data_dir: &Path,
    player_guid: &str,
) -> Result<EditResult, String> {
    let new_sav = SavFile::from_gvas(gvas, sav.compression)?;
    let roundtrip_ok = new_sav.roundtrip_ok();
    let candidate_bytes = new_sav.to_bytes()?;
    let candidate_path = world_data_dir
        .join("Players")
        .join(format!("{player_guid}.sav"));

    let candidate =
        SavFile::from_bytes(&candidate_bytes).map_err(|e| format!("候选存档校验失败: {e}"))?;
    candidate
        .parse()
        .map_err(|e| format!("候选存档校验失败: {e}"))?;

    atomic_write::commit_file_set(
        world_data_dir,
        &[FileMutation {
            relative_path: PathBuf::from("Players").join(format!("{player_guid}.sav")),
            content: Some(candidate_bytes),
        }],
    )?;

    SavFile::load(&candidate_path)
        .and_then(|persisted| persisted.parse().map(|_| ()))
        .map_err(|e| format!("写回后校验失败: {e}"))?;
    Ok(EditResult {
        ok: true,
        backup_id: String::new(),
        roundtrip_ok,
        warnings: Vec::new(),
    })
}

fn player_path(world_data_dir: &Path, player_guid: &str) -> Result<(String, PathBuf), String> {
    let player_guid = path_util::normalize_player_guid(player_guid)
        .ok_or_else(|| "角色文件名非法".to_string())?;
    let player_path = world_data_dir
        .join("Players")
        .join(format!("{player_guid}.sav"));
    if !player_path.is_file() {
        return Err("未找到所选角色的存档文件".to_string());
    }
    Ok((player_guid, player_path))
}

fn read_points_field(gvas: &GvasFile, field_name: &str) -> Result<i32, String> {
    let save_data = sav_io::top_field(gvas, "SaveData")
        .and_then(sav_io::struct_value)
        .and_then(sav_io::custom_fields)
        .ok_or_else(|| "玩家存档缺少 SaveData 结构".to_string())?;
    match sav_io::field(save_data, field_name) {
        Some(property) => {
            sav_io::as_int(property).ok_or_else(|| format!("玩家存档中的 {field_name} 不是整数"))
        }
        None => Ok(0),
    }
}

fn write_points_field(gvas: &mut GvasFile, field_name: &str, value: i32) -> Result<(), String> {
    let save_data = sav_io::top_field_mut(gvas, "SaveData")
        .and_then(sav_io::struct_value_mut)
        .and_then(sav_io::custom_fields_mut)
        .ok_or_else(|| "玩家存档缺少 SaveData 结构".to_string())?;

    if let Some(property) = sav_io::field_mut(save_data, field_name) {
        if let Property::IntProperty(integer) = property {
            integer.value = value;
            return Ok(());
        }
        return Err(format!("玩家存档中的 {field_name} 不是整数"));
    }

    // 与参考项目一致：老版本或新角色缺字段时补建为 IntProperty。
    save_data.insert(
        field_name.to_string(),
        vec![Property::IntProperty(IntProperty { value })],
    );
    Ok(())
}

fn player_technology_points_in_dir(
    world_data_dir: &Path,
    player_guid: &str,
) -> Result<PlayerTechnologyPoints, String> {
    let (_, player_path) = player_path(world_data_dir, player_guid)?;
    let gvas = SavFile::load(&player_path)?.parse()?;
    Ok(PlayerTechnologyPoints {
        technology_points: read_points_field(&gvas, "TechnologyPoint")?,
        ancient_technology_points: read_points_field(&gvas, "bossTechnologyPoint")?,
    })
}

fn update_technology_points_in_dir(
    world_data_dir: &Path,
    player_guid: &str,
    technology_points: i32,
    ancient_technology_points: i32,
) -> Result<EditResult, String> {
    if !(0..=MAX_TECHNOLOGY_POINTS).contains(&technology_points)
        || !(0..=MAX_TECHNOLOGY_POINTS).contains(&ancient_technology_points)
    {
        return Err(format!("科技点必须介于 0 和 {MAX_TECHNOLOGY_POINTS} 之间"));
    }

    let (player_guid, player_path) = player_path(world_data_dir, player_guid)?;
    let sav = SavFile::load(&player_path)?;
    let mut gvas = sav.parse()?;
    write_points_field(&mut gvas, "TechnologyPoint", technology_points)?;
    write_points_field(&mut gvas, "bossTechnologyPoint", ancient_technology_points)?;

    let mut result = save_player_gvas_atomic(&sav, &gvas, world_data_dir, &player_guid)?;
    result.warnings = vec![format!(
        "普通科技点已设为 {technology_points}，古代科技点已设为 {ancient_technology_points}"
    )];
    Ok(result)
}

/// 读取指定角色科技点。仅解析玩家存档，不会改写文件。
pub fn player_technology_points_impl(
    req: &PlayerTechnologyPointsRequest,
) -> Result<PlayerTechnologyPoints, String> {
    let world_data_dir = path_util::find_world_data_dir(Path::new(&req.world_path))
        .ok_or_else(|| "未找到世界数据(Level.sav)".to_string())?;
    player_technology_points_in_dir(&world_data_dir, &req.player_guid)
}

/// 原子更新指定角色的普通科技点和古代科技点。
pub fn update_player_technology_points_impl(
    req: &UpdatePlayerTechnologyPointsRequest,
) -> Result<EditResult, String> {
    let world_data_dir = path_util::find_world_data_dir(Path::new(&req.world_path))
        .ok_or_else(|| "未找到世界数据(Level.sav)".to_string())?;
    update_technology_points_in_dir(
        &world_data_dir,
        &req.player_guid,
        req.technology_points,
        req.ancient_technology_points,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn updates_technology_point_counts_in_real_backup_copy() {
        let source = PathBuf::from("F:/1/server/SaveGames/0/1A91A61548C7B6FD7B58B2B70710F7EE");
        assert!(source.is_dir(), "F:\\1 中的服务器备份应存在");
        let temp = std::env::temp_dir().join(format!(
            "palworld-technology-points-copy-{}",
            std::process::id(),
        ));
        let _ = std::fs::remove_dir_all(&temp);
        let player_guid = "4E239D4F000000000000000000000000";
        std::fs::create_dir_all(temp.join("Players")).expect("应创建测试角色目录");
        std::fs::copy(source.join("Level.sav"), temp.join("Level.sav"))
            .expect("应复制世界索引文件");
        std::fs::copy(
            source.join("Players").join(format!("{player_guid}.sav")),
            temp.join("Players").join(format!("{player_guid}.sav")),
        )
        .expect("应复制测试角色存档");

        let result = update_technology_points_in_dir(&temp, player_guid, 777, 31)
            .expect("应原子写入普通与古代科技点");
        assert!(result.ok && result.roundtrip_ok);

        let state =
            player_technology_points_in_dir(&temp, player_guid).expect("写回后应重新解析科技点");
        assert_eq!(state.technology_points, 777);
        assert_eq!(state.ancient_technology_points, 31);
        assert!(
            !temp
                .join("Players")
                .read_dir()
                .unwrap()
                .flatten()
                .any(|entry| { entry.file_name().to_string_lossy().contains(".txn-") }),
            "成功后不得遗留事务文件"
        );
        let _ = std::fs::remove_dir_all(&temp);
    }

    #[test]
    fn rejects_out_of_range_technology_points_before_reading_player_file() {
        let error =
            update_technology_points_in_dir(Path::new("C:/missing-world"), "valid-player", -1, 0)
                .expect_err("负数科技点必须被拒绝");
        assert_eq!(error, "科技点必须介于 0 和 9999999 之间");
    }
}
