# 桌面级 App 窗口设计调研（2026-07-21）

> 背景：Palworld 服务器管理工具（Tauri 2 + Vue 3）窗口重做。上一轮把窗口改成
> "透明全窗 + CSS `backdrop-filter` 整窗玻璃"，用户实测为**负优化**
> （无阴影、拖不动边缘、不像原生）。本文联网调研结论，确定正确方向，**本轮不写代码**。

---

## 一、结论先行

1. **当前方案是负优化，根因在 OS 层，不在 CSS。**
   在 Windows 上把窗口设为 `transparent: true`，窗口会变成
   `WS_EX_LAYERED` 分层窗口。DWM（桌面窗口管理器）会**跳过现代合成器**，
   导致三件事同时失效：**DWM 阴影消失、原生边缘缩放（WS_THICKFRAME）失效、
   OS 无法把窗口圆角化**。这正好对应你看到的"贴纸感 + 拖不动边 + 不像原生"。

2. **CSS `backdrop-filter` 做不出真正的系统毛玻璃。**
   纯 CSS 模糊只作用于 WebView 内容层，**无法穿透到原生窗口管理器**
   （WebView2 限制），不能影响标题栏和窗口背景，只能作降级方案。
   真正的系统毛玻璃必须调 OS 原生接口（Windows 的 DWM）。

3. **正确方向：用 OS 原生材质承载"磨砂玻璃"，关闭系统标题栏但保留
   DWM 的圆角/阴影/缩放，再用自定义标题栏承载品牌。**
   具体通过 Tauri 的 `window-vibrancy` crate（`apply_mica` / `apply_acrylic`）
   调 DWM API 实现。CSS 玻璃降级为"面板级"点缀，不再用来撑整窗。

---

## 二、业界规范（Microsoft / Apple 官方）

**Microsoft — Windows 11 应用最佳实践（官方文档）：**
- **圆角是自动的——只要你"让系统画边框和阴影"。** 官方明确警告：
  "避免自定义窗口边框和阴影，这可能导致系统无法将窗口角落圆角化。"
- **材质分工**：`Mica` 用于**长期存在的表面**（标题栏、侧边栏），高性能、
  透出桌面壁纸、失焦时回退纯色；`Acrylic` 用于**临时表面**（右键菜单、
  浮层、弹窗）。两者都是系统级合成材质，不是 CSS。
- 避免纯黑纯白，用更柔和的色调（与我们的暖色方向一致）。
- 要支持 Snap Layouts（Win+Z）、深色/浅色主题。

**Apple — 2025 Liquid Glass（iOS 26 / macOS Tahoe）：**
- 半透明材质**反射并折射环境**，实时渲染高光，随明暗自适应。
- 控件 / 工具栏 / 侧边栏由该材质构成，作为**浮于内容之上的独立功能层**，
  并"折射背后内容"以保持环境感。这正是"温暖磨砂玻璃"想要的氛围，
  但它靠 OS 原生渲染，不是 CSS。

---

## 三、负优化的根因（技术确证）

CSDN《Win11 窗口 API 如何实现圆角、毛玻璃及 Mica 材质效果》一文
（技术深度高，与本次问题一一对应）指出 `SetWindowRgn` / 透明分层窗口
（`WS_EX_LAYERED`）有**三大不可逆副作用**：

| 副作用 | 技术后果 | 对应你看到的现象 |
|--------|----------|------------------|
| 强制启用 `WS_EX_LAYERED` | DWM 跳过现代合成器，Mica 渲染器被绕过，仅剩 fallback | 玻璃质感不对、无系统材质 |
| 破坏窗口边界拓扑 | DWM 无法计算阴影偏移与模糊采样范围 → **阴影消失**；Acrylic 采样源变成桌面而非前台窗口 | **"贴纸感"（无阴影）** |
| 禁用标题栏命中测试 | 系统无法识别拖拽区 → 原生拖拽动画、Snap Layouts、焦点高亮失效；`WS_THICKFRAME` 未设 → 无缩放边 | **拖不动边缘缩放**、不像原生 |

