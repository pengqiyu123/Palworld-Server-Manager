# Palworld Server Manager

面向 Windows 10/11 的 Palworld 专用服务器桌面管理器，基于 Tauri 2、Vue 3 和 Rust。它把服务器安装后的常用操作、朋友联机、配置、实时状态、备份和存档迁移集中到一个界面中。

## 已实现功能

- 识别并管理由应用内或外部启动的 PalServer，支持 REST 优雅关服与强制停止。
- 启动 Radmin VPN 和 Palworld，显示真实在线人数、FPS、版本及玩家列表。
- 通过本机 REST 管理接口查看信息、保存世界、广播和安排关服。
- 编辑 `PalWorldSettings.ini`，提供休闲、正常、挑战三档预设；保存后由用户重启服务器生效。
- 发现本地单机与专用服世界，创建完整备份、操作回滚点并查看角色/公会详情。
- 按“世界迁移 → 创建目标角色 → 完整角色转移 → 可选恢复原公会”完成单机转专用服。
- 在停服且游戏关闭时重命名玩家/公会、调整等级和科技点、解锁科技、设置会长或执行级联删除。

所有存档写入均要求服务器和游戏关闭，并先建立备份或回滚点。世界迁移不会复制来源 `WorldOption.sav`，服务器规则继续由配置页管理。

## 开发

进入 `Palworld/` 后执行：

```powershell
npm install
npm test
npm run lint
npm run build
npm run tauri:dev
```

Rust 验证：

```powershell
cd src-tauri
$env:CARGO_TARGET_DIR = 'C:\codex-target\palworld-manager'
cargo test --all-targets
```

项目位于 exFAT 盘时，建议把 Cargo target 放在 NTFS 盘，避免文件锁错误。正式安装包使用 `npm run tauri:build` 构建。

## 仓库结构

- `Palworld/src/`：Vue 前端、Pinia 状态和 Tauri 类型化调用。
- `Palworld/src-tauri/src/`：服务器、REST、备份、迁移与修改器后端。
- `Palworld/tests/`、`Palworld/src-tauri/tests/`：前端和 Rust 集成测试。
- `docs/`：产品、架构、存档研究、发布记录及历史归档；从 `docs/README.md` 开始阅读。
- `reference-projects/`：本地研究资料，已忽略且不会上传。

## 许可证

本项目以 [GPL-3.0-or-later](LICENSE) 发布。公会存档编解码改编自 PalworldSaveTools 的 GPL 子包；经验与科技数据表遵循上游 MIT 条款。详情见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。
