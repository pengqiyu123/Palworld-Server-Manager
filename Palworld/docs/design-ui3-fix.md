# UI3 概览修正 + 空配置首启引导 — 紧设计

> 范围：仅 `src/views/OverviewView.vue`（T1/T2 均在此文件）。基于实读全文（866 行）出设计，行号以实读为准。
> 铁律：只产出设计文档，不改源码。不动 `save_edit` 测试模块、`config.rs`、`stores/*`。不提交 git、不跑 dev server。

---

## 0. 读码结论

| 项 | 现状（实读） | 判定 |
|---|---|---|
| `.launch-zone` 块 | :13-106，**常驻**（不进 v-if/v-else），仪表盘/向导两态都显示 → 与向导 `s1-wrap`(:182-242) 步骤冗余 | **整块删** |
| 仪表盘模式 | :109-179 `v-if isDashboard`，含指标卡(:134-159)+进程行(:162-167)+7步引导(:170-178)，**无任何关服按钮** | 指标/引导保留不变；进程行补关服（见 §3 判断） |
| 向导模式 `s1-wrap` | :182-242 `v-else`，含 step-badge/s1-hero SVG/s1-title/s1-sub/steps-row(2×StepCard)/cta-row(onDetect+onManual)/tip-row | 保留框架，**重构 steps-row+cta-row → StepCard1(三路径)+StepCard2(三启动)** |
| `StepCard.vue` | 纯展示，props={num,title,desc,state}，**无 slot** | 当标题卡用，操作内容放下方自定义区 |
| 关服入口 | `onGracefulShutdown`/`onForceStop` **唯一 UI 触发点 = `.launch-zone:73-74`**；Sidebar/仪表盘均无 | 删 launch-zone 后运行态(仪表盘)失关服入口 → **必须补**（见 §3 设计判断） |
| `onMounted` | :293-299，`onSuccess`回调 + `void checkConfigInitialized()` + `void detectPaths()` | checkConfigInitialized 改 await，末尾接入首启跳转 |
| `checkConfigInitialized` | :529-539，`!serverPathDetected` 时 early return(false)；否则 `api.config.isInitialized` | 末尾加首启跳转判断 |
| `isConfigInitialized` | ref(:528) | 复用 |
| `config-banner` | :4-10 `v-if serverPathDetected && !isConfigInitialized` | 保留作后续软提示 |
| 现有 router import | **无** `vue-router` import | 需加 `useRouter` |

---

## 1. 修正方向总览

**T1 概览修正**：删 `.launch-zone`(:13-106) → 三路径+三启动按钮并进向导 `s1-wrap` 的 StepCard 美学（StepCard 当标题卡 + 下方 glass 操作区）；清死 CSS；补运行态关服入口。

**T2 空配置首启跳转**：`checkConfigInitialized` 完成后，若 `serverPathDetected && !isConfigInitialized && !isRunning` 且本会话未跳过 → `router.push('/config')` + toast；每会话仅一次（module-level 锁防循环），后续访问靠 `config-banner` 软提示。

---

## 2. 文件级改动 — `src/views/OverviewView.vue`

### 2.1 删

| 对象 | 行号 | 说明 |
|---|---|---|
| `.launch-zone` 整块 | :12-106 | 含第1步三路径 + 第2步三启动 + lz-hint，常驻冗余 |
| `onManual()` 函数 | :418-425 | 与 `onManualServer`(:395-402) 重复（仅 setManual vs finishDetect 差异），合并后不再被引用 |
| `detectedPath` computed | :325-327 | 仅被已删的 `.lz-hint:105` 引用 |

### 2.2 死 CSS 清单（只服务于已删 `.launch-zone`，整段删）

`.launch-zone`(:723-731)、`.path-list`(:733)、`.path-row`(:738)+`.path-row.ok`(:749)、`.path-info`(:753)、`.path-name`(:761)、`.path-state`(:766)+`.ok/.todo`(:772)、`.path-detail`(:774)、`.flow-title`(:784)、`.flow-row`(:790)、`.flow-card`(:795)+`.flow-card.done`(:806)、`.fc-head`(:810)、`.fc-num`(:815)+`.flow-card.done .fc-num`(:829)、`.fc-state`(:830)+`.ok/.todo`(:836)、`.fc-title`(:838)、`.fc-desc`(:843)、`.flow-card .btn`(:849)、`.lz-hint`(:850)+`.lz-hint code`(:857)。

