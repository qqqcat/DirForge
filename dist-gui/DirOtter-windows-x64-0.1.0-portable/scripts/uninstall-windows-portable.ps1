param(
    [string]$InstallDir = (Join-Path $env:LOCALAPPDATA "Programs\DirOtter")
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$startMenuShortcut = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs\DirOtter.lnk"
$desktopShortcut = Join-Path ([Environment]::GetFolderPath("Desktop")) "DirOtter.lnk"

Get-Process -Name DirOtter -ErrorAction SilentlyContinue | Stop-Process -Force

foreach ($shortcut in @($startMenuShortcut, $desktopShortcut)) {
    if (Test-Path $shortcut) {
        Remove-Item -LiteralPath $shortcut -Force
    }
}

if (Test-Path $InstallDir) {
    Remove-Item -LiteralPath $InstallDir -Recurse -Force
}

Write-Host "Removed DirOtter from $InstallDir"
