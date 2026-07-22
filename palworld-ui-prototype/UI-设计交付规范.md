# Palworld 服务器管理器 · UI 设计交付规范

> **交付物类型**：高保真交互式原型 + 设计规格说明书（供专业编程团队开发集成）
> **原型文件**：`palworld-ui-prototype/index.html`（单文件、零构建，浏览器直接打开）
> **设计语言版本**：暖色磨砂玻璃 v2.4.0 + 幻兽帕鲁宠物风格
> **设计 Token 来源**：与线上 Vue 应用 `src/style.css` 的 `--palwarm-*` 体系 **完全一致**，可直接 1:1 落地为组件
> **窗口交互基线**：遵循《桌面窗口设计调研报告》结论——保留系统 caption 按钮（最小化/最大化/关闭），内容扩展到标题栏区域，不在透明分层窗上自绘 chrome

---

## 0. 如何使用本交付物

| 阅读对象 | 建议顺序 |
|---|---|
| 前端开发 | §2 视觉规范 → §3 页面层级 → §4 组件复用 → §6 交互流程 → §10 对接清单 |
| UI/视觉走查 | §2 视觉规范 + §7 宠物风格 + 打开 `index.html` 逐页核对 |
| 架构 / 技术负责人 | §8 窗口与系统交互（红线）+ §10 落地文件结构 |

**原型即真相来源**：所有尺寸、间距、圆角、配色、状态均已落在 `index.html` 的 CSS 与原生结构中，开发时以原型视觉为准，本规范作语义与流程补充。

---

## 1. 设计原则

1. **北极星**：温暖、安静而可靠的「苹果桌面控制台」+ 幻兽帕鲁的「宠物风格」——毛茸茸、圆润、有生命力，但不花哨。**宠物风格仅作视觉装饰语言，不新增与服务器管理无关的功能页（如帕鲁图鉴）。**
2. **交互正确性优先于视觉自由度**（调研结论）：系统窗口能力（拖拽 / 三按钮 / 缩放 / 阴影）永远可用，视觉定制在其之上做。
3. **一致性先于个性**：所有页面共用同一套 `--palwarm-*`，组件级复用率目标 ≥ 90%。
4. **状态显性化**：loading / empty / error / dirty 均有专门视觉，不靠文字解释。

---

## 2. 视觉规范（Design Tokens）

### 2.1 色彩
**品牌主色（暖珊瑚）**
| Token | 值 | 用途 |
|---|---|---|
| `--palwarm-primary` | `#e66f51` | 主按钮、激活态、强调 |
| `--palwarm-primary-deep` | `#c9543a` | 按下/深珊瑚、激活文字 |
| `--palwarm-primary-soft` | `rgba(230,111,81,.14)` | 激活背景、浅强调 |

**中性与背景**
| Token | 值 | 用途 |
|---|---|---|
| `--palwarm-background` | `#f5ede2` | 窗体底色（暖米） |
| `--palwarm-card` | `rgba(255,252,247,.72)` | 玻璃卡片 |
| `--palwarm-card-strong` | `rgba(255,250,244,.88)` | 强玻璃/弹层 |
| `--palwarm-foreground` | `#3f322c` | 主文字 |
| `--text-mid` / `--text-lo` | `#77675f` / `#a39383` | 次级 / 弱化文字 |

**语义状态**
| 状态 | 前景 | 背景 |
|---|---|---|
| success | `#4f8a6b` | `rgba(79,138,107,.14)` |
| warning | `#b8782f` | `rgba(184,120,47,.14)` |
| error | `#c9554d` | `rgba(201,85,77,.12)` |
| info | `#4b7896` | `rgba(75,120,150,.14)` |

**宠物元素色板（帕鲁属性）** —— 用于装饰 chip、吉祥物头像配色（仅视觉装饰，不承载图鉴功能）
| 元素 | 前景 | 背景 |
|---|---|---|
| 草 grass | `#7fae5a` | `rgba(127,174,90,.16)` |
| 火 fire | `#e8734a` | `rgba(232,115,74,.16)` |
| 水 water | `#5b9bd1` | `rgba(91,155,209,.16)` |
| 电 electric | `#e6b54a` | `rgba(230,181,74,.18)` |
| 冰 ice | `#7fc4cf` | `rgba(127,196,207,.18)` |
| 暗 dark | `#6b5b73` | `rgba(107,91,115,.16)` |
| 中性 neutral | `#b89a7a` | `rgba(184,154,122,.18)` |

