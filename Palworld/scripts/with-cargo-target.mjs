#!/usr/bin/env node
// 为 cargo / tauri 显式设置 CARGO_TARGET_DIR，避免在 exFAT (F:) 上构建。
//
// 用法：
//   node scripts/with-cargo-target.mjs tauri build --no-bundle   # 构建 release EXE
//   node scripts/with-cargo-target.mjs tauri dev                 # 开发模式
//   node scripts/with-cargo-target.mjs check                     # cargo check
//   node scripts/with-cargo-target.mjs test                      # cargo test
//   node scripts/with-cargo-target.mjs fmt                       # cargo fmt
//   node scripts/with-cargo-target.mjs clippy                    # cargo clippy
//
// 覆盖默认 target 目录：
//   $env:PALWORLD_CARGO_TARGET_DIR = 'D:\my-ntfs-target'
//   npm run tauri:build:portable
//
// 说明：
// - first arg === 'tauri' 时在项目根运行 `npx tauri <...>`（tauri.conf.json 约定）。
// - 其余视为 cargo 子命令，在 src-tauri/ 运行（Cargo.toml 所在目录）。
// - target 目录不存在时自动创建。

import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync } from 'node:fs';
import { join, resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { buildToolPath, resolveCargoTargetDir } from './cargo-target.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const PROJECT_ROOT = resolve(__dirname, '..');
const SRC_TAURI = join(PROJECT_ROOT, 'src-tauri');

const targetDir = resolveCargoTargetDir();
if (!existsSync(targetDir)) {
  mkdirSync(targetDir, { recursive: true });
}

const args = process.argv.slice(2);
if (args.length === 0) {
  console.error('用法: node scripts/with-cargo-target.mjs <command> [args...]');
  console.error('示例:');
  console.error('  node scripts/with-cargo-target.mjs tauri build --no-bundle');
  console.error('  node scripts/with-cargo-target.mjs test');
  console.error('覆盖默认 target 目录: 设置 PALWORLD_CARGO_TARGET_DIR 环境变量');
  process.exit(2);
}

const env = { ...process.env, CARGO_TARGET_DIR: targetDir };
const first = args[0];
let result;
if (first === 'tauri') {
  // 优先用项目本地的 tauri CLI（node_modules/.bin），避免依赖全局 npx/npm PATH。
  // 这样无论通过 `npm run` 还是直接 `node scripts/...` 调用都能解析到 tauri。
  const childEnv = {
    ...env,
    PATH: buildToolPath({ projectRoot: PROJECT_ROOT }),
  };
  result = spawnSync('tauri', args.slice(1), {
    stdio: 'inherit',
    env: childEnv,
    cwd: PROJECT_ROOT,
    shell: process.platform === 'win32',
  });
} else {
  result = spawnSync('cargo', args, {
    stdio: 'inherit',
    env,
    cwd: SRC_TAURI,
  });
}

process.exit(result.status ?? 1);
