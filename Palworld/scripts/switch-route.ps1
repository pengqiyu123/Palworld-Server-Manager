<#
.SYNOPSIS
  Switch frontend route via file-trigger mechanism + capture screenshot
.DESCRIPTION
  Writes route path to Palworld\.route-switch file (polled by Rust backend),
  waits for route transition, captures screenshot.
.PARAMETER RouteName
  One of: dashboard / config / network / rcon / troubleshoot
#>

param(
    [Parameter(Mandatory=$true)]
    [ValidateSet("dashboard","config","network","rcon","troubleshoot")]
    [string]$RouteName,

    [string]$ProcessName = "palworld-server-manager",
    [string]$WindowTitle = "Palworld Server Manager",
    [string]$TriggerFile = "",
    [string]$ScreenshotDir = "",
    [int]$WaitSeconds = 2
)

# Resolve default paths to absolute (avoid issues with $PSScriptRoot in subshells)
# Note: Rust backend (cargo run) uses src-tauri/ as working directory, so trigger file
# must be placed there for the polling thread to find it.
if (-not $TriggerFile) {
    $TriggerFile = [System.IO.Path]::GetFullPath([System.IO.Path]::Combine($PSScriptRoot, "..", "src-tauri", ".route-switch"))
}
if (-not $ScreenshotDir) {
    $ScreenshotDir = [System.IO.Path]::GetFullPath([System.IO.Path]::Combine($PSScriptRoot, "..", "screenshots"))
}

$ErrorActionPreference = "Stop"

Add-Type -AssemblyName System.Drawing

# ---------- Win32 API ----------
Add-Type @"
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;

public class Win32Helper2 {
    public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumWindowsProc lpEnumFunc, IntPtr lParam);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr hWnd, System.Text.StringBuilder lpString, int nMaxCount);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowTextLengthW(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT lpRect);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint lpdwProcessId);
    [DllImport("user32.dll")] public static extern IntPtr GetShellWindow();

    [StructLayout(LayoutKind.Sequential)]
    public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }

    public const int SW_RESTORE = 9;

    public static List<IntPtr> FindWindowsByTitle(string titlePart) {
        var result = new List<IntPtr>();
        EnumWindows((hWnd, lParam) => {
            if (!IsWindowVisible(hWnd)) return true;
            if (hWnd == GetShellWindow()) return true;
            int len = GetWindowTextLengthW(hWnd);
            if (len <= 0) return true;
            var sb = new System.Text.StringBuilder(len + 1);
            GetWindowTextW(hWnd, sb, sb.Capacity);
            if (sb.ToString().IndexOf(titlePart, StringComparison.OrdinalIgnoreCase) >= 0) {
                result.Add(hWnd);
            }
            return true;
        }, IntPtr.Zero);
        return result;
    }

    public static List<IntPtr> FindVisibleWindowsByPid(uint pid) {
        var result = new List<IntPtr>();
        EnumWindows((hWnd, lParam) => {
            if (!IsWindowVisible(hWnd)) return true;
            if (hWnd == GetShellWindow()) return true;
            uint windowPid;
            GetWindowThreadProcessId(hWnd, out windowPid);
            if (windowPid == pid) result.Add(hWnd);
            return true;
        }, IntPtr.Zero);
        return result;
    }
}
"@

Write-Host "=== Switch Route: $RouteName ===" -ForegroundColor Cyan

# 1. Find main window (by title first, fallback to process)
$windows = [Win32Helper2]::FindWindowsByTitle($WindowTitle)
$matchMode = "title"

if ($windows.Count -eq 0) {
    Write-Host "Title lookup failed, falling back to process '$ProcessName'..." -ForegroundColor Yellow
    $proc = Get-Process -Name $ProcessName -ErrorAction SilentlyContinue | Select-Object -First 1
    if (-not $proc) {
        Write-Host "ERROR: Process '$ProcessName' not running" -ForegroundColor Red
        exit 1
    }
    $windows = [Win32Helper2]::FindVisibleWindowsByPid([uint32]$proc.Id)
    $matchMode = "pid"
    if ($windows.Count -eq 0) {
        Write-Host "ERROR: No visible windows for PID=$($proc.Id)" -ForegroundColor Red
        exit 1
    }
}

# Pick largest window
$mainHwnd = [IntPtr]::Zero
$bestSize = 0
foreach ($h in $windows) {
    $r = New-Object Win32Helper2+RECT
    if ([Win32Helper2]::GetWindowRect($h, [ref]$r)) {
        $size = ($r.Right - $r.Left) * ($r.Bottom - $r.Top)
        if ($size -gt $bestSize) {
            $bestSize = $size
            $mainHwnd = $h
        }
    }
}

if ($mainHwnd -eq [IntPtr]::Zero) {
    Write-Host "ERROR: Cannot determine main window" -ForegroundColor Red
    exit 1
}

Write-Host ("Main window: HWND=0x{0:x} (mode={1})" -f $mainHwnd.ToInt64(), $matchMode)

# 2. Ensure window is foreground
[Win32Helper2]::ShowWindow($mainHwnd, [Win32Helper2]::SW_RESTORE) | Out-Null
[Win32Helper2]::SetForegroundWindow($mainHwnd) | Out-Null
Start-Sleep -Milliseconds 300

# 3. Write trigger file
$triggerDir = Split-Path $TriggerFile -Parent
if (-not (Test-Path $triggerDir)) {
    New-Item -ItemType Directory -Path $triggerDir -Force | Out-Null
}
$routePath = "/$RouteName"
Set-Content -Path $TriggerFile -Value $routePath -NoNewline -Encoding UTF8
Write-Host "Trigger file written: $TriggerFile (route=$routePath)"

# 4. Wait for route transition
Start-Sleep -Seconds $WaitSeconds

# 5. Capture screenshot
$rect = New-Object Win32Helper2+RECT
[Win32Helper2]::GetWindowRect($mainHwnd, [ref]$rect) | Out-Null
$w = $rect.Right - $rect.Left
$h = $rect.Bottom - $rect.Top

if ($w -le 0 -or $h -le 0) {
    Write-Host "ERROR: Invalid window size ${w}x${h}" -ForegroundColor Red
    exit 1
}

$bmp = New-Object System.Drawing.Bitmap $w, $h
$graphics = [System.Drawing.Graphics]::FromImage($bmp)
$graphics.CopyFromScreen($rect.Left, $rect.Top, 0, 0, (New-Object System.Drawing.Size($w, $h)))

$timestamp = Get-Date -Format "yyyyMMdd-HHmmss"
if (-not (Test-Path $ScreenshotDir)) {
    New-Item -ItemType Directory -Path $ScreenshotDir -Force | Out-Null
}
$outPath = Join-Path $ScreenshotDir "route-$RouteName-$timestamp.png"

$bmp.Save($outPath, [System.Drawing.Imaging.ImageFormat]::Png)
$graphics.Dispose()
$bmp.Dispose()

$fileItem = Get-Item $outPath
Write-Host ""
Write-Host "=== Result ===" -ForegroundColor Cyan
Write-Host "Route: $RouteName"
Write-Host "Screenshot: $outPath"
Write-Host "Size: $($fileItem.Length) bytes"
Write-Host "Dimensions: ${w}x${h}"

$report = @{
    route = $RouteName
    hwnd = ("0x{0:x}" -f $mainHwnd.ToInt64())
    width = $w
    height = $h
    screenshot = $outPath
    size_bytes = $fileItem.Length
} | ConvertTo-Json -Compress

Write-Host $report
exit 0
