#!/usr/bin/env node
// 便携文件夹版打包脚本（Node ESM）。
//
// 组装布局：
//   <staging>/
//     Palworld Server Manager.exe
//     portable.flag
//     README.txt
//     LICENSE.txt
//     THIRD_PARTY_NOTICES.txt
//     data/.gitkeep
//     backups/local/.gitkeep
//     backups/server/.gitkeep
//     backups/snapshots/.gitkeep
//     backups/_system/.gitkeep
//
// 产物（dist-portable/）：
//   Palworld-Server-Manager-Portable-v<version>-win64.zip
//   Palworld-Server-Manager-Portable-v<version>-win64.zip.sha256
//   manifest.json
//
// 用法：node scripts/package-portable.mjs --exe <path> --out dist-portable [--version 1.0.0]
// 不传 --exe 时按候选顺序自动探测 release EXE。

import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import {
  cpSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  renameSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { basename, dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { resolveCargoTargetDir } from './cargo-target.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const PROJECT_ROOT = resolve(__dirname, '..');

export const PRODUCT_NAME = 'Palworld Server Manager';
// 分发产物里的友好文件名（staging 阶段会把 cargo 原始产物改名复制为它）。
export const EXE_NAME = `${PRODUCT_NAME}.exe`;
// ZIP 解压后必须得到「一个文件夹」，而不是把文件直接散落到用户选择的目录。
// 该常量既是 staging 目录名，也是 ZIP 内的根文件夹名。
export const PORTABLE_FOLDER_NAME = 'Palworld Server Manager Portable';

/**
 * 从 src-tauri/Cargo.toml 读取 `[package] name`，即 Tauri/cargo 真实输出的 EXE 文件名。
 * 真实构建产物是 `palworld-server-manager.exe`（cargo 包名），而不是友好的
 * `Palworld Server Manager.exe`。打包脚本必须按 cargo 包名定位真实产物，
 * 再在 staging 阶段改名为 EXE_NAME。
 */
function readCargoBinName() {
  try {
    const toml = readFileSync(join(PROJECT_ROOT, 'src-tauri', 'Cargo.toml'), 'utf8');
    const m = toml.match(/^\s*name\s*=\s*"([^"]+)"\s*$/m);
    if (m) return m[1];
  } catch {
    // fallthrough
  }
  return 'palworld-server-manager';
}
export const CARGO_BIN_NAME = readCargoBinName();
export const CARGO_BIN_EXE = `${CARGO_BIN_NAME}.exe`;

// 禁止进入发布包的文件/目录模式（相对 staging 根，正斜杠）。
export const FORBIDDEN_PATTERNS = [
  /(^|\/)\.env$/i,
  /\.pdb$/i,
  /(^|\/)target(\/|$)/i,
  /(^|\/)node_modules(\/|$)/i,
  /(^|\/)\.git(\/|$)/i,
  /\.log$/i,
];

/**
 * 构建 EXE 探测候选（按优先级）：与 with-cargo-target.mjs 同源的 target-dir 优先，
 * 回退默认 cargo target（src-tauri/target）。注意查找的是 cargo 真实产物名
 * （CARGO_BIN_EXE），不是分发的友好名。
 */
export function buildExeCandidates({ cargoTargetDir, projectRoot } = {}) {
  const target = cargoTargetDir || resolveCargoTargetDir();
  const root = projectRoot || PROJECT_ROOT;
  return [
    join(target, 'release', CARGO_BIN_EXE),
    join(root, 'src-tauri', 'target', 'release', CARGO_BIN_EXE),
  ];
}

const EXE_CANDIDATES = buildExeCandidates();

/** 扫描 staging 目录，发现禁止文件则抛错（code=FORBIDDEN_FILES）。 */
export function scanForbidden(stagingRoot) {
  const found = [];
  const walk = (dir) => {
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      const full = join(dir, entry.name);
      const rel = relative(stagingRoot, full).replace(/\\/g, '/');
      if (FORBIDDEN_PATTERNS.some((re) => re.test(rel) || re.test(entry.name))) {
        found.push(rel);
      }
      if (entry.isDirectory()) walk(full);
    }
  };
  walk(stagingRoot);
  if (found.length > 0) {
    const err = new Error(`staging 目录包含禁止文件: ${found.join(', ')}`);
    err.code = 'FORBIDDEN_FILES';
    err.files = found;
    throw err;
  }
  return [];
}