> 对比度：正文 `#3f322c` 落在 `#f5ede2` 上对比度 ≈ 9.8:1，远超 WCAG AA 4.5:1；次要文字 `#77675f` 约 5.2:1，达标。

### 2.2 字体
- 中文 UI：`'Noto Sans SC','Microsoft YaHei',system-ui`（`--palwarm-font-sans`）
- 等宽（端口/IP/命令/数值）：`'JetBrains Mono',ui-monospace`（`--palwarm-font-mono`）
- 字阶：12 / 13 / 14 / 16 / 18 / 20 / 22(页标题) / 24(大数值)
- 字重：正文 400、标签/按钮 600、页标题 700

### 2.3 圆角 / 阴影 / 间距
- 圆角：`--r-input:10` `--r-btn:12` `--r-card:18` `--r-btn-lg:14` `--r-pill:999` `--palwarm-radius-panel:32`
- 阴影：`--warm-shadow:0 8px 28px -10px rgba(96,62,40,.28)`；主按钮 `0 8px 22px -6px rgba(230,111,81,.42)`
- 间距（4 基准）：`--space-1:4` `--space-2:8` `--space-3:12` `--space-4:16` `--space-6:24` `--space-8:32`
- 玻璃：`--glass-blur:blur(24px) saturate(145%)`

---

## 3. 页面层级关系（信息架构）

```
Palworld 服务器管理器（窗口）
└── 侧边导航 Sidebar
    ├── 概览 Overview              [/overview]   服务器健康仪表 + 在线玩家速览 + 近期事件
    ├── 玩家管理 Player Mgmt       [/players]   【新增页】在线玩家(RCON) + 玩家统计；宠物风格仅作装饰吉祥物点缀
    ├── 配置 Config               [/config]     PalWorldSettings.ini 图形化分组编辑
    ├── 网络 Network              [/network]    端口 / 虚拟局域网 / 防火墙
    ├── RCON 控制台 Rcon          [/rcon]       命令终端 + 快捷指令
    ├── 实时日志 Logs             [/logs]       滚动日志 + 级别过滤
    ├── 配置备份 Backup           [/backup]     备份历史 + 还原/删除
    └── 设置 Settings             [/settings]   外观 / 窗口行为 / 通知 / 更新
```

**当前线上状态对照**：`/overview /config /network /rcon` 已有实现；`/logs /backup /settings` 为 `PlaceholderView`（本轮补全）；**`/players` 为本次新增页**，承载 RCON 玩家管理（踢/封/私聊），宠物风格以装饰吉祥物点缀于页底，线上需在 `router/index.ts` 与 `views/PlayersView.vue` 落地。

---

## 4. 组件库与复用说明

### 4.1 基础组件（全局复用）
| 组件 | 原型 class | 对应线上 Vue 组件 | 复用点 |
|---|---|---|---|
| 主/次/成功/危险按钮 | `.btn .btn-primary/.btn-ghost/.btn-success/.btn-danger` | `ui/BaseButton.vue` | 全页 CTA |
| 玻璃卡片 | `.card` | `ui/GlassPanel.vue` | 所有区块容器 |
| 文本/下拉输入 | `.input` `.select` | （待抽） | 搜索、配置值、设置项 |
| 开关 Toggle | `.toggle` | `ui/`（CfgItem 内） | 设置、配置开关项 |
| 状态药丸 | `.status-pill.ok/off/block/info` | `ui/StatusPill.vue` | 端口、玩家、运行态 |
| 元素 chip | `.chip.grass/fire/...` | （新增 `ui/ElementChip.vue`） | 装饰标签、吉祥物元素标记 |
| 表格 | `.table` | `components/server/`（PlayerTable） | 玩家、备份 |
| 终端 | `.terminal` | `ui/Terminal.vue` | RCON、日志 |
| 模态确认 | `.modal-mask .modal` | `ui/ConfirmDialog.vue` | 踢/封/删二次确认 |
| Toast | `.toast-host .toast` | `ui/Toast.vue` + `stores/toast.ts` | 全页反馈 |
| Tooltip | `.app-tooltip` + `[data-tip]` | `ui/Tooltip.vue` | 术语解释 ⓘ |
| 骨架屏 | `.skeleton` | （新增） | 加载态 |
| 空态 | `.empty` | （新增） | 无数据 |

