<#
.SYNOPSIS
  Window verification script - verify Palworld Server Manager main window size and capture screenshot
.DESCRIPTION
  Layer 3 of the triple-fallback strategy: takes over when Tauri's internal Win32 fix fails.
  Enumerates all top-level windows by title "Palworld Server Manager" (not relying on Process.MainWindowHandle),
  picks the largest matching window as the real app main window, checks size and auto-fixes, then captures screenshot.
  Fallback: if no window matches the title (title may be empty due to Tauri/WebView2 issue),
  falls back to filtering by process name and picks the largest visible top-level window of that process.
.OUTPUTS
  JSON report: {"status":"OK|FIXED|FAILED|NOT_FOUND","hwnd":"0x...","width":N,"height":N,"screenshot":"..."}
.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts/verify-window.ps1
#>

param(
    [string]$WindowTitle = "Palworld Server Manager",
    [string]$ProcessName = "palworld-server-manager",
    [int]$TargetWidth = 1200,
    [int]$TargetHeight = 800,
    [int]$Tolerance = 50,
    [string]$ScreenshotDir = "$PSScriptRoot\..\screenshots"
)

$ErrorActionPreference = "Stop"

# ---------- Win32 API registration ----------
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

    public static List<IntPtr> FindWindowsByTitle(string titlePart) {
        var result = new List<IntPtr>();
        EnumWindows((hWnd, lParam) => {
            if (!IsWindowVisible(hWnd)) return true;
            if (hWnd == GetShellWindow()) return true;

            int len = GetWindowTextLengthW(hWnd);
            if (len <= 0) return true;

            var sb = new System.Text.StringBuilder(len + 1);
            GetWindowTextW(hWnd, sb, sb.Capacity);
            string title = sb.ToString();
            if (title.IndexOf(titlePart, StringComparison.OrdinalIgnoreCase) >= 0) {
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

# ---------- Screenshot function ----------
function Take-Screenshot {
    param(
        [IntPtr]$Hwnd,
        [string]$OutputPath
    )

    Add-Type -AssemblyName System.Drawing
    Add-Type -AssemblyName System.Windows.Forms

    $rect = New-Object Win32Helper+RECT
    [Win32Helper]::GetWindowRect($Hwnd, [ref]$rect) | Out-Null
    $w = $rect.Right - $rect.Left
    $h = $rect.Bottom - $rect.Top

    if ($w -le 0 -or $h -le 0) {
        return $null
    }

    $bmp = New-Object System.Drawing.Bitmap $w, $h
    $graphics = [System.Drawing.Graphics]::FromImage($bmp)
    $graphics.CopyFromScreen($rect.Left, $rect.Top, 0, 0, (New-Object System.Drawing.Size($w, $h)))

    $dir = Split-Path $OutputPath -Parent
    if (-not (Test-Path $dir)) {
        New-Item -ItemType Directory -Path $dir -Force | Out-Null
    }

    $bmp.Save($OutputPath, [System.Drawing.Imaging.ImageFormat]::Png)
    $graphics.Dispose()
    $bmp.Dispose()

    return $OutputPath
}

# ---------- Main logic ----------

Write-Host "=== Palworld Server Manager Window Verification ===" -ForegroundColor Cyan
Write-Host "Target title: $WindowTitle"
Write-Host "Target process: $ProcessName"
Write-Host "Target size: ${TargetWidth}x${TargetHeight} (tolerance +-${Tolerance})"
Write-Host ""

# 1. Enumerate matching windows (by title first, fallback to process name)
$matches = [Win32Helper]::FindWindowsByTitle($WindowTitle)
$matchMode = "title"

if ($matches.Count -eq 0) {
    Write-Host "No window matches title '$WindowTitle', falling back to process name '$ProcessName'..." -ForegroundColor Yellow
    $proc = Get-Process -Name $ProcessName -ErrorAction SilentlyContinue | Select-Object -First 1
    if (-not $proc) {
        $report = @{
            status = "NOT_FOUND"
            message = "No process named '$ProcessName' is running"
        } | ConvertTo-Json -Compress
        Write-Host $report
        exit 1
    }
    $procId = [uint32]$proc.Id
    Write-Host "Process found: PID=$procId, enumerating its visible windows..."
    $matches = [Win32Helper]::FindVisibleWindowsByPid($procId)
    $matchMode = "pid"
    if ($matches.Count -eq 0) {
        $report = @{
            status = "NOT_FOUND"
            message = "Process '$ProcessName' (PID=$procId) has no visible top-level windows"
        } | ConvertTo-Json -Compress
        Write-Host $report
        exit 1
    }
}

Write-Host "Found $($matches.Count) matching window(s) via $matchMode mode:"

# 2. Pick the largest one as the real main window
$bestHwnd = [IntPtr]::Zero
$bestWidth = 0
$bestHeight = 0
$bestLeft = 0
$bestTop = 0

foreach ($h in $matches) {
    $w = 0; $ht = 0; $l = 0; $t = 0
    if ([Win32Helper]::TryGetSize($h, [ref]$w, [ref]$ht, [ref]$l, [ref]$t)) {
        Write-Host ("  HWND=0x{0:x}  size={1}x{2}  pos=({3},{4})" -f $h.ToInt64(), $w, $ht, $l, $t)
        if ($w -gt $bestWidth) {
            $bestHwnd = $h
            $bestWidth = $w
            $bestHeight = $ht
            $bestLeft = $l
            $bestTop = $t
        }
    }
}

Write-Host ""
Write-Host ("Selected main window: HWND=0x{0:x}  size={1}x{2}" -f $bestHwnd.ToInt64(), $bestWidth, $bestHeight) -ForegroundColor Yellow

# 3. Check size
function Test-SizeOk {
    param([int]$W, [int]$H)
    return ([math]::Abs($W - $TargetWidth) -le $Tolerance) -and ([math]::Abs($H - $TargetHeight) -le $Tolerance)
}

$status = "OK"
$needsFix = -not (Test-SizeOk $bestWidth $bestHeight)

if ($needsFix) {
    Write-Host "Size abnormal, attempting auto-fix (layer 3)..." -ForegroundColor Yellow

    $screenW = [Win32Helper]::GetSystemMetrics([Win32Helper]::SM_CXSCREEN)
    $screenH = [Win32Helper]::GetSystemMetrics([Win32Helper]::SM_CYSCREEN)
    $x = [int](($screenW - $TargetWidth) / 2)
    $y = [int](($screenH - $TargetHeight) / 2)
    if ($x -lt 0) { $x = 50 }
    if ($y -lt 0) { $y = 50 }

    Write-Host ("Calling MoveWindow HWND=0x{0:x}  pos=({1},{2})  size={3}x{4}" -f $bestHwnd.ToInt64(), $x, $y, $TargetWidth, $TargetHeight)

    [Win32Helper]::ShowWindow($bestHwnd, [Win32Helper]::SW_RESTORE) | Out-Null
    [Win32Helper]::MoveWindow($bestHwnd, $x, $y, $TargetWidth, $TargetHeight, $true) | Out-Null
    [Win32Helper]::SetForegroundWindow($bestHwnd) | Out-Null

    # Also set the window title (Tauri 2 may have failed to set it)
    if ($matchMode -eq "pid") {
        Write-Host "Setting window title to '$WindowTitle' (was empty)..."
        [Win32Helper]::SetWindowTextW($bestHwnd, $WindowTitle) | Out-Null
    }

    Start-Sleep -Seconds 2

    $w = 0; $ht = 0; $l = 0; $t = 0
    if ([Win32Helper]::TryGetSize($bestHwnd, [ref]$w, [ref]$ht, [ref]$l, [ref]$t)) {
        $bestWidth = $w
        $bestHeight = $ht
        $bestLeft = $l
        $bestTop = $t

        if (Test-SizeOk $bestWidth $bestHeight) {
            $status = "FIXED"
            Write-Host "Fix successful!" -ForegroundColor Green
        } else {
            $status = "FAILED"
            Write-Host "Fix failed, size still ${bestWidth}x${bestHeight}" -ForegroundColor Red
        }
    } else {
        $status = "FAILED"
        Write-Host "Cannot read window size after fix" -ForegroundColor Red
    }
} else {
    Write-Host "Size OK" -ForegroundColor Green
}

# 4. Screenshot
$timestamp = Get-Date -Format "yyyyMMdd-HHmmss"
$screenshotAbsPath = (Resolve-Path -LiteralPath (New-Item -ItemType Directory -Path $ScreenshotDir -Force).FullName).Path
$screenshotFull = Join-Path $screenshotAbsPath "window-$timestamp.png"

$saved = Take-Screenshot -Hwnd $bestHwnd -OutputPath $screenshotFull

if ($saved) {
    Write-Host "Screenshot saved: $screenshotFull" -ForegroundColor Green
} else {
    Write-Host "Screenshot failed" -ForegroundColor Red
    $screenshotFull = $null
}

# 5. Output JSON report
$report = [ordered]@{
    status = $status
    hwnd = ("0x{0:x}" -f $bestHwnd.ToInt64())
    width = $bestWidth
    height = $bestHeight
    screenshot = if ($screenshotFull) { $screenshotFull } else { $null }
}

$json = $report | ConvertTo-Json -Compress
Write-Host ""
Write-Host "=== Verification Report ===" -ForegroundColor Cyan
Write-Host $json

# 6. Exit code
if ($status -eq "OK" -or $status -eq "FIXED") {
    exit 0
} else {
    exit 1
}