/** 复制 LICENSE 与第三方声明；缺失 LICENSE 则抛错（发布包必须含许可证）。 */
export function copyLicenseOrThrow(licenseRoot, stagingRoot) {
  const licenseSrc = join(licenseRoot, 'LICENSE');
  if (!existsSync(licenseSrc)) {
    const err = new Error(`缺失 LICENSE 文件: ${licenseSrc}（发布包必须包含许可证）`);
    err.code = 'LICENSE_MISSING';
    throw err;
  }
  cpSync(licenseSrc, join(stagingRoot, 'LICENSE.txt'));
  const thirdPartySrc = join(licenseRoot, 'THIRD_PARTY_NOTICES.md');
  if (existsSync(thirdPartySrc)) {
    cpSync(thirdPartySrc, join(stagingRoot, 'THIRD_PARTY_NOTICES.txt'));
  }
}

/** 写中文 README.txt。 */
export function writeReadme(stagingRoot, opts = {}) {
  const version = opts.version || '1.0.0';
  const content = [
    `Palworld Server Manager 便携版 v${version}`,
    '',
    '【运行方式】',
    '1. 将整个文件夹解压到任意可写位置（如 D:\\PalworldServerManager）。',
    '2. 双击 "Palworld Server Manager.exe" 启动。',
    '3. 请勿从压缩包内直接运行——必须先解压。',
    '',
    '【便携数据位置】',
    '所有应用数据随文件夹移动：',
    '  - 设置：data/settings.json',
    '  - 项目日志：data/logs/app.log（程序运行自动记录，含崩溃信息）',
    '  - 配置备份：data/config-backups/',
    '  - 世界备份：backups/local、backups/server、backups/snapshots、backups/_system',
    '',
    '【前置条件：Microsoft Edge WebView2 Runtime】',
    '本程序依赖 WebView2 Runtime（Evergreen）。大多数 Windows 11 已预装。',
    '若启动失败并提示缺少 WebView2，请从微软官网下载安装：',
    '  https://developer.microsoft.com/microsoft-edge/webview2/',
    '',
    '【移动与升级】',
    ' - 移动：直接移动整个文件夹即可，设置与备份随文件夹一起迁移。',
    ' - 升级：用新版本的 EXE 覆盖旧 EXE，保留 data/ 与 backups/ 即可保留所有数据。',
    '',
    '【便携模式启用】',
    'EXE 同级的 portable.flag 文件启用便携模式。删除该文件则回退到安装模式',
    '（数据存放到系统 AppData）。便携版用户请勿删除 portable.flag。',
    '',
  ].join('\r\n');
  const path = join(stagingRoot, 'README.txt');
  writeFileSync(path, content, 'utf8');
  return path;
}

/** 组装 staging 目录。返回 stagingRoot。 */
export function buildStaging(exeSrc, opts = {}) {
  const stagingRoot = opts.stagingRoot;
  if (!stagingRoot) throw new Error('stagingRoot is required');
  if (!existsSync(exeSrc)) {
    const err = new Error(
      `找不到 EXE: ${exeSrc}（请先运行 npm run tauri:build:portable，或通过 --exe 指定路径）`
    );
    err.code = 'EXE_MISSING';
    throw err;
  }
  rmSync(stagingRoot, { recursive: true, force: true });
  mkdirSync(stagingRoot, { recursive: true });

  cpSync(exeSrc, join(stagingRoot, EXE_NAME));
  writeFileSync(join(stagingRoot, 'portable.flag'), '');

  const licenseRoot = opts.licenseRoot || PROJECT_ROOT;
  copyLicenseOrThrow(licenseRoot, stagingRoot);

  writeReadme(stagingRoot, opts);

  for (const sub of [
    'data',
    'data/logs',
    'data/config-backups',
    'backups/local',
    'backups/server',
    'backups/snapshots',
    'backups/_system',
  ]) {
    const dir = join(stagingRoot, sub);
    mkdirSync(dir, { recursive: true });
    writeFileSync(join(dir, '.gitkeep'), '');
  }
  return stagingRoot;
}

