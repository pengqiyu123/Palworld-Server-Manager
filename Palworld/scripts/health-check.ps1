<#
.SYNOPSIS
  Comprehensive health check for Palworld Server Manager
.DESCRIPTION
  Runs a battery of diagnostic checks:
    1. WebView2 Runtime installation (registry)
    2. Port 5222 occupancy (netstat)
    3. Firewall rules for PalworldServer (PowerShell NetSecurity)
    4. App main window size (calls verify-window.ps1)
    5. Screenshot saved to screenshots/health-YYYYMMDD-HHmmss.png
  Outputs a JSON report and an exit code:
    0 = OK     (all checks passed)
    1 = WARN   (warnings present, app still usable)
    2 = ERROR   (severe error, app likely unusable)
.PARAMETER CheckPort
  Port number to check for occupancy (default 5222)
.PARAMETER ProcessName
  Process name of the app (default palworld-server-manager)
.PARAMETER WindowTitle
  Expected main window title (default "Palworld Server Manager")
.PARAMETER ScreenshotDir
  Directory where the screenshot will be saved (default ../screenshots)
.PARAMETER VerifyWindowScript
  Path to verify-window.ps1 (default ./verify-window.ps1)
.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts/health-check.ps1
#>

param(
    [int]$CheckPort = 5222,
    [string]$ProcessName = "palworld-server-manager",
    [string]$WindowTitle = "Palworld Server Manager",
    [string]$ScreenshotDir = "",
    [string]$VerifyWindowScript = ""
)

$ErrorActionPreference = "Continue"

# Resolve default paths to absolute
if (-not $ScreenshotDir) {
    $ScreenshotDir = [System.IO.Path]::GetFullPath([System.IO.Path]::Combine($PSScriptRoot, "..", "screenshots"))
}
if (-not $VerifyWindowScript) {
    $VerifyWindowScript = Join-Path $PSScriptRoot "verify-window.ps1"
}

# Ensure screenshots dir exists
if (-not (Test-Path $ScreenshotDir)) {
    New-Item -ItemType Directory -Path $ScreenshotDir -Force | Out-Null
}

# ---------- helpers ----------
function Get-IsoTimestamp {
    return (Get-Date).ToString("yyyy-MM-ddTHH:mm:ssK")
}

function Get-FileTimestamp {
    return (Get-Date).ToString("yyyyMMdd-HHmmss")
}

# ---------- check 1: WebView2 Runtime ----------
function Check-WebView2 {
    $paths = @(
        "HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}",
        "HKLM:\SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}",
        "HKCU:\SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"
    )
    foreach ($p in $paths) {
        if (Test-Path $p) {
            $props = Get-ItemProperty $p -ErrorAction SilentlyContinue
            $ver = $props.pv
            if ($ver) {
                return @{
                    status = "ok"
                    detail = "WebView2 Runtime installed (version $ver)"
                }
            }
        }
    }
    return @{
        status = "error"
        detail = "WebView2 Runtime NOT found in registry. Install from https://developer.microsoft.com/microsoft-edge/webview2/"
    }
}

# ---------- check 2: port occupancy ----------
function Check-Port {
    param([int]$Port)
    try {
        $connections = Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue
        if ($connections) {
            $owners = @()
            foreach ($c in $connections) {
                $proc = Get-Process -Id $c.OwningProcess -ErrorAction SilentlyContinue
                $procName = if ($proc) { $proc.ProcessName } else { "PID=$($c.OwningProcess)" }
                $owners += "$procName (PID=$($c.OwningProcess))"
            }
            $ownerStr = ($owners | Select-Object -Unique) -join ", "
            return @{
                status = "warn"
                detail = "Port $Port is LISTENING by: $ownerStr"
            }
        } else {
            return @{
                status = "ok"
                detail = "Port $Port is free (no listener)"
            }
        }
    } catch {
        return @{
            status = "warn"
            detail = "Cannot query port $Port (Get-NetTCPConnection unavailable): $($_.Exception.Message)"
        }
    }
}

# ---------- check 3: firewall rules ----------
function Check-Firewall {
    $rules = @()
    try {
        $rules = @(Get-NetFirewallRule -ErrorAction SilentlyContinue | Where-Object {
            $_.DisplayName -like "*Palworld*" -or
            $_.DisplayName -like "*PalServer*" -or
            $_.DisplayName -like "*PalworldServer*" -or
            $_.Name -like "*Palworld*" -or
            $_.Name -like "*PalServer*" -or
            $_.Name -like "*PalworldServer*"
        })
    } catch {
        return @{
            status = "warn"
            detail = "Cannot query firewall rules: $($_.Exception.Message)"
        }
    }

    if ($rules.Count -eq 0) {
        return @{
            status = "warn"
            detail = "No PalworldServer-related firewall rules found. Run app and use 'one-click allow' to add them."
        }
    }

    $names = ($rules | Select-Object -First 5 | ForEach-Object { $_.DisplayName }) -join ", "
    return @{
        status = "ok"
        detail = "Found $($rules.Count) firewall rule(s): $names"
    }
}