> Mica 合规清单：**必须保留 `WS_THICKFRAME`**（原生缩放的前提），
> **不能用 `SetWindowRgn`**，并设置 `DWMWA_WINDOW_CORNER_PREFERENCE`。
> 裸透明窗既丢了 `WS_THICKFRAME` 又丢了 DWM 圆角/阴影——正是我们踩的坑。

---

## 四、Tauri 社区的正确做法

1. **关闭装饰 + 透明窗 + 自定义标题栏**是通用起点；但透明窗在 Windows 上
   `data-tauri-drag-region` 命中测试不稳，**需 `startDragging()` 程序化兜底**
   （拖动用程序化 API 是对的，这一步没做错）。
2. **缩放是已知硬骨头**：Tauri 至今**没有** `data-tauri-drag-resize-region`
   属性（issue #7900）。需自己在四边/四角 div 上监听 `mousedown` 调
   `start_resize_dragging`（仅 v2 有）。Tauri 自身源码 `undecorated_resizing.rs`
   就是用子 HWND 做命中测试，给无装饰窗补回缩放。
3. **`window-vibrancy` crate（正确的系统玻璃方案）**：
   - `apply_mica(&window)` — Windows 11 原生 Mica（透出壁纸、高性能、失焦回退纯色）
   - `apply_acrylic(&window)` — Windows 10/11 Acrylic（模糊窗口背后的内容）
   - `apply_blur(&window)` — 简单模糊
   - 这些**直接调 DWM API**，得到"穿透窗口背景的真实系统毛玻璃"，
     且**不像裸透明窗那样杀死阴影与缩放**（因为它走 DWM API 而非 `SetWindowRgn`）。
   - 注意：用 vibrancy 时需 `html, body { background: transparent }`；
     弱机有 GPU 开销，Win11 某些版本在缩放/拖拽时 acrylic 有重绘代价，需不透明降级。

---

## 五、两条正确路径（等你拍板，现在不写代码）

**路径 A — OS 原生材质（推荐）**
- `decorations: false` + `window-vibrancy`（`apply_mica` 给侧栏/标题栏，
  `apply_acrylic` 给弹层）→ 自动获得 **DWM 圆角 + 原生阴影 + 原生缩放 + 真实玻璃**；
- 自定义标题栏承载品牌（珊瑚暖橙）；CSS 模糊仅用于内部面板；
- 代价：弱机 GPU 开销、acrylic 缩放/拖拽重绘代价、需不透明降级。

**路径 B — 全自定义 chrome（控制力最强、最脆）**
- 透明窗 + 手动补：DWM 圆角（`DwmSetWindowAttribute` `DWMWA_WINDOW_CORNER_PREFERENCE`）
  + `set_shadow(true)` + 四边/四角 `start_resize_dragging`；
- 工作量最大、跨平台最易碎，但视觉完全可控。

---

## 六、Glassmorphism 可用性要点（通用）

- 玻璃**后面必须有东西**（深度/分层），否则退化为半透明盒子。
- 模糊要配 **10–30% 不透明实色叠层**保证文字对比（我们的暖色面板/文字需确保这点）。
- 模糊半径通常 10–30px；背景越杂越要加模糊。

---

## 七、下一步（等你选 A 或 B，确定后我再动手）

1. 还原本轮被改坏的圆角/阴影/缩放相关的临时代码；
2. 接 `window-vibrancy`（路径 A）或补 DWM 圆角+阴影+缩放（路径 B）；
3. 把"整窗 CSS 玻璃"降级为"面板级玻璃"；
4. 标题栏拖拽用 `data-tauri-drag-region` + `startDragging()` 兜底，
   四边/四角做缩放区。

> 我推荐 **路径 A**：直接对应微软"Mica 用于长期表面（侧栏/标题栏）"的官方指引，
> 用最小风险拿到原生质感 + 真实毛玻璃 + 阴影 + 缩放，且正好契合
> "温暖磨砂玻璃的桌面级 app" 诉求。
