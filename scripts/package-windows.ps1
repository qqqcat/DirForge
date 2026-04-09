param(
    [string]$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path,
    [string]$Version,
    [ValidateSet("debug", "release")]
    [string]$Configuration = "release",
    [string]$BinaryPath,
    [string]$OutputRoot
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Get-WorkspaceVersion {
    param([string]$ManifestPath)

    $manifest = Get-Content -Raw -Path $ManifestPath
    if ($manifest -match '(?m)^\s*version\s*=\s*"(?<version>[^"]+)"\s*$') {
        return $Matches.version
    }

    throw "Unable to resolve version from $ManifestPath"
}

if (-not $Version) {
    if ($env:GITHUB_REF_TYPE -eq "tag" -and $env:GITHUB_REF_NAME) {
        $Version = $env:GITHUB_REF_NAME.TrimStart("v")
    } else {
        $Version = Get-WorkspaceVersion -ManifestPath (Join-Path $RepoRoot "Cargo.toml")
    }
}

if (-not $BinaryPath) {
    $BinaryPath = Join-Path $RepoRoot "target\$Configuration\dirotter-app.exe"
}

if (-not (Test-Path $BinaryPath)) {
    throw "Binary not found: $BinaryPath"
}
$BinaryPath = (Resolve-Path $BinaryPath).Path

if (-not $OutputRoot) {
    $OutputRoot = Join-Path $RepoRoot "dist"
}

$artifactName = "DirOtter-windows-x64-$Version-portable"
$stageDir = Join-Path $OutputRoot $artifactName
$docsDir = Join-Path $stageDir "docs"
$scriptsDir = Join-Path $stageDir "scripts"
$zipPath = Join-Path $OutputRoot "$artifactName.zip"
$checksumPath = "$zipPath.sha256.txt"

New-Item -ItemType Directory -Force -Path $OutputRoot | Out-Null
if (Test-Path $stageDir) {
    Remove-Item -LiteralPath $stageDir -Recurse -Force
}
if (Test-Path $zipPath) {
    Remove-Item -LiteralPath $zipPath -Force
}
if (Test-Path $checksumPath) {
    Remove-Item -LiteralPath $checksumPath -Force
}

New-Item -ItemType Directory -Force -Path $stageDir, $docsDir, $scriptsDir | Out-Null

$packagedExe = Join-Path $stageDir "DirOtter.exe"
Copy-Item -LiteralPath $BinaryPath -Destination $packagedExe

Copy-Item -LiteralPath (Join-Path $RepoRoot "README.md") -Destination (Join-Path $stageDir "README.md")
Copy-Item -LiteralPath (Join-Path $RepoRoot "docs\quickstart.md") -Destination (Join-Path $docsDir "quickstart.md")
Copy-Item -LiteralPath (Join-Path $RepoRoot "docs\dirotter-install-usage.md") -Destination (Join-Path $docsDir "dirotter-install-usage.md")
Copy-Item -LiteralPath (Join-Path $RepoRoot "scripts\install-windows-portable.ps1") -Destination (Join-Path $scriptsDir "install-windows-portable.ps1")
Copy-Item -LiteralPath (Join-Path $RepoRoot "scripts\uninstall-windows-portable.ps1") -Destination (Join-Path $scriptsDir "uninstall-windows-portable.ps1")

$signatureStatus = (Get-AuthenticodeSignature -FilePath $packagedExe).Status.ToString()
$commit = $null
try {
    $commit = (git -C $RepoRoot rev-parse HEAD).Trim()
} catch {
    $commit = $null
}

$buildInfo = [ordered]@{
    artifact = $artifactName
    version = $Version
    configuration = $Configuration
    packaged_at_utc = [DateTime]::UtcNow.ToString("o")
    commit = $commit
    source_binary = $BinaryPath
    signed = $signatureStatus -eq "Valid"
    signature_status = $signatureStatus
}

$buildInfo | ConvertTo-Json -Depth 4 | Set-Content -Path (Join-Path $stageDir "BUILD-INFO.json")

Compress-Archive -Path $stageDir -DestinationPath $zipPath -Force
$hash = (Get-FileHash -LiteralPath $zipPath -Algorithm SHA256).Hash.ToLowerInvariant()
$checksumLine = "$hash *$([System.IO.Path]::GetFileName($zipPath))"
Set-Content -Path $checksumPath -Value $checksumLine

Write-Host "Packaged $artifactName"
Write-Host "Zip: $zipPath"
Write-Host "Checksum: $checksumPath"

if ($env:GITHUB_OUTPUT) {
    "artifact_name=$artifactName" | Out-File -FilePath $env:GITHUB_OUTPUT -Encoding utf8 -Append
    "zip_path=$zipPath" | Out-File -FilePath $env:GITHUB_OUTPUT -Encoding utf8 -Append
    "checksum_path=$checksumPath" | Out-File -FilePath $env:GITHUB_OUTPUT -Encoding utf8 -Append
}
