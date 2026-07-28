# 给混元的项目认知纠正

以下内容用于纠正 `F:\study\.workbuddy\memory` 中已经过时的项目结论，不是任务说明、实施计划或后续开发指令。

```text
Palworld Server Manager 的 V2/F5 三阶段迁移、SaveGames/_backups 与
“每次整世界快照”均为历史排查记录，不是当前产品方案。

当前生产方案为 V4，代码位于 Palworld/src-tauri/src/backup_service.rs 和
Palworld/src-tauri/src/save_edit/v4_*.rs，前端位于 Palworld/src/views/
SaveMigrationView.vue 与 SaveManagementView.vue。

已实现并验证的事实：
1. 备份根目录：开发模式为 Palworld/backups，发布模式为管理器 exe 同级 backups；
   用户主动设置后才使用自定义目录，绝不静默回退 LocalAppData。
2. 目录语义固定为 local（本地完整备份）、server（服务器完整备份）、
   snapshots（操作回滚点）和 _system（索引、工作流）。完整备份手动删除；
   每世界只自动保留最近 3 个已完成回滚点，写入中或需恢复的回滚点不自动删除。
3. 世界迁移只修改目标服务器世界：先建立 server 完整备份，再暂存、校验、原子替换；
   本地源不重复自动备份，保留目标服务器规则。
4. 完整角色转移与恢复原公会已经拆分：角色转移保留目标角色身份，默认不改变公会；
   公会恢复只可读取同一世界迁移工作流记录的原单机主角色，不允许前端任意指定公会。
5. 独立“朋友角色导入”没有公会恢复能力。应用重启只恢复工作流状态，绝不自动执行后续写操作。
6. 所有 V4 写操作都要求服务器停止、先建立备份或回滚点、候选文件回读解析、
   同目录 tmp + sync + rename 提交；中断或失败按工作流恢复。
7. 已运行验证：Rust 主测试 126 passed/0 failed/1 ignored，角色迁移集成测试
   7 passed/0 failed；前端 Vitest 59 passed；vue-tsc 与 Vite production build 均通过。
   F:\1 原始存档未写入，真实存档测试均针对副本。

当前边界：背包和帕鲁的独立编辑尚未开放；旧 F5 的随机 GUID 角色转移与 V2 双向
绑定均不是当前迁移实现。
```

核验入口：`Palworld/src-tauri/src/backup_service.rs`、`Palworld/src-tauri/src/save_edit/v4_migration.rs`、`Palworld/src-tauri/src/save_edit/v4_character_operation.rs`、`Palworld/src-tauri/src/save_edit/v4_guild_recovery.rs`、`Palworld/src-tauri/tests/v4_full_character_transfer.rs`。
