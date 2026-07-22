Add-Type @"
using System;
using System.Runtime.InteropServices;
public class WinHelper {
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int n);
  [StructLayout(LayoutKind.Sequential)]
  public struct RECT { public int L, T, R, B; }
}
"@

$proc = Get-Process -Name 'palworld-server-manager' -ErrorAction SilentlyContinue
if (-not $proc) {
  Write-Host "ERROR: Process 'palworld-server-manager' not found"
  exit 1
}

$handle = $proc.MainWindowHandle
Write-Host "PID: $($proc.Id)"
Write-Host "MainWindowHandle: $handle"
Write-Host "MainWindowTitle: '$($proc.MainWindowTitle)'"

if ($handle -eq [IntPtr]::Zero) {
  Write-Host "ERROR: MainWindowHandle is zero"
  exit 1
}

# Restore if minimized, then bring to front
[WinHelper]::ShowWindow($handle, 9) | Out-Null   # SW_RESTORE
Start-Sleep -Milliseconds 200
[WinHelper]::ShowWindow($handle, 5) | Out-Null   # SW_SHOW
Start-Sleep -Milliseconds 200
[WinHelper]::SetForegroundWindow($handle) | Out-Null
Start-Sleep -Milliseconds 800

$rect = New-Object WinHelper+RECT
[WinHelper]::GetWindowRect($handle, [ref]$rect) | Out-Null

$width = $rect.R - $rect.L
$height = $rect.B - $rect.T

Write-Host "Window Rect: L=$($rect.L) T=$($rect.T) R=$($rect.R) B=$($rect.B)"
Write-Host "Window Size: ${width}x${height}"

if ($width -ge 1000 -and $height -ge 700) {
  Write-Host "STATUS: SUCCESS - Window size is correct (>= 1000x700)"
} elseif ($width -gt 0 -and $height -gt 0) {
  Write-Host "STATUS: WARNING - Window exists but size is ${width}x${height}"
} else {
  Write-Host "STATUS: FAILED - Window size is ${width}x${height}"
}