> 向导样式 `s1-wrap/s1-col/s1-hero/s1-title/s1-sub/step-badge/steps-row/cta-row/tip-row`(:692 前后) **保留**。合并后操作区用新 class + `--glass-*` token 重写（见 2.4）。

### 2.3 改 — 向导 `s1-wrap` 重构（:182-242）

保留 `step-badge`/`s1-hero`/`s1-title`/`s1-sub`/`tip-row` 框架；用两段 `.sc-block`（StepCard 标题卡 + `.sc-body` 操作区）替换原 `steps-row`+`cta-row`(:202-230)：

```html
<!-- StepCard 1：获取三个应用路径 -->
<div class="sc-block">
  <StepCard :num="1" title="获取应用路径" desc="自动定位 服务器 / Radmin / 游戏" :state="step1State" />
  <div class="sc-body">
    <!-- 服务器行：已定位显路径；未定位显「自动探测」(onDetect)+「手动选目录」(onManualServer) -->
    <div class="sc-path-row" :class="{ ok: serverPathDetected }"> … </div>
    <!-- Radmin 行：networkStore.checkRadmin 态；未定位显「手动选目录」(onManualRadmin) -->
    <div class="sc-path-row" :class="{ ok: radminDetected }"> … </div>
    <!-- 游戏行：steam://rungameid/1623730 探测；未定位显「手动确认」(onManualGame) -->
    <div class="sc-path-row" :class="{ ok: gameDetected }"> … </div>
  </div>
</div>

<!-- StepCard 2：启动并联机 -->
<div class="sc-block">
  <StepCard :num="2" title="启动并联机" desc="开服 + Radmin + 游戏客户端" :state="step2State" />
  <div class="sc-body sc-launch-row">
    <!-- 服务器卡：!isRunning→启动服务器(onStart)；isRunning→优雅关服+强制停止(onGracefulShutdown/onForceStop) -->
    <div class="sc-launch-card" :class="{ done: isRunning }"> … </div>
    <!-- Radmin 卡：onLaunchRadmin -->
    <div class="sc-launch-card" :class="{ done: radminLaunched }"> … </div>
    <!-- 游戏卡：onLaunchGame -->
    <div class="sc-launch-card"> … </div>
  </div>
</div>
```

**computed 重定义**（替换 :329-336）：
```ts
const step1State = computed<'active'|'locked'|'done'>(() =>
  serverPathDetected.value && radminDetected.value && gameDetected.value ? 'done'
  : serverPathDetected.value ? 'active' : 'active')
const step2State = computed<'active'|'locked'|'done'>(() =>
  isRunning.value ? 'done' : serverPathDetected.value ? 'active' : 'locked')
const wizardStepText = computed(() =>
  isRunning.value ? '服务器运行中 · 可优雅关服'
  : serverPathDetected.value ? '路径已就绪 · 点击启动开服'
  : '首次设置 · 定位应用路径')
```

### 2.4 加 — 新 CSS（glass token，置于向导样式区）

