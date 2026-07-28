// cargo target 目录解析（共享模块）。
//
// 背景：F: 盘为 exFAT，cargo target 必须落 NTFS 以保证构建可靠性与速度。
// 历史做法是把 target-dir 硬编码进 src-tauri/.cargo/config.toml，但换电脑或
// C 盘不可用时会直接破坏构建。改为由构建脚本显式设置可覆盖的 CARGO_TARGET_DIR。
//
// 覆盖优先级：PALWORLD_CARGO_TARGET_DIR > CARGO_TARGET_DIR > 默认值。
// with-cargo-target.mjs 与 package-portable.mjs 都通过本模块读取同一目标目录，
// 确保「构建 EXE」与「打包 EXE」定位到同一处。

import { delimiter as PATH_DELIMITER, dirname, join } from 'node:path';

export const DEFAULT_CARGO_TARGET_DIR = join('C:', 'codex-target', 'palworld-portable');

export function resolveCargoTargetDir() {
  const override = process.env.PALWORLD_CARGO_TARGET_DIR || process.env.CARGO_TARGET_DIR;
  if (override && override.trim()) return override.trim();
  return DEFAULT_CARGO_TARGET_DIR;
}

export function buildToolPath({
  projectRoot,
  nodeExecutable = process.execPath,
  currentPath = process.env.PATH || '',
  delimiter = PATH_DELIMITER,
}) {
  return [
    join(projectRoot, 'node_modules', '.bin'),
    dirname(nodeExecutable),
    currentPath,
  ].filter(Boolean).join(delimiter);
}