/** 计算 staging 内每个文件的 SHA-256 与大小，返回 manifest 对象。 */
export function computeManifest(stagingRoot, meta = {}) {
  const files = [];
  const walk = (dir) => {
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      const full = join(dir, entry.name);
      if (entry.isDirectory()) {
        walk(full);
        continue;
      }
      const rel = relative(stagingRoot, full).replace(/\\/g, '/');
      const data = readFileSync(full);
      const sha256 = createHash('sha256').update(data).digest('hex');
      files.push({ path: rel, sha256, size_bytes: data.length });
    }
  };
  walk(stagingRoot);
  files.sort((a, b) => a.path.localeCompare(b.path));
  return {
    product: PRODUCT_NAME,
    version: meta.version || '1.0.0',
    built_at: meta.built_at || new Date().toISOString(),
    // 版本追溯：
    // - source_base_commit：仓库 HEAD，标识构建时的版本起点。
    // - portable_source_sha256：Palworld 源码树（排除产物）的归一化哈希，
    //   用于唯一标识实际打包的源码。
    source_base_commit: meta.source_base_commit || null,
    portable_source_sha256: meta.portable_source_sha256 || null,
    webview2_policy: 'evergreen-prerequisite',
    files,
  };
}

// 计算 Palworld 源码树的归一化 SHA-256，用于版本追溯。
// 排除构建产物与依赖：node_modules、target、dist、dist-portable、.git、.cargo 等。
export function computePortableSourceSha256(root) {
  const excludeDirs = new Set([
    'node_modules',
    'target',
    'dist',
    'dist-portable',
    '.git',
    '.cargo',
    '.trae',
  ]);
  const files = [];
  const walk = (dir, rel = '') => {
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      const relPath = rel ? `${rel}/${entry.name}` : entry.name;
      if (entry.isDirectory()) {
        if (excludeDirs.has(entry.name)) continue;
        walk(join(dir, entry.name), relPath);
      } else {
        files.push(relPath);
      }
    }
  };
  walk(root);
  files.sort();
  const hasher = createHash('sha256');
  for (const f of files) {
    const data = readFileSync(join(root, f));
    const fileSha = createHash('sha256').update(data).digest('hex');
    hasher.update(f);
    hasher.update('\0');
    hasher.update(fileSha);
    hasher.update('\0');
  }
  return hasher.digest('hex');
}

/** 将 staging 压缩为 ZIP，使 ZIP 内含外层 `Palworld Server Manager Portable/` 根文件夹。
 *  优先用系统自带 tar.exe（libarchive，Windows 10+ 自带，不依赖可能不可用的
 *  Microsoft.PowerShell.Archive 模块）；失败则回退 .NET ZipFile（PowerShell 内置
 *  System.IO.Compression.FileSystem，includeBaseDirectory=true 保留外层文件夹）。 */
export function zipStaging(stagingRoot, outZip) {
  rmSync(outZip, { force: true });
  const parent = dirname(stagingRoot);
  const child = basename(stagingRoot);
  // tar: -a 按扩展名选格式(.zip)，-C parent child → ZIP 内含 child/ 根文件夹。
  try {
    execFileSync('tar', ['-a', '-cf', outZip, '-C', parent, child], { stdio: 'pipe' });
    return outZip;
  } catch (e1) {
    const ps =
      `Add-Type -AssemblyName System.IO.Compression.FileSystem; ` +
      `[System.IO.Compression.ZipFile]::CreateFromDirectory(` +
      `'${stagingRoot}', '${outZip}', [System.IO.Compression.CompressionLevel]::Optimal, $true)`;
    try {
      execFileSync('powershell', ['-NoProfile', '-Command', ps], { stdio: 'pipe' });
      return outZip;
    } catch (e2) {
      throw new Error(
        `ZIP 压缩失败：tar.exe 与 .NET ZipFile 均不可用。\n  tar: ${e1.message}\n  .NET: ${e2.message}`
      );
    }
  }
}

