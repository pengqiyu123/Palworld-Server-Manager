# Palworld Server Manager

一款基于 **Tauri 2 + Vue 3** 的 Palworld 专用服务器桌面管理器，目标是一站式搞定：启动/停止 PalServer、Radmin VPN 联机引导、启动游戏本体、存档与世界备份/恢复、以及存档数据修改等能力。

## 仓库结构

```
Palworld-Server-Manager/        # 本仓库根
├── Palworld/                    # 主应用（Tauri2 + Vue3）
│   ├── src/                     # 前端源码（Vue3 + Vite）
│   ├── src-tauri/               # Rust 后端（Tauri 命令、Rcon、存档管理）
│   ├── package.json
│   └── .env.example             # 环境变量示例（实际 .env 不入库）
├── docs/                        # 需求 / 设计 / 调研文档
├── palworld-ui-prototype/       # 早期 UI 原型参考
├── palworld-warm-glass-preview/ # 暖色玻璃风格预览
└── reference-projects/          # 第三方参考项目（本地保留，不入库）
```

> ⚠️ `reference-projects/` 是第三方开源项目的本地克隆（如 PalworldSaveTools、zaigie 的服务器工具等），体积大且非本仓库代码，**已被 .gitignore 排除，不会上传**。

## 快速开始

```bash
cd Palworld
cp .env.example .env          # 按需修改
npm install
npm run tauri dev             # 启动开发模式（会弹出桌面窗口）
```

生产构建：

```bash
npm run tauri build
```

## 功能进度

- ✅ 启动/停止 PalServer
- ✅ 启动 Radmin VPN + 联机加入引导弹窗
- ✅ 启动游戏本体（steam://rungame/1623730 优先，exe 兜底）
- ✅ 世界存档发现 / 整包备份 / 恢复（P0）
- 🚧 角色跨服导出/导入、本地存档→服务器存档转换、数据修改（规划/调研中）

详见 `docs/`。