```css
.sc-block { margin-top: 18px; display: flex; flex-direction: column; gap: 10px; }
.sc-body { display: flex; flex-direction: column; gap: 10px; }
.sc-path-row {
  display: flex; align-items: center; gap: 12px; padding: 12px 16px;
  border-radius: var(--r-card, 12px);
  background: var(--glass-bg, rgba(255,252,247,0.72));
  backdrop-filter: var(--glass-blur); -webkit-backdrop-filter: var(--glass-blur);
  border: 1px solid var(--glass-border, rgba(116,88,72,0.14));
}
.sc-path-row.ok { border-color: rgba(79,138,107,0.4); background: rgba(79,138,107,0.08); }
.sc-launch-row { flex-direction: row; flex-wrap: wrap; }
.sc-launch-card {
  flex: 1 1 200px; display: flex; flex-direction: column; gap: 8px; padding: 16px;
  border-radius: var(--r-card, 12px);
  background: var(--glass-bg, rgba(255,252,247,0.72));
  backdrop-filter: var(--glass-blur); -webkit-backdrop-filter: var(--glass-blur);
  border: 1px solid var(--glass-border, rgba(116,88,72,0.14));
}
.sc-launch-card.done { background: rgba(79,138,107,0.10); border-color: rgba(79,138,107,0.4); }
```
> 路径态徽标/序号圆点等细样式复用既有 `.path-state`/`.fc-num` 视觉语义，但 class 名归到 `.sc-*` 命名空间，颜色一律 `--palwarm-*`/`--glass-*`/`--green`/`--primary` token，禁止散写 hex、禁止改回深色冷调。

### 2.5 加 — 仪表盘运行态关服入口（设计判断·补回归）

**判断依据**：关服入口原仅存于 `.launch-zone:73-74`（已删）；`onStart` 成功后 `setMode('dashboard')`(:436) → 运行态在仪表盘，而仪表盘(:109-179)无关服按钮 → 删 launch-zone 后运行态**无法关服**，明确回归。向导 StepCard2 的关服分支仅覆盖「运行中仍停留向导」边缘态，不覆盖主流运行态（仪表盘）。

**最小必要补充**（不动指标卡 :134-159、不动 7 步引导 :170-178）：在仪表盘进程状态行 `dash-proc-row`(:162-167) 末尾追加两按钮：
```html
<div class="dash-proc-row">
  <StatusPill status="ok" :text="`运行中 · PID ${serverStore.status.pid ?? '?'}`" />
  <span class="dash-proc-path" v-if="settingsStore.settings.server_path">{{ … }}</span>
  <button class="btn btn-ghost btn-sm" :disabled="serverStore.loading" @click="onGracefulShutdown">优雅关服</button>
  <button class="btn btn-danger-ghost btn-sm" :disabled="serverStore.loading" @click="onForceStop">强制停止</button>
</div>
```
> 复用既有 `onGracefulShutdown`/`onForceStop`（:471/:487），零新逻辑。若老板坚持「仪表盘 :109-179 字面零改动」，则裁剪本项——但需接受运行态无关服入口的回归，建议保留。

### 2.6 加 — T2 空配置首启跳转

**import**（:259 区域）：
```ts
import { useRouter } from 'vue-router'
```
**module-level 锁**（防重定向循环；<script setup> 顶层 let 是每次 setup 重建，必须用独立 `<script>` 非 setup 块持会话级状态）：
```html
<script lang="ts">
// 会话级：空配置首启跳转仅触发一次，防 /config ↔ /overview 循环
let configRedirectDone = false
</script>
<script setup lang="ts">
… 既有 …
const router = useRouter()
```
**onMounted 改为 await checkConfigInitialized**（:293-299）：
```ts
onMounted(async () => {
  onboardingStore.onSuccess(() => { toast.success('🎉 联机成功！…') })
  await checkConfigInitialized()   // 由 void 改 await，跳转在其内判定
  void detectPaths()
})
```
**checkConfigInitialized 末尾加跳转**（:529-539）：
```ts
async function checkConfigInitialized(): Promise<void> {
  if (!serverPathDetected.value) { isConfigInitialized.value = false; return }
  try {
    isConfigInitialized.value = await api.config.isInitialized(settingsStore.settings.server_path)
  } catch { isConfigInitialized.value = false }
  // 空配置首启跳转：仅当已定位服务器 + 配置未初始化 + 未运行 + 本会话未跳过
  if (serverPathDetected.value && !isConfigInitialized.value && !isRunning.value && !configRedirectDone) {
    configRedirectDone = true
    toast.info('首次开服，请先填充默认配置')
    router.push('/config')
  }
}
```
> 跳转条件含 `serverPathDetected`（与 `config-banner:4` 一致，未定位服务器时不跳 config）。`configRedirectDone` 会话级锁保证仅跳一次；后续访问 `/overview` 不再跳，靠 `config-banner`(:4-10) 软提示。

