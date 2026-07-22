$searchPaths = @(
    "F:\study\Palworld-Server-Manager\Palworld\.route-switch-probe.txt",
    "F:\study\Palworld-Server-Manager\.route-switch-probe.txt",
    "F:\study\.route-switch-probe.txt",
    "F:\study\Palworld-Server-Manager\Palworld\src-tauri\.route-switch-probe.txt",
    "F:\study\Palworld-Server-Manager\Palworld\src-tauri\target\debug\.route-switch-probe.txt"
)
foreach ($p in $searchPaths) {
    if (Test-Path $p) {
        Write-Host "FOUND: $p" -ForegroundColor Green
        Get-Content $p
    } else {
        Write-Host "not found: $p"
    }
}
Write-Host ""
Write-Host "Searching F:\study for probe file..."
Get-ChildItem -Path "F:\study" -Filter ".route-switch-probe.txt" -Recurse -ErrorAction SilentlyContinue -Force | Select-Object FullName