# ---------- check 4: window size (delegates to verify-window.ps1) ----------
function Check-Window {
    if (-not (Test-Path $VerifyWindowScript)) {
        return @{
            status = "warn"
            detail = "verify-window.ps1 not found at $VerifyWindowScript"
        }
    }

    try {
        $output = & powershell -ExecutionPolicy Bypass -File $VerifyWindowScript 2>&1
        $lastLine = ($output | Where-Object { $_ -match '^{.*}$' } | Select-Object -Last 1)
        if (-not $lastLine) {
            return @{
                status = "warn"
                detail = "verify-window.ps1 did not emit JSON. Output: $($output -join ' | ')"
            }
        }

        $wreport = $lastLine | ConvertFrom-Json
        $status = $wreport.status
        $w = $wreport.width
        $h = $wreport.height

        switch ($status) {
            "OK" {
                return @{
                    status = "ok"
                    detail = "Window OK ($w x $h)"
                    verify_report = $wreport
                }
            }
            "FIXED" {
                return @{
                    status = "warn"
                    detail = "Window was abnormal, auto-fixed to $w x $h"
                    verify_report = $wreport
                }
            }
            "FAILED" {
                return @{
                    status = "error"
                    detail = "Window size fix FAILED ($w x $h)"
                    verify_report = $wreport
                }
            }
            "NOT_FOUND" {
                return @{
                    status = "warn"
                    detail = "App window not found. Is the app running?"
                    verify_report = $wreport
                }
            }
            default {
                return @{
                    status = "warn"
                    detail = "Unknown verify-window status: $status"
                    verify_report = $wreport
                }
            }
        }
    } catch {
        return @{
            status = "error"
            detail = "verify-window.ps1 invocation failed: $($_.Exception.Message)"
        }
    }
}

# ---------- screenshot capture ----------
# Captures the largest visible top-level window of the target process.
# Falls back to a full-screen capture if the process or its window cannot be located.
function Take-Screenshot {
    param(
        [string]$ProcessNameMatch,
        [string]$OutputPath
    )

    Add-Type -AssemblyName System.Drawing
    Add-Type -AssemblyName System.Windows.Forms

    # Reuse Win32Helper if verify-window.ps1 already defined it; otherwise define our own.
    if (-not ('Win32Helper' -as [type])) {
        Add-Type @"
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;

public class Win32Helper {
    public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumWindowsProc lpEnumFunc, IntPtr lParam);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr hWnd, System.Text.StringBuilder lpString, int nMaxCount);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowTextLengthW(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT lpRect);
    [DllImport("user32.dll")] public static extern bool MoveWindow(IntPtr hWnd, int X, int Y, int nWidth, int nHeight, bool bRepaint);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern bool SetWindowTextW(IntPtr hWnd, string lpString);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint lpdwProcessId);
    [DllImport("user32.dll")] public static extern IntPtr GetShellWindow();
    [DllImport("user32.dll")] public static extern int GetSystemMetrics(int nIndex);

    [StructLayout(LayoutKind.Sequential)]
    public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }

    public const int SM_CXSCREEN = 0;
    public const int SM_CYSCREEN = 1;
    public const int SW_RESTORE = 9;
    public const int SW_SHOW = 5;

    public static List<IntPtr> FindVisibleWindowsByPid(uint pid) {
        var result = new List<IntPtr>();
        EnumWindows((hWnd, lParam) => {
            if (!IsWindowVisible(hWnd)) return true;
            if (hWnd == GetShellWindow()) return true;
            uint windowPid;
            GetWindowThreadProcessId(hWnd, out windowPid);
            if (windowPid == pid) {
                result.Add(hWnd);
            }
            return true;
        }, IntPtr.Zero);
        return result;
    }

    public static bool TryGetSize(IntPtr hWnd, out int width, out int height, out int left, out int top) {
        RECT rect;
        if (GetWindowRect(hWnd, out rect)) {
            left = rect.Left; top = rect.Top;
            width = rect.Right - rect.Left;
            height = rect.Bottom - rect.Top;
            return true;
        }
        left = top = width = height = 0;
        return false;
    }
}
"@
    }

    $captureLeft = 0
    $captureTop = 0
    $captureW = 0
    $captureH = 0
    $captured = $false

    # Try to find the largest visible top-level window of the target process
    $proc = Get-Process -Name $ProcessNameMatch -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($proc) {
        $procId = [uint32]$proc.Id
        $windows = [Win32Helper]::FindVisibleWindowsByPid($procId)
        $bestArea = 0
        foreach ($h in $windows) {
            $w = 0; $ht = 0; $l = 0; $t = 0
            if ([Win32Helper]::TryGetSize($h, [ref]$w, [ref]$ht, [ref]$l, [ref]$t)) {
                $area = $w * $ht
                if ($area -gt $bestArea) {
                    $bestArea = $area
                    $captureLeft = $l
                    $captureTop = $t
                    $captureW = $w
                    $captureH = $ht
                }
            }
        }
        if ($bestArea -gt 0) {
            $captured = $true
        }
    }

    # Fallback: capture the primary screen
    if (-not $captured) {
        $captureW = [Win32Helper]::GetSystemMetrics([Win32Helper]::SM_CXSCREEN)
        $captureH = [Win32Helper]::GetSystemMetrics([Win32Helper]::SM_CYSCREEN)
        $captureLeft = 0
        $captureTop = 0
        $captured = ($captureW -gt 0 -and $captureH -gt 0)
    }

    if (-not $captured) { return $null }

    $bmp = New-Object System.Drawing.Bitmap $captureW, $captureH
    $graphics = [System.Drawing.Graphics]::FromImage($bmp)
    $graphics.CopyFromScreen($captureLeft, $captureTop, 0, 0, (New-Object System.Drawing.Size($captureW, $captureH)))
    $dir = Split-Path $OutputPath -Parent
    if (-not (Test-Path $dir)) { New-Item -ItemType Directory -Path $dir -Force | Out-Null }
    $bmp.Save($OutputPath, [System.Drawing.Imaging.ImageFormat]::Png)
    $graphics.Dispose()
    $bmp.Dispose()
    return $OutputPath
}