/** 写 ZIP 的 SHA-256 sidecar（<zip>.sha256）。返回 { sidecar, sha256 }。 */
export function writeSha256Sidecar(zipPath) {
  const data = readFileSync(zipPath);
  const sha256 = createHash('sha256').update(data).digest('hex');
  const sidecar = `${zipPath}.sha256`;
  writeFileSync(sidecar, `${sha256}  ${basename(zipPath)}\n`);
  return { sidecar, sha256 };
}

function readVersion() {
  try {
    const pkg = JSON.parse(readFileSync(join(PROJECT_ROOT, 'package.json'), 'utf8'));
    return pkg.version || '1.0.0';
  } catch {
    return '1.0.0';
  }
}

function resolveExe(arg, candidates = null) {
  if (arg) return resolve(arg);
  // 运行时重新计算候选，使 PALWORLD_CARGO_TARGET_DIR 环境变量覆盖即时生效
  // （EXE_CANDIDATES 仅在模块加载时算一次，会漏掉后续 env 变更）。
  const list = candidates || buildExeCandidates();
  for (const candidate of list) {
    if (existsSync(candidate)) return candidate;
  }
  return list[list.length - 1]; // 返回最后候选，让 buildStaging 抛 EXE_MISSING
}

function parseArgs(argv) {
  const args = { exe: null, out: 'dist-portable', version: null };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--exe') args.exe = argv[++i];
    else if (a === '--out') args.out = argv[++i];
    else if (a === '--version') args.version = argv[++i];
  }
  return args;
}

function sourceBaseCommit() {
  // Palworld 位于仓库子目录，git 会向上发现仓库根目录。
  try {
    const out = execFileSync(
      'git',
      ['-C', PROJECT_ROOT, 'rev-parse', 'HEAD'],
      { stdio: ['ignore', 'pipe', 'ignore'], encoding: 'utf8' }
    );
    return out.trim();
  } catch {
    return null;
  }
}

/** 紧凑时间戳，用于版本化发布子目录名（YYYYMMDD-HHmmss）。 */
function formatDirTimestamp(d = new Date()) {
  const pad = (n) => String(n).padStart(2, '0');
  return `${d.getFullYear()}${pad(d.getMonth() + 1)}${pad(d.getDate())}-${pad(d.getHours())}${pad(d.getMinutes())}${pad(d.getSeconds())}`;
}

/**
 * 生成版本化发布子目录路径：`<outDir>/v<version>-<timestamp>`。
 * 绝不删除既有目录——若名称碰撞（同一秒内重建），追加计数器 -1、-2 … 直到不碰撞。
 * 旧版本保留在各自版本化子目录中，仅通过 latest.json 指向当前版本，
 * 满足「不删除既有发布物」约束。
 */
export function uniqueVersionedDir(outDir, version) {
  const ts = formatDirTimestamp();
  let name = `v${version}-${ts}`;
  let dir = join(outDir, name);
  let counter = 1;
  while (existsSync(dir)) {
    name = `v${version}-${ts}-${counter}`;
    dir = join(outDir, name);
    counter++;
  }
  return dir;
}

/**
 * 将构建好的 tmpDir 原子发布为版本化子目录，再原子更新 latest.json 索引。
 *
 * 关键安全约束：
 *  - 绝不递归删除 outDir 或任何既有版本化子目录。
 *  - rename 到 uniqueVersionedDir 选出的「不存在」子目录（同卷原子）。
 *    若 versionedDir 意外已存在且非空，rename 会失败抛错——这是正确的，
 *    不覆盖任何既有发布物。
 *  - 新版本完整落盘后，才用「先写 .tmp 再 rename」原子更新 latest.json。
 *    latest 更新失败不影响已发布的版本化目录，仅 latest 指针保持旧值。
 *
 * 失败语义：任一步抛错时，tmpDir 由调用方清理；outDir 与既有版本化子目录不受影响。
 */
