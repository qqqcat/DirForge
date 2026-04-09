param(
    [string]$InstallDir = (Join-Path $env:LOCALAPPDATA "Programs\DirOtter"),
    [switch]$CreateDesktopShortcut
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function New-Shortcut {
    param(
        [string]$ShortcutPath,
        [string]$TargetPath,
        [string]$WorkingDirectory
    )

    $shell = New-Object -ComObject WScript.Shell
    $shortcut = $shell.CreateShortcut($ShortcutPath)
    $shortcut.TargetPath = $TargetPath
    $shortcut.WorkingDirectory = $WorkingDirectory
    $shortcut.IconLocation = $TargetPath
    $shortcut.Save()
}

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$packageRoot = (Resolve-Path (Join-Path $scriptDir "..")).Path
$sourceExe = Join-Path $packageRoot "DirOtter.exe"

if (-not (Test-Path $sourceExe)) {
    throw "DirOtter.exe was not found next to the installer script. Extract the release archive first."
}

New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
Copy-Item -Path (Join-Path $packageRoot "*") -Destination $InstallDir -Recurse -Force

$installedExe = Join-Path $InstallDir "DirOtter.exe"
$startMenuDir = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs"
$startMenuShortcut = Join-Path $startMenuDir "DirOtter.lnk"

New-Item -ItemType Directory -Force -Path $startMenuDir | Out-Null
New-Shortcut -ShortcutPath $startMenuShortcut -TargetPath $installedExe -WorkingDirectory $InstallDir

if ($CreateDesktopShortcut) {
    $desktopShortcut = Join-Path ([Environment]::GetFolderPath("Desktop")) "DirOtter.lnk"
    New-Shortcut -ShortcutPath $desktopShortcut -TargetPath $installedExe -WorkingDirectory $InstallDir
}

Write-Host "Installed DirOtter to $InstallDir"
Write-Host "Start menu shortcut: $startMenuShortcut"
