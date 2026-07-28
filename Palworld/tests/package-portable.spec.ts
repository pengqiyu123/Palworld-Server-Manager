import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { basename, dirname, join } from 'node:path';
import {
  buildExeCandidates,
  buildStaging,
  CARGO_BIN_EXE,
  CARGO_BIN_NAME,
  computeManifest,
  computePortableSourceSha256,
  EXE_NAME,
  packageAll,
  PORTABLE_FOLDER_NAME,
  PRODUCT_NAME,
  releaseToVersionedDir,
  scanForbidden,
  uniqueVersionedDir,
  writeSha256Sidecar,
  zipStaging,
} from '../scripts/package-portable.mjs';
import { buildToolPath } from '../scripts/cargo-target.mjs';

function makeTmp(): string {
  return mkdtempSync(join(tmpdir(), 'palworld-pkg-'));
}
function makeLicenseRoot(dir: string): void {
  writeFileSync(join(dir, 'LICENSE'), 'GPL-3.0 fake license text\n');
  writeFileSync(join(dir, 'THIRD_PARTY_NOTICES.md'), 'third party notices\n');
}
function makeStubExe(dir: string): string {
  const exe = join(dir, 'stub.exe');
  writeFileSync(exe, 'stub-exe-bytes');
  return exe;
}
function sha256OfFile(path: string): string {
  return createHash('sha256').update(readFileSync(path)).digest('hex');
}