export function releaseToVersionedDir(tmpDir, outDir, versionedDir, latestInfo) {
  mkdirSync(outDir, { recursive: true });
  // rename 到不存在的版本化子目录（同卷原子）。不预删除任何东西。
  renameSync(tmpDir, versionedDir);
  // 新版本已完整落盘 → 原子更新 latest.json 索引。
  const latestPath = join(outDir, 'latest.json');
  const tmpLatest = join(outDir, '.latest.json.tmp');
  writeFileSync(tmpLatest, `${JSON.stringify(latestInfo, null, 2)}\n`, 'utf8');
  renameSync(tmpLatest, latestPath);
}

export function packageAll(opts) {
  const { exe, out, version } = opts;
  const ver = version || readVersion();
  const exePath = resolveExe(exe);
  const outDir = resolve(PROJECT_ROOT, out);

  // 原子发布：所有产物先在与 outDir 同级的临时目录里构建，全部成功后
  // 原子重命名为「版本化子目录」（绝不递归删除 outDir / 旧版本），再原子更新 latest.json。
  // 任一步骤失败都只清理临时目录，既有发布物与 outDir 完全不受影响。
  const outParent = dirname(outDir);
  const tmpDir = join(outParent, `.${basename(outDir)}.tmp-${process.pid}`);
  rmSync(tmpDir, { recursive: true, force: true });
  mkdirSync(tmpDir, { recursive: true });

  try {
    // staging 目录名 = ZIP 内根文件夹名，确保解压得到一个文件夹。
    const stagingRoot = join(tmpDir, PORTABLE_FOLDER_NAME);
    const zipName = `Palworld-Server-Manager-Portable-v${ver}-win64.zip`;
    const tmpZip = join(tmpDir, zipName);

    buildStaging(exePath, {
      stagingRoot,
      version: ver,
      licenseRoot: PROJECT_ROOT,
    });
    scanForbidden(stagingRoot);
    const manifest = computeManifest(stagingRoot, {
      version: ver,
      source_base_commit: sourceBaseCommit(),
      portable_source_sha256: computePortableSourceSha256(PROJECT_ROOT),
    });
    // manifest 先写临时目录，压缩成功后才随 ZIP 一起发布。
    writeFileSync(join(tmpDir, 'manifest.json'), JSON.stringify(manifest, null, 2), 'utf8');
    zipStaging(stagingRoot, tmpZip);
    const { sha256 } = writeSha256Sidecar(tmpZip);
    // staging 仅是中间产物，ZIP 生成后即删除，版本化目录最终只含 zip + sha256 + manifest。
    rmSync(stagingRoot, { recursive: true, force: true });

    // 全部成功 → 发布到版本化子目录（不删除 outDir / 旧版本），再更新 latest.json。
    const versionedDir = uniqueVersionedDir(outDir, ver);
    releaseToVersionedDir(tmpDir, outDir, versionedDir, {
      version: ver,
      built_at: new Date().toISOString(),
      dir: basename(versionedDir),
      zip: zipName,
      sha256,
      sha256_file: `${zipName}.sha256`,
      manifest: 'manifest.json',
    });
    return {
      zipPath: join(versionedDir, zipName),
      sha256,
      manifestPath: join(versionedDir, 'manifest.json'),
      versionedDir,
      latestPath: join(outDir, 'latest.json'),
    };
  } catch (e) {
    // 失败：清理临时目录，保持 outDir 与既有发布物完全不受影响。
    rmSync(tmpDir, { recursive: true, force: true });
    throw e;
  }
}

// CLI 入口
const isMain = process.argv[1] && resolve(process.argv[1]) === __filename;
if (isMain) {
  try {
    const args = parseArgs(process.argv.slice(2));
    const result = packageAll(args);
    console.log('便携版打包完成：');
    console.log(`  ZIP:    ${result.zipPath}`);
    console.log(`  SHA256: ${result.sha256}`);
    console.log(`  清单:   ${result.manifestPath}`);
  } catch (e) {
    console.error(`[package-portable] 失败: ${e.message}`);
    process.exit(1);
  }
}