---

## 3. 复用 handler/函数清单

| 函数/ref | 行号 | 处置 |
|---|---|---|
| `detectPaths` | :365 | **保留** onMounted 自动探测三路径 |
| `onDetect` | :338 | **保留** StepCard1 服务器行「自动探测/重试」按钮 |
| `detectLabel` | :323 | **保留**（onDetect/detectPaths 写） |
| `onManualServer` | :395 | **保留** StepCard1 服务器行「手动选目录」 |
| `onManualRadmin` | :404 | **保留** StepCard1 Radmin 行 |
| `onManualGame` | :412 | **保留** StepCard1 游戏行 |
| `onManual` | :418 | **删**（与 onManualServer 重复） |
| `detectedPath` | :325 | **删**（仅 lz-hint 用） |
| `onStart` | :427 | **保留** StepCard2 服务器卡 |
| `onLaunchRadmin` | :511 | **保留** StepCard2 Radmin 卡 |
| `onLaunchGame` | :308 | **保留** StepCard2 游戏卡 |
| `onGracefulShutdown` | :471 | **保留** StepCard2 运行态 + 仪表盘 dash-proc-row |
| `onForceStop` | :487 | **保留** 同上 |
| `checkConfigInitialized` | :529 | **改**（末尾加跳转） |
| `isConfigInitialized` | :528 | **保留** |
| `isRunning` | :304 | **保留** |
| `serverPathDetected` | :305 | **保留** |
| `step1State/step2State/wizardStepText` | :329-336 | **改**（重定义，见 2.3） |
| `radminDetected/radminPath/gameDetected/radminLaunched/launchingRadmin/launchingGame` | :287-289/284/282 | **保留** |

---

## 4. 任务列表

| ID | 名称 | 文件 | 依赖 | 优先级 | 内容 |
|---|---|---|---|---|---|
| T1 | 概览向导合并 + 关服闭环 | OverviewView.vue | 无 | P0 | 删 .launch-zone(:13-106)+死CSS；重构 s1-wrap 为 StepCard1(三路径)+StepCard2(三启动)；删 onManual/detectedPath；重定义 step1/2State/wizardStepText；仪表盘 dash-proc-row 补关服按钮(§2.5) |
| T2 | 空配置首启跳转 | OverviewView.vue | 无 | P0 | 加 useRouter import + `<script>`块 configRedirectDone 锁；onMounted await checkConfigInitialized；checkConfigInitialized 末尾加跳转(toast+router.push) |

> T1/T2 同文件、无依赖、无冲突（T1 改 template/CSS/部分 script，T2 加 import/锁/onMounted/checkConfigInitialized），工程师可一次合并提交。

---

## 5. 共享知识

- 颜色一律 `--palwarm-*`/`--glass-*`/`--green`/`--primary`/`--r-card` token（`src/style.css` 已定义）；**禁止散写 hex、禁止改回深色冷调**。
- 关服闭环：StepCard2(isRunning 分支) + 仪表盘 dash-proc-row 两处复用同一 `onGracefulShutdown`/`onForceStop`，零新 store 调用。
- `configRedirectDone` 必须放独立 `<script lang="ts">`（非 setup）块——`<script setup>` 顶层 let 每次 setup 重建，无法跨挂载保会话级。
- 首启跳转条件含 `serverPathDetected`：未定位服务器时不跳 `/config`（config 页依赖 server_path），与 `config-banner:4` 显示条件对齐。
- 不动 `save_edit` 测试模块、`config.rs`、`stores/*`、路由表、Sidebar。
- `onStart` 成功后 `setMode('dashboard')` + `startPolling()` 既有逻辑保留不变。

---

## 6. 待明确事项

空。方向已拍板；§2.5 仪表盘关服入口为读码后发现的回归风险，已给出最小补充方案（不动指标/引导），属设计判断而非开放问题——若老板坚持仪表盘字面零改动，裁剪 §2.5 即可，但需接受运行态关服回归。
