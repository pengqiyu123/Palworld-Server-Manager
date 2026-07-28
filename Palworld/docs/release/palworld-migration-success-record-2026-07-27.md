# Palworld 存档迁移成功记录与后续设计决策

> V4 重构已完成，最新实现与验证记录见 `palworld-migration-v4-implementation-record-2026-07-27.md`。本文保留此前 Fix Host 实机成功的历史证据；其中“下一版”描述已由 V4 实现取代。

**状态**：2026-07-27 实机验收通过。本文是当前迁移功能的权威记录；旧交接文档中与本文冲突的结论均已失效。

## 1. 已成功交付的范围

本次完成并经真人进服验证的是“单机世界迁入专用服 + 原主机身份接管”：

1. 整包迁移本地世界到专用服目标世界。
2. 原账号进入服务器创建占位角色并退出。
3. 停服后执行 Fix Host 身份交换，同时重绑原公会成员、管理员、建筑和帕鲁所有权。

用户使用 Steam 账号 `76561199381352956` 登录后，服务器识别 Player ID `4E239D4F000000000000000000000000`。角色、公会权限、物品、帕鲁、据点和建筑均正常，游戏再次读档、保存后未清理数据。

这次成功不等于“通用跨世界角色转移”已经完成。当前已验证能力应命名为 **主机身份接管（Fix Host）**。

本次实测环境：

- 游戏版本：`v1.0.1.100619`
- 本地世界：`%LocalAppData%/Pal/Saved/SaveGames/76561199381352956/1D5D1F304D3AA1FE2818BA98D5223DFE`
- 服务器世界：`E:/SteamLibrary/steamapps/common/PalServer/Pal/Saved/SaveGames/0/1A91A61548C7B6FD7B58B2B70710F7EE`
- 原主机 UID：`00000000000000000000000000000001`
- 服务器 UID：`4E239D4F000000000000000000000000`

## 2. 验证证据

迁移前自动备份与修复候选均由本项目解析器和参考项目 `palsav` 独立解析。正式写入前后得到以下不变量：

| 数据 | 迁移前 | 修复候选/正式档 |
|---|---:|---:|
| CharacterSaveParameterMap | 389 | 389 |
| 玩家 | 2 | 2 |
| 帕鲁 | 387 | 387 |
| ItemContainerSaveData | 436 | 436 |
| CharacterContainerSaveData | 6 | 6 |
| GroupSaveDataMap | 6 | 6 |
| BaseCampSaveData | 2 | 2 |
| 归属原主机的帕鲁 | 363 | 0 |
| 归属服务器 UID 的帕鲁 | 0 | 363 |

正式世界与候选目录共 92 个文件，逐文件 SHA-256 差异为 0。Rust 全量回归结果为 `87 passed / 0 failed / 1 ignored`。最后由用户实际进入服务器完成业务验收。

## 3. 事故根因与正确算法

失败版本全局交换了 CSPM key 中的 `PlayerUId`，把 387 个帕鲁条目的稳定键也改成服务器 UID。Palbox/CharacterContainer 仍引用原键，游戏加载后判定引用失效，将 CSPM 从 389 条清理到 2 条，并连带丢失公会物品和帕鲁。

正确规则与 `reference-projects/PalworldSaveTools-main/.../fix_host_save.py` 一致：

- 只按两名玩家的 `InstanceId` 交换两条玩家 CSPM key。
- 帕鲁 CSPM key 保持原值；只改 RawData 的 `OwnerPlayerUId`。
- 只交换明确的所有权字段：`OwnerPlayerUId`、`owner_player_uid`、`build_player_uid`、`private_lock_player_uid`。
- 结构化修改公会 RawData 的管理员、成员、character handle 和 marker owner。
- 同步修改两份玩家文件、DPS 所有者和 PalStorageContainerId，最后交换玩家文件名。
- 任一核心文件无法完整解析时停止，不做裸字节降级。

关键实现位于：

- `Palworld/src-tauri/src/save_edit/fix_host.rs`
- `Palworld/src-tauri/src/save_edit/sav_io.rs`
- `Palworld/src-tauri/src/save_edit.rs`

## 4. 与同行产品的功能边界

参考项目包含两类不同功能，不能混为一个“角色迁移”：

### 4.1 Fix Host：同世界身份交换

`fix_host_save.py::fix_save` 同时交换两名玩家身份，并可通过 `guild_fix` 修改公会关系。本项目本次实机成功的算法属于这一类，适合“单机主机世界迁服后，原账号接管自己的角色和公会”。

### 4.2 Character Transfer：跨世界角色数据复制