### 4.2 业务组件（本项目专属）
| 组件 | 原型结构 | 说明 |
|---|---|---|
| 帕鲁头像 `PalAvatar` | `.pal-avatar[data-pal]` + `palAvatar(type)` 生成 SVG | **宠物风格核心**：7 种元素配色 + 爪耳/角/表情，可 `bob` 浮动 |
| 帕鲁卡片 `PalCard` | `.pal-card`（头像+名+等级+元素chip+亲密度条） | ⚠️ 已弃用：帕鲁图鉴页已移除；该组件暂不落地（若未来做「按玩家查看其帕鲁」存档解析功能可复用） |
| 信息行 `Companion` | `.companion`（头像+名+状态） | 概览在线玩家行 / 通用信息行（装饰性帕鲁头像可作吉祥物） |
| 配置组 `CfgGroup` | `.cfg-group`（可折叠头 + 2 列 body） | 配置页参数分类 |
| 配置项 `CfgItem` | `.cfg-item`（标签 + 值/开关，dirty 高亮） | 单行参数，含「已改动」态 |
| 端口卡 `PortCard` | `.port-card`（图标+标题+状态药丸+协议/描述） | 网络页 |
| 实时条 `LiveBar` | `.livebar`（运行态药丸 + 版本/uptime/TPS/IP） | 概览顶部状态条 |

> **复用纪律**：元素 chip、帕鲁头像、配置组三类被多页共用（概览/玩家/配置/网络/设置），必须抽到 `ui/` 下独立组件，禁止页面内重复实现。

---

## 5. 导航与页面流转逻辑

- **主导航**：侧边栏 8 项，点击切换 `.screen`（线上即 `router-link` + `active-class`）。激活态 = 左侧 3px 珊瑚描边 + 浅珊瑚底。
- **跨页关键流转**：
  1. 概览「在线玩家」→「查看全部玩家」按钮 `goNav('players')` → 玩家管理页。
  2. 概览「广播 /Broadcast」→ 走 RCON 广播（与 RCON 页共用 `toast` 反馈）。
  3. 配置「应用并写盘」→ 自动触发备份（与配置备份页同源）→ Toast「已自动备份 #043」。
  4. RCON「玩家管理」语义入口 → 玩家管理页（踢/封操作集中在玩家管理页，避免 RCON 与玩家管理两处分裂）。
  5. 设置「标题栏 / 毛玻璃材质」选项 → 直连 §8 窗口交互红线，切换即提示系统材质策略。

---

## 6. 关键交互流程（含状态）

### 6.1 踢出 / 封禁玩家（二次确认）
```
玩家表格行「踢出」→ 打开 ConfirmDialog（标题=踢出玩家，文案=立即生效）
  → 取消：关闭，无副作用
  → 确认：toast('success','已踢出 X') + 刷新玩家列表（rconStore.kickPlayer）
```
> 封禁同构，文案改为「不可再次加入」。**必须二次确认**（SRS M4-F5 验收）。

### 6.2 配置修改 → 写盘 → 自动备份
```
配置项 contenteditable / Toggle 变更 → 该项进入 .dirty（珊瑚描边+环）
  → 点「应用并写盘」→ 写 PalWorldSettings.ini + 生成备份（最多 20 份）
  → Toast 成功 + 清除 dirty 标记
  → 未保存时「撤销改动」清空所有 dirty
```

### 6.3 配置备份还原 / 删除
```
备份历史行 → 还原：ConfirmDialog → toast 成功 → 触发配置重载
          → 删除：ConfirmDialog（不可撤销）→ toast → 列表移除
          → 对比：打开差异视图（待实现，原型用 toast 占位）
```

### 6.4 实时日志过滤
```
顶部 Tab（全部/信息/警告/错误）→ 按 lv 过滤 t-body → 跟随滚动
暂停：停止自动滚动（原型用 toast 占位）
```

### 6.5 全局反馈
- **Toast**：右下浮入，2.6s 自动消；success/error/info 三色左边框。
- **Tooltip**：`[data-tip]` 元素 hover 显示暖色玻璃浮层（术语解释，如「什么是帕鲁动画」「为什么保留系统标题栏」）。

---

## 7. 宠物风格（帕鲁）视觉语言

- **生物头像 `palAvatar(type)`**：纯 SVG 程序化生成，统一圆润身体 + 大眼高光 + 腮红 + 表情弧 + 顶部元素符号（草=叶、火=焰、水=滴、电=闪、冰=雪花、暗=月、中性=爪）。7 种元素配色见 §2.1。
- **动效**：`.bob` 让头像上下轻浮（2.4s 缓动），呼应「有生命力的伙伴」；遵循 `prefers-reduced-motion` 时停用。
- **使用场景**：装饰性吉祥物（玩家管理页底部）、标题栏应用图标、设置「界面动效」开关联动。**注意：宠物风格仅作视觉装饰，不新增图鉴/物种档案类功能页——服务器管理器不维护帕鲁图鉴。**
- **设计约束**：不引入写实照片或版权素材；全部为矢量、可主题化、零外部依赖。