describe('package-portable script', () => {
  let work: string;
  beforeEach(() => {
    work = makeTmp();
  });
  afterEach(() => {
    rmSync(work, { recursive: true, force: true });
  });

  it('builds staging with flag, readme, license and full layout', () => {
    const licenseRoot = join(work, 'license');
    mkdirSync(licenseRoot, { recursive: true });
    makeLicenseRoot(licenseRoot);
    const exe = makeStubExe(work);
    const staging = join(work, 'staging');

    buildStaging(exe, { stagingRoot: staging, version: '1.0.0', licenseRoot });

    expect(existsSync(join(staging, 'portable.flag'))).toBe(true);
    expect(existsSync(join(staging, 'README.txt'))).toBe(true);
    expect(existsSync(join(staging, 'LICENSE.txt'))).toBe(true);
    expect(existsSync(join(staging, 'THIRD_PARTY_NOTICES.txt'))).toBe(true);
    expect(existsSync(join(staging, `${PRODUCT_NAME}.exe`))).toBe(true);
    // 数据目录布局与报告承诺一致：data/logs、data/config-backups 必须存在。
    expect(existsSync(join(staging, 'data', '.gitkeep'))).toBe(true);
    expect(existsSync(join(staging, 'data', 'logs', '.gitkeep'))).toBe(true);
    expect(existsSync(join(staging, 'data', 'config-backups', '.gitkeep'))).toBe(true);
    expect(existsSync(join(staging, 'backups', 'local', '.gitkeep'))).toBe(true);
    expect(existsSync(join(staging, 'backups', 'server', '.gitkeep'))).toBe(true);
    expect(existsSync(join(staging, 'backups', 'snapshots', '.gitkeep'))).toBe(true);
    expect(existsSync(join(staging, 'backups', '_system', '.gitkeep'))).toBe(true);
    // README 应含 WebView2 前置条件说明，且不再写「暂未启用项目日志」。
    const readme = readFileSync(join(staging, 'README.txt'), 'utf8');
    expect(readme).toContain('WebView2');
    expect(readme).not.toContain('暂未启用项目日志');
    expect(readme).toContain('data/logs/app.log');
  });

  it('forbidden files are rejected by scanForbidden', () => {
    const licenseRoot = join(work, 'license');
    mkdirSync(licenseRoot, { recursive: true });
    makeLicenseRoot(licenseRoot);
    const exe = makeStubExe(work);
    const staging = join(work, 'staging');
    buildStaging(exe, { stagingRoot: staging, version: '1.0.0', licenseRoot });

    // 注入禁止文件/目录
    writeFileSync(join(staging, '.env'), 'SECRET=1');
    mkdirSync(join(staging, 'target'), { recursive: true });
    writeFileSync(join(staging, 'target', 'x.pdb'), 'pdb-bytes');
    mkdirSync(join(staging, 'node_modules'), { recursive: true });
    writeFileSync(join(staging, 'node_modules', 'y'), 'nm-bytes');
    writeFileSync(join(staging, 'debug.log'), 'log-bytes');

    expect(() => scanForbidden(staging)).toThrowError(/禁止文件/);
  });

  it('manifest hashes and sizes match each staged file', () => {
    const licenseRoot = join(work, 'license');
    mkdirSync(licenseRoot, { recursive: true });
    makeLicenseRoot(licenseRoot);
    const exe = makeStubExe(work);
    const staging = join(work, 'staging');
    buildStaging(exe, { stagingRoot: staging, version: '1.0.0', licenseRoot });

    const manifest = computeManifest(staging, {
      version: '1.0.0',
      source_base_commit: 'abc123',
      portable_source_sha256: 'def456',
    });

    expect(manifest.product).toBe(PRODUCT_NAME);
    expect(manifest.version).toBe('1.0.0');
    expect(manifest.source_base_commit).toBe('abc123');
    expect(manifest.portable_source_sha256).toBe('def456');
    expect(manifest.webview2_policy).toBe('evergreen-prerequisite');
    expect(manifest.files.length).toBeGreaterThan(0);
    // 旧字段 git_commit 不应再出现。
    expect((manifest as Record<string, unknown>).git_commit).toBeUndefined();

    for (const entry of manifest.files) {
      const full = join(staging, entry.path);
      expect(existsSync(full)).toBe(true);
      expect(entry.sha256).toBe(sha256OfFile(full));
      expect(entry.size_bytes).toBe(readFileSync(full).length);
    }
  });

  it('portable source sha256 is deterministic and excludes build artifacts', () => {
    // 两次计算同源树应得到相同哈希。
    const a = computePortableSourceSha256(work);
    const b = computePortableSourceSha256(work);
    expect(a).toBe(b);
    expect(a).toMatch(/^[0-9a-f]{64}$/);

    // 写入产物目录（应被排除）不应改变哈希。
    mkdirSync(join(work, 'node_modules', 'pkg'), { recursive: true });
    writeFileSync(join(work, 'node_modules', 'pkg', 'index.js'), 'module.exports=1');
    mkdirSync(join(work, 'dist-portable'), { recursive: true });
    writeFileSync(join(work, 'dist-portable', 'x.zip'), 'zip-bytes');
    expect(computePortableSourceSha256(work)).toBe(a);

    // 修改源文件应改变哈希。
    writeFileSync(join(work, 'src.txt'), 'source change');
    expect(computePortableSourceSha256(work)).not.toBe(a);
  });

  it('zip and sha256 sidecar are produced', () => {
    const licenseRoot = join(work, 'license');
    mkdirSync(licenseRoot, { recursive: true });
    makeLicenseRoot(licenseRoot);
    const exe = makeStubExe(work);
    // staging 目录名必须等于 PORTABLE_FOLDER_NAME，ZIP 才会含外层文件夹。
    const staging = join(work, PORTABLE_FOLDER_NAME);
    buildStaging(exe, { stagingRoot: staging, version: '1.0.0', licenseRoot });

    const zipPath = join(work, 'portable.zip');
    zipStaging(staging, zipPath);
    expect(existsSync(zipPath)).toBe(true);
    expect(readFileSync(zipPath).length).toBeGreaterThan(0);

    const { sidecar, sha256 } = writeSha256Sidecar(zipPath);
    expect(existsSync(sidecar)).toBe(true);
    expect(sha256).toMatch(/^[0-9a-f]{64}$/);
    expect(sha256).toBe(sha256OfFile(zipPath));
    // sidecar 内容应含 64 位 hex
    expect(readFileSync(sidecar, 'utf8').trim()).toContain(sha256);
  });

  it('zip extracts to a single outer folder containing the full layout', () => {
    // 真实解压验收：产 ZIP → 解压 → 验根文件夹 + 布局 + portable.flag。
    const licenseRoot = join(work, 'license');
    mkdirSync(licenseRoot, { recursive: true });
    makeLicenseRoot(licenseRoot);
    const exe = makeStubExe(work);
    const staging = join(work, PORTABLE_FOLDER_NAME);
    buildStaging(exe, { stagingRoot: staging, version: '1.0.0', licenseRoot });

    const zipPath = join(work, 'portable.zip');
    zipStaging(staging, zipPath);

    const extractRoot = join(work, 'extracted');
    mkdirSync(extractRoot, { recursive: true });
    // 与生产打包保持同一工具链：部分 Windows 环境的
    // Microsoft.PowerShell.Archive 模块无法加载，不能让测试依赖它。
    execFileSync('tar', ['-xf', zipPath, '-C', extractRoot], { stdio: 'pipe' });

    // 解压后必须得到一个外层文件夹，而不是文件直接散落到 extractRoot。
    const outer = join(extractRoot, PORTABLE_FOLDER_NAME);
    expect(existsSync(outer)).toBe(true);
    expect(existsSync(join(outer, 'portable.flag'))).toBe(true);
    expect(existsSync(join(outer, `${PRODUCT_NAME}.exe`))).toBe(true);
    expect(existsSync(join(outer, 'README.txt'))).toBe(true);
    expect(existsSync(join(outer, 'LICENSE.txt'))).toBe(true);
    expect(existsSync(join(outer, 'data', 'logs', '.gitkeep'))).toBe(true);
    expect(existsSync(join(outer, 'data', 'config-backups', '.gitkeep'))).toBe(true);
    expect(existsSync(join(outer, 'backups', 'local', '.gitkeep'))).toBe(true);
    expect(existsSync(join(outer, 'backups', 'server', '.gitkeep'))).toBe(true);
    expect(existsSync(join(outer, 'backups', 'snapshots', '.gitkeep'))).toBe(true);
    expect(existsSync(join(outer, 'backups', '_system', '.gitkeep'))).toBe(true);

    // extractRoot 顶层应只有外层文件夹一个条目（无散落文件）。
    expect(readdirSync(extractRoot)).toHaveLength(1);
  });

  it('exe candidates target the real cargo binary name, not the friendly name', () => {
    // 真实 Tauri/cargo 产物是 palworld-server-manager.exe（Cargo.toml 包名），
    // 脚本必须按它定位，而不是分发的友好名 Palworld Server Manager.exe。
    expect(CARGO_BIN_NAME).toBe('palworld-server-manager');
    expect(CARGO_BIN_EXE).toBe('palworld-server-manager.exe');
    expect(CARGO_BIN_EXE).not.toBe(EXE_NAME);

    const candidates = buildExeCandidates({
      cargoTargetDir: join('C:', 'fake-target'),
      projectRoot: join(work, 'fake-project'),
    });
    // 每个候选都应以真实 cargo 产物名结尾，不得出现友好名。
    for (const c of candidates) {
      expect(basename(c)).toBe(CARGO_BIN_EXE);
      expect(basename(c)).not.toBe(EXE_NAME);
    }
  });

  it('buildStaging renames the real cargo binary to the friendly EXE name', () => {
    // 模拟真实构建产物：名为 palworld-server-manager.exe 的 stub。
    const licenseRoot = join(work, 'license');
    mkdirSync(licenseRoot, { recursive: true });
    makeLicenseRoot(licenseRoot);
    const realExe = join(work, CARGO_BIN_EXE);
    writeFileSync(realExe, 'real-cargo-exe-bytes');
    const staging = join(work, 'staging');

    buildStaging(realExe, { stagingRoot: staging, version: '1.0.0', licenseRoot });

    // staging 内应改名为友好名 Palworld Server Manager.exe。
    expect(existsSync(join(staging, EXE_NAME))).toBe(true);
    expect(existsSync(join(staging, CARGO_BIN_EXE))).toBe(false);
  });

  it('packageAll locates the real cargo binary from a fake target dir', () => {
    // 端到端：在临时 target/release 下放真实产物名 stub，让 packageAll 自动定位。
    const fakeTarget = join(work, 'target');
    mkdirSync(join(fakeTarget, 'release'), { recursive: true });
    const realExe = join(fakeTarget, 'release', CARGO_BIN_EXE);
    writeFileSync(realExe, 'real-cargo-exe-bytes');

    // 用 PALWORLD_CARGO_TARGET_DIR 指向 fakeTarget，让 buildExeCandidates 命中。
    const prev = process.env.PALWORLD_CARGO_TARGET_DIR;
    process.env.PALWORLD_CARGO_TARGET_DIR = fakeTarget;
    try {
      // 父仓库 LICENSE 真实存在，packageAll 会用它。
      const outDir = join(work, 'dist-portable');
      const result = packageAll({ exe: null, out: outDir, version: '1.0.0' });
      expect(existsSync(result.zipPath)).toBe(true);
      expect(existsSync(join(dirname(result.zipPath), 'manifest.json'))).toBe(true);
      // staging 已被清理（原子发布后 tmpDir 已 rename 为 outDir，内部无 staging 残留）。
      expect(existsSync(join(outDir, PORTABLE_FOLDER_NAME))).toBe(false);
    } finally {
      if (prev === undefined) delete process.env.PALWORLD_CARGO_TARGET_DIR;
      else process.env.PALWORLD_CARGO_TARGET_DIR = prev;
    }
  });

  it('packageAll failure leaves outDir untouched with no fake artifacts', () => {
    // 失败路径：传入不存在的 --exe，buildStaging 抛 EXE_MISSING。
    // outDir 不应被创建，outParent 不应残留 .tmp- 临时目录。
    const outDir = join(work, 'dist-portable');
    const outParent = dirname(outDir);
    expect(() =>
      packageAll({ exe: join(work, 'does-not-exist.exe'), out: outDir, version: '1.0.0' })
    ).toThrowError(/EXE|找不到 EXE|找不到/);

    // outDir 从未被创建（原子发布：仅成功才 rename）。
    expect(existsSync(outDir)).toBe(false);
    // outParent 不应残留任何 .dist-portable.tmp-* 临时目录。
    const tmpLeftovers = readdirSync(outParent).filter((n) =>
      /^\.dist-portable\.tmp-/.test(n)
    );
    expect(tmpLeftovers.length).toBe(0);
  });

  it('uniqueVersionedDir never returns an existing directory and handles collisions', () => {
    // 「不删除既有发布物」约束的核心：uniqueVersionedDir 绝不返回已存在的目录。
    const outDir = join(work, 'dist-portable');
    mkdirSync(outDir, { recursive: true });

    // 空目录 → 返回的路径不存在。
    const first = uniqueVersionedDir(outDir, '1.0.0');
    expect(existsSync(first)).toBe(false);
    expect(basename(first)).toMatch(/^v1\.0\.0-\d{8}-\d{6}$/);

    // 创建该目录后，同一秒内再次调用应返回带计数器后缀的不同路径。
    mkdirSync(first, { recursive: true });
    const second = uniqueVersionedDir(outDir, '1.0.0');
    expect(second).not.toBe(first);
    expect(existsSync(second)).toBe(false);
  });

  it('releaseToVersionedDir rename failure leaves existing artifacts and latest.json intact', () => {
    // P0 安全约束：发布阶段 rename 失败（杀毒/权限/占用）时，既有发布物与 latest 指针
    // 必须完全不受影响。releaseToVersionedDir 不预删除任何东西，rename 到已存在的
    // 非空目录会抛错——验证此路径下旧产物原样保留。
    const outDir = join(work, 'dist-portable');
    mkdirSync(outDir, { recursive: true });

    // 既有发布物：一个版本化目录 + latest.json 指针。
    const existingDir = join(outDir, 'v1.0.0-20240101-120000');
    mkdirSync(existingDir, { recursive: true });
    writeFileSync(join(existingDir, 'old.zip'), 'old-zip-bytes');
    writeFileSync(join(existingDir, 'old.zip.sha256'), 'oldhash  old.zip');
    writeFileSync(join(existingDir, 'manifest.json'), '{"version":"1.0.0","old":true}');
    const oldLatest = {
      version: '1.0.0',
      built_at: '2024-01-01T12:00:00.000Z',
      dir: 'v1.0.0-20240101-120000',
      zip: 'old.zip',
      sha256: 'oldhash',
      sha256_file: 'old.zip.sha256',
      manifest: 'manifest.json',
    };
    writeFileSync(join(outDir, 'latest.json'), JSON.stringify(oldLatest, null, 2));

    // 新构建的 tmpDir（待发布）。
    const tmpDir = join(work, 'new-release-tmp');
    mkdirSync(tmpDir, { recursive: true });
    writeFileSync(join(tmpDir, 'new.zip'), 'new-zip-bytes');

    // 让 versionedDir 指向已存在的非空目录 → renameSync 抛错（releaseToVersionedDir 不预删除）。
    expect(() =>
      releaseToVersionedDir(tmpDir, outDir, existingDir, {
        version: '1.0.1',
        built_at: '2024-01-02T12:00:00.000Z',
        dir: 'v1.0.0-20240101-120000',
        zip: 'new.zip',
        sha256: 'newhash',
        sha256_file: 'new.zip.sha256',
        manifest: 'manifest.json',
      })
    ).toThrow();

    // 既有版本化目录内容未变（未被覆盖/删除）。
    expect(readFileSync(join(existingDir, 'old.zip'), 'utf8')).toBe('old-zip-bytes');
    expect(readFileSync(join(existingDir, 'old.zip.sha256'), 'utf8')).toBe('oldhash  old.zip');
    expect(readFileSync(join(existingDir, 'manifest.json'), 'utf8')).toBe('{"version":"1.0.0","old":true}');

    // latest.json 指针未变（latest 更新在 rename 之后，rename 失败则不触及）。
    const latest = JSON.parse(readFileSync(join(outDir, 'latest.json'), 'utf8'));
    expect(latest).toEqual(oldLatest);

    // outDir 下应只有那一个既有版本化目录 + latest.json，没有任何新产物残留。
    const entries = readdirSync(outDir).sort();
    expect(entries).toEqual(['latest.json', 'v1.0.0-20240101-120000']);

    // tmpDir 由调用方负责清理，内容未被改动。
    expect(readFileSync(join(tmpDir, 'new.zip'), 'utf8')).toBe('new-zip-bytes');
  });

  it('packageAll successful new release preserves old versioned dirs and updates latest.json', () => {
    // 端到端「不删除既有发布物」：已有旧版本时，新成功发布应新增一个版本化目录、
    // 保留旧目录原样，并把 latest.json 指针原子更新到新版本。
    const fakeTarget = join(work, 'target');
    mkdirSync(join(fakeTarget, 'release'), { recursive: true });
    const realExe = join(fakeTarget, 'release', CARGO_BIN_EXE);
    writeFileSync(realExe, 'real-cargo-exe-bytes');

    const outDir = join(work, 'dist-portable');
    mkdirSync(outDir, { recursive: true });

    // 既有旧版本发布物。
    const oldDir = join(outDir, 'v1.0.0-20240101-120000');
    mkdirSync(oldDir, { recursive: true });
    writeFileSync(join(oldDir, 'old.zip'), 'old-zip-bytes');
    const oldLatest = {
      version: '1.0.0',
      built_at: '2024-01-01T12:00:00.000Z',
      dir: 'v1.0.0-20240101-120000',
      zip: 'old.zip',
      sha256: 'oldhash',
      sha256_file: 'old.zip.sha256',
      manifest: 'manifest.json',
    };
    writeFileSync(join(outDir, 'latest.json'), JSON.stringify(oldLatest, null, 2));

    const prev = process.env.PALWORLD_CARGO_TARGET_DIR;
    process.env.PALWORLD_CARGO_TARGET_DIR = fakeTarget;
    try {
      const result = packageAll({ exe: null, out: outDir, version: '1.0.1' });
      // 新版本化目录已创建，且与旧目录不同。
      expect(existsSync(result.versionedDir)).toBe(true);
      expect(result.versionedDir).not.toBe(oldDir);
      expect(existsSync(result.zipPath)).toBe(true);

      // 旧版本化目录原样保留（未被删除/覆盖）。
      expect(existsSync(oldDir)).toBe(true);
      expect(readFileSync(join(oldDir, 'old.zip'), 'utf8')).toBe('old-zip-bytes');

      // latest.json 已原子更新到新版本。
      const latest = JSON.parse(readFileSync(join(outDir, 'latest.json'), 'utf8'));
      expect(latest.version).toBe('1.0.1');
      expect(latest.dir).toBe(basename(result.versionedDir));
      expect(latest.sha256).toBe(result.sha256);

      // outDir 下应同时存在新旧两个版本化目录 + latest.json。
      const entries = readdirSync(outDir).sort();
      expect(entries).toContain('v1.0.0-20240101-120000');
      expect(entries).toContain(basename(result.versionedDir));
      expect(entries).toContain('latest.json');
    } finally {
      if (prev === undefined) delete process.env.PALWORLD_CARGO_TARGET_DIR;
      else process.env.PALWORLD_CARGO_TARGET_DIR = prev;
    }
  });
});

