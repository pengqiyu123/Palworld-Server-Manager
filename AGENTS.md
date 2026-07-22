# AGENTS.md

## 项目简介

这是一个幻兽帕鲁专用服务器管理器，提供服务器启停、配置编辑、网络与防火墙检查、RCON 控制和故障排查功能，并通过 Tauri 打包为 Windows 桌面应用。

## 技术栈

- 前端：Vue 3、TypeScript、Pinia、Vite
- 桌面端：Tauri 2、Rust
- 包管理器：npm

## 常用命令

在 `Palworld` 目录执行：

- `npm run dev`：启动前端开发服务
- `npm run build`：执行类型检查并构建前端
- `npm run preview`：预览前端构建结果
- `npm run tauri:dev`：启动桌面应用开发模式
- `npm run tauri:build`：构建桌面应用

## 代码规范

- Vue 组件使用 `<script setup lang="ts">` 与 Composition API。
- TypeScript 开启严格模式，避免未使用的变量和参数。
- 使用 2 空格缩进、单引号，语句末尾不强制分号。
- 组件文件使用 PascalCase，变量和函数使用 camelCase。
- 通用视觉样式集中维护在 `Palworld/src/style.css`，业务状态保留在 Pinia store 中。
- 修改后至少运行 `npm run build`，确保类型检查与生产构建通过。
