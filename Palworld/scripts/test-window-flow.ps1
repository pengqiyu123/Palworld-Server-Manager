<#
.SYNOPSIS
  端到端测试脚本 - 启动 npm run tauri:dev → 等待 → 验证窗口 → 终止进程
.DESCRIPTION
  自动化测试流程：
  1. 后台启动 npm run tauri:dev
  2. 监听 stdout 等待 "Running target\debug\palworld-server-manager.exe"
  3. 再等待 10 秒确保窗口完全初始化（含第二层 Win32 修复的 800ms 延迟）
  4. 调用 verify-window.ps1
  5. 解析 JSON 输出判断 status
  6. 终止 npm run tauri:dev 进程
.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts/test-window-flow.ps1
#>

$ErrorActionPreference = "Continue"

$ProjectRoot = Split-Path $PSScriptRoot -Parent
$VerifyScript = Join-Path $PSScriptRoot "verify-window.ps1"

Write-Host "=== 端到端测试流程 ===" -ForegroundColor Cyan
Write-Host "项目根目录: $ProjectRoot"
Write-Host ""

# 1. 启动 npm run tauri:dev（后台）
Write-Host "[1/5] 启动 npm run tauri:dev ..." -ForegroundColor Yellow
$job = Start-Job -ScriptBlock {
    param($root)
    Set-Location $root
    npm run tauri:dev 2>&1
} -ArgumentList $ProjectRoot

# 2. 监听 stdout 等待应用启动（最多 5 分钟）
Write-Host "[2/5] 等待 Rust 编译 + 应用启动 ..." -ForegroundColor Yellow
$maxWait = 300  # 5 分钟
$waited = 0
$started = $false

while ($waited -lt $maxWait) {
    $output = Receive-Job -Job $job -Keep
    if ($output -match "Running target.*palworld-server-manager\.exe") {
        $started = $true
        Write-Host "检测到应用已启动（耗时 ${waited}s）" -ForegroundColor Green
        break
    }
    Start-Sleep -Seconds 2
    $waited += 2

    # 每 30 秒打印进度
    if ($waited % 30 -eq 0) {
        Write-Host "  已等待 ${waited}s ..."
    }
}

if (-not $started) {
    Write-Host "[FAILED] 等待 ${maxWait}s 后仍未检测到应用启动" -ForegroundColor Red
    Stop-Job -Job $job -Force
    Remove-Job -Job $job -Force
    exit 1
}

# 3. 再等待 10 秒确保窗口完全初始化
Write-Host "[3/5] 等待 10s 让窗口初始化完成 ..." -ForegroundColor Yellow
Start-Sleep -Seconds 10

# 4. 调用 verify-window.ps1
Write-Host "[4/5] 调用 verify-window.ps1 ..." -ForegroundColor Yellow
$verifyOutput = & powershell -ExecutionPolicy Bypass -File $VerifyScript
$verifyExit = $LASTEXITCODE

Write-Host ""
Write-Host "verify-window.ps1 输出:" -ForegroundColor Cyan
$verifyOutput | ForEach-Object { Write-Host "  $_" }

# 提取最后一行 JSON
$jsonLine = ($verifyOutput | Where-Object { $_ -match '^\{"status"' } | Select-Object -Last 1)
if ($jsonLine) {
    try {
        $report = $jsonLine | ConvertFrom-Json
        Write-Host ""
        Write-Host "解析结果:" -ForegroundColor Cyan
        Write-Host "  status: $($report.status)"
        Write-Host "  hwnd: $($report.hwnd)"
        Write-Host "  size: $($report.width)x$($report.height)"
        if ($report.screenshot) {
            Write-Host "  screenshot: $($report.screenshot)"
        }
    } catch {
        Write-Host "JSON 解析失败: $_" -ForegroundColor Red
    }
}

# 5. 终止 npm run tauri:dev
Write-Host ""
Write-Host "[5/5] 终止 npm run tauri:dev ..." -ForegroundColor Yellow
Stop-Job -Job $job -Force
Remove-Job -Job $job -Force

# 额外清理：杀掉残留的 palworld-server-manager.exe 进程
Get-Process -Name 'palworld-server-manager' -ErrorAction SilentlyContinue | ForEach-Object {
    Write-Host "清理残留进程 PID=$($_.Id)"
    Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue
}

Write-Host ""
if ($verifyExit -eq 0) {
    Write-Host "=== 测试通过 ===" -ForegroundColor Green
    exit 0
} else {
    Write-Host "=== 测试失败 ===" -ForegroundColor Red
    exit 1
}