---

## 8. 窗口与系统交互（红线 · 来自调研结论）

> 以下为 **不可妥协** 的窗口实现约束，开发落地 `TitleBar` / `main.rs` 时必须遵守：

1. **保留系统 caption 按钮**：最小化/最大化/关闭由系统绘制并保留命中；自定义标题栏仅做「内容扩展」（`ExtendsContentIntoTitleBar` + `SetTitleBar`），并为按钮区预留 `RightInset` 空间，避免内容盖住三按钮。
2. **禁止 `decorations:false` 彻底无边框自绘**：调研已判定其为「负优化」（拖拽与三按钮热区从源头消失）。
3. **材质用系统原生**：主窗优先 `window-vibrancy` 的 `apply_mica`（Win11）；避免 `transparent:true` 分层窗（丢原生阴影 / Aero Snap / 缩放）。
4. **拖拽区必须显式标注**：若仍用无边框方案，元素须 `data-tauri-drag-region`（Tauri）或 `-webkit-app-region:drag`（Electron），且按钮必须排除出拖拽区（`no-drag`），否则点击被吞。
5. **Tauri v2 权限**：拖拽需 capability 显式授权 `core:window:allow-start-dragging`，否则 `startDragging()` 静默失败。
6. **macOS**：保留左上角 traffic lights，不隐藏/不替换；可 `apply_liquid_glass`（macOS 26+）。

原型中的标题栏已按此呈现：左侧 `.drag-zone`（可拖拽区）、右侧 `.win-controls`（系统保留区，仅作视觉示意，真实由系统接管）。

---

## 9. 无障碍与适配

- **对比度**：正文 ≥ 4.5:1，大文字 ≥ 3:1（见 §2.1 实测）。
- **焦点**：所有可交互元素 `:focus-visible` 2px 珊瑚描边（按钮已含）。
- **触摸目标**：按钮/开关最小 36–40px 高，达标 44px 目标（含留白）。
- **文本缩放**：布局用 `rem`/`flex`，支持浏览器 200% 缩放不破版。
- **动效敏感度**：`.bob` 等动画在 `prefers-reduced-motion: reduce` 下停播。
- **语义**：真实落地时用 `<aside>/<nav>/<main>/<section>` + `aria-*`；原型为视觉走查用 `div` 结构。

---

## 10. 开发对接清单（落地到 Vue 组件）

**新增 / 待实现页面**（线上目前为 `PlaceholderView` 或缺页）：
- [ ] `views/PlayersView.vue` —— 玩家管理页（玩家表来自 `rconStore.players`；页底装饰吉祥物用 `PalAvatar` 组件，纯装饰不参与统计）
- [ ] `views/LogsView.vue` —— 实时日志（接 `LogPanel.vue` 思路 + 级别过滤）
- [ ] `views/BackupView.vue` —— 备份历史（接 `list_config_backups`）
- [ ] `views/SettingsView.vue` —— 设置（含 §8 窗口行为选项）
- [ ] `router/index.ts` 增加 `/players` 路由，移除对应 `PlaceholderView` 占位

**新增组件建议**：
- [ ] `ui/PalAvatar.vue`（接 `palAvatar(type)` SVG 生成逻辑，当前用作装饰吉祥物）
- [ ] `ui/ElementChip.vue`（7 元素 chip，用于装饰标签）
- [ ] `ui/Companion.vue`（通用信息行，概览在线玩家复用）
- [ ] `ui/Skeleton.vue` / `ui/EmptyState.vue`

**Token 同步**：原型 CSS 顶部 `--palwarm-*` 与 `src/style.css` 已对齐；宠物扩展色板（§2.1 元素色）需补进 `style.css` 的 `--pal-*` 区块。

**验证门禁**：每个新页达到——视觉与原型一致（走查）、键盘可达、Toast/Modal/空态齐备、窗口交互遵守 §8。

---

*— UI Designer（像素君）· 2026-07-21 · 原型与规范均基于现有 `--palwarm-*` 体系与《桌面窗口设计调研报告》结论，未改动任何业务代码。*