describe('cargo tool PATH', () => {
  it('includes the current Node directory so tauri.cmd can launch node after reboot', () => {
    const path = buildToolPath({
      projectRoot: 'F:\\app',
      nodeExecutable: 'C:\\bundled-node\\node.exe',
      currentPath: 'C:\\Windows\\System32',
      delimiter: ';',
    });

    expect(path.split(';')).toEqual([
      'F:\\app\\node_modules\\.bin',
      'C:\\bundled-node',
      'C:\\Windows\\System32',
    ]);
  });
});

describe('portable release repository layout', () => {
  it('loads release notices from the Palworld application directory', () => {
    const source = readFileSync(join(process.cwd(), 'scripts', 'package-portable.mjs'), 'utf8');

    expect(source).toContain('licenseRoot: PROJECT_ROOT')
    expect(source).not.toContain("licenseRoot: resolve(PROJECT_ROOT, '..')")
  });

  it('builds the frontend explicitly before invoking the portable Tauri config', () => {
    const packageJson = JSON.parse(readFileSync(join(process.cwd(), 'package.json'), 'utf8'));
    const script = packageJson.scripts['tauri:build:portable'];
    const config = JSON.parse(
      readFileSync(join(process.cwd(), 'scripts', 'tauri-portable.conf.json'), 'utf8'),
    );

    expect(script).toContain('node node_modules/vite/bin/vite.js build')
    expect(script).toContain('--config scripts/tauri-portable.conf.json')
    expect(config.build.beforeBuildCommand).toBe('')
  });
});