# ---------- main ----------
Write-Host "=== Palworld Server Manager Health Check ===" -ForegroundColor Cyan
Write-Host "Started at $(Get-IsoTimestamp)"
Write-Host ""

# Run checks
Write-Host "[1/4] Checking WebView2 Runtime..." -ForegroundColor Yellow
$webview2 = Check-WebView2
Write-Host "  -> $($webview2.status): $($webview2.detail)"

Write-Host "[2/4] Checking port $CheckPort..." -ForegroundColor Yellow
$port = Check-Port -Port $CheckPort
Write-Host "  -> $($port.status): $($port.detail)"

Write-Host "[3/4] Checking firewall rules..." -ForegroundColor Yellow
$firewall = Check-Firewall
Write-Host "  -> $($firewall.status): $($firewall.detail)"

Write-Host "[4/4] Checking app window size..." -ForegroundColor Yellow
$window = Check-Window
Write-Host "  -> $($window.status): $($window.detail)"

Write-Host ""
Write-Host "Capturing screenshot..." -ForegroundColor Yellow
$screenshotFile = Join-Path $ScreenshotDir ("health-" + (Get-FileTimestamp) + ".png")
$screenshotSaved = Take-Screenshot -ProcessNameMatch $ProcessName -OutputPath $screenshotFile
if ($screenshotSaved) {
    Write-Host "  -> saved: $screenshotSaved" -ForegroundColor Green
} else {
    Write-Host "  -> screenshot skipped (app not running or window not found)" -ForegroundColor Yellow
    $screenshotSaved = $null
}

# Compute overall status
# weight: error > warn > ok
$statusOrder = @{ "ok" = 0; "warn" = 1; "error" = 2 }
$checks = @(
    @{ key = "webview2";  title = "WebView2 Runtime"; status = $webview2.status; detail = $webview2.detail },
    @{ key = "port";      title = "Port $CheckPort";       status = $port.status;     detail = $port.detail }
    @{ key = "firewall";  title = "Firewall rules";    status = $firewall.status; detail = $firewall.detail }
    @{ key = "window";    title = "App window";        status = $window.status;   detail = $window.detail }
)

$worst = 0
foreach ($c in $checks) {
    $s = $statusOrder[$c.status]
    if ($s -gt $worst) { $worst = $s }
}

$overall = switch ($worst) {
    0 { "OK" }
    1 { "WARN" }
    2 { "ERROR" }
}

# Build JSON report (note: use ordered so timestamp comes first)
$report = [ordered]@{
    timestamp       = Get-IsoTimestamp
    overall_status  = $overall
    checks          = $checks
    screenshot      = $screenshotSaved
}

$json = $report | ConvertTo-Json -Depth 6
Write-Host ""
Write-Host "=== Health Check Report ===" -ForegroundColor Cyan
Write-Host $json

# Exit code
switch ($overall) {
    "OK"    { exit 0 }
    "WARN"  { exit 1 }
    "ERROR" { exit 2 }
}