参考工作流先由 `transfer_tech_and_data` 复制角色数据并保留目标角色的 UID、InstanceId、GroupId、PalStorageContainerId 和背包容器身份，再由 `transfer_character_only` 用目标 UID/InstanceId 替换目标 CSPM 内容并复制所需容器和 DPS。`transfer_guild` 是另一项显式操作。

这才适合“把同学的角色加入我的世界”：默认只把角色数据写入同学在目标服创建的角色，**不能自动把我的公会或会长身份交给同学**。

当前 `Palworld/src-tauri/src/save_edit/transfer.rs` 仍按旧的 ArrayProperty/随机 GUID/尽力而为写回设计，和真实 CSPM MapProperty 及参考算法不一致，尚未达到生产可用标准，不得宣称通用角色转移已完成。

## 5. 下一版产品流程

UI 和后端应拆为三个独立、可验证的命令：

1. **迁移世界**：源世界 -> 目标专用服；只做世界数据层复制。
2. **转移角色**：源 Level 角色 -> 目标 Level 已创建角色；默认保留目标 UID、容器身份和目标公会，不改管理员。
3. **绑定公会权限**：可选操作。用户明确选择目标公会及角色权限；“设为会长/管理员”必须二次确认。

另提供一个明确命名的快捷预设 **接管原单机主机**，内部执行已经验证的 Fix Host + 公会接管。它不能用于普通成员导入。

当前代码在 `save_edit.rs` 中强制 `run_phase_b == run_phase_c`，前端按钮也写成“绑定角色与公会”。这是已确认的产品边界问题，下一轮应先写失败测试，再拆接口；本次文档记录不代表拆分已经实现。

## 6. 备份与快照决策

用户提出“不在三个步骤中反复创建完整快照”的方向正确，但仅检查“目录里有某个备份”不够安全：旧备份可能早于最新登录或服务器保存，无法恢复本次操作前状态。

采用两层保护：

### 6.1 用户可见的完整备份

- 迁移前必须配置持久化 `backup_root`，路径存在、可写，且不能位于源/目标世界内部。
- 没有配置路径：阻止迁移，提供“前往备份设置”。
- 已配置但没有当前目标世界的有效备份：弹窗提供“立即备份并继续”或取消。
- 已有与当前目标世界内容指纹一致的备份：复用，不重复复制。
- 只有“存在且匹配当前状态”的备份才算有效，不能只按目录名称判断。

### 6.2 内部轻量事务备份

每个写盘操作仍需失败回滚，但不再复制整个 `SaveGames/0`：

| 操作 | 事务备份范围 |
|---|---|
| 世界迁移 | 目标世界数据目录（完整备份或可复用指纹备份） |
| 角色转移 | `Level.sav`、目标玩家 `.sav`、目标 `_dps.sav`，并记录原先不存在的文件 |
| 公会绑定 | `Level.sav`、目标玩家 `.sav` |
| Fix Host | `Level.sav`、两份玩家 `.sav`、两份 `_dps.sav` |

事务备份在结构校验和用户实机确认前保留；确认成功后仅保留最近一次或按容量上限清理。这样既保留原子回滚，也避免每一步复制全部世界和其他服务器世界。

现状需修正：Phase A 已支持 `backup_root` 和相同内容复用；v2 B/C 仍固定把整个 `SaveGames/0` 写入 `_migration_backups`，未使用用户配置的备份路径。

## 7. 成功经验

1. **先定义操作语义**：Fix Host、角色复制、公会绑定是不同产品，不应共用一个含糊的“迁移”按钮。
2. **以 InstanceId 定位玩家**：UID 会迁移，InstanceId 才能区分玩家记录与帕鲁稳定引用。
3. **结构数量是强验收项**：只验证角色能登录不够；CSPM、物品容器、帕鲁容器、公会和据点数量必须迁移前后对拍。
4. **游戏实机保存是最终门槛**：解析器可读、单测通过仍不足以证明游戏不会清理引用；必须进服、退出并再次解析。
5. **参考实现用于行为校准**：先读 `fix_host_save.py` 和 `character_transfer.py` 的最终状态，再独立实现并用参考解析器对拍。
6. **测试必须覆盖旧错误**：回归测试明确断言帕鲁 CSPM key 不变、OwnerPlayerUId 改变、玩家 key 交换、总条目数不变。
7. **真实档只通过 staging 原子替换**：候选先在临时副本构建和解析，验证后再将旧 live 目录移入备份并原子换位。

## 8. 参考与许可证边界

- `PalworldSaveTools-main/LICENSE`：项目外层工具代码为 MIT。
- `PalworldSaveTools-main/src/palsav/LICENSE`：解析库为 GPL-3.0。

本项目可读取参考实现、记录文件格式和做结果对拍；如保持当前许可证，不应直接复制或链接 `src/palsav` 的 GPL 实现。底层格式应独立实现，并保留测试证据与来源说明。
