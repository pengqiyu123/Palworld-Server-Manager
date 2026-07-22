<#
.SYNOPSIS
  Check whether Microsoft WebView2 Runtime is installed
.DESCRIPTION
  Reads registry keys under HKLM/HKCU EdgeUpdate Clients to verify WebView2 Runtime presence.
  Also checks the Microsoft Edge browser binary at its default install path as a fallback indicator.
  Prints human-readable summary; exits 0 if WebView2 Runtime is found, 1 otherwise.
.OUTPUTS
  Console output only (use scripts/health-check.ps1 for JSON report)
.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts/check-webview2.ps1
#>

$paths = @(
    "HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}",
    "HKLM:\SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}",
    "HKCU:\SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"
)
$found = $false
foreach ($p in $paths) {
    if (Test-Path $p) {
        $props = Get-ItemProperty $p -ErrorAction SilentlyContinue
        Write-Host ("Found at: " + $p)
        Write-Host ("  pv (version): " + $props.pv)
        Write-Host ("  name: " + $props.name)
        $found = $true
    }
}

if (-not $found) {
    Write-Host "WebView2 Runtime NOT found in registry"
}

# Also check edge browser
$edgePath = "C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"
if (Test-Path $edgePath) {
    $ver = (Get-Item $edgePath).VersionInfo.FileVersion
    Write-Host ("Edge browser: " + $edgePath + "  ver=" + $ver)
} else {
    Write-Host "Edge browser not found at default path"
}

# Check process details - useful for verifying the app is running
Write-Host ""
Write-Host "--- Process details ---"
Get-Process -Name 'palworld-server-manager' -ErrorAction SilentlyContinue | Format-List ProcessName, Id, MainWindowHandle, MainWindowTitle, StartTime, Responding

# Exit code: 0 = found, 1 = not found
if ($found) { exit 0 } else { exit 1 }
