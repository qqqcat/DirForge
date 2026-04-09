param(
    [Parameter(Mandatory = $true)]
    [string[]]$Files,
    [string]$TimestampUrl = "http://timestamp.digicert.com",
    [string]$CertificateBase64 = $env:WINDOWS_CODESIGN_CERT_BASE64,
    [string]$CertificatePassword = $env:WINDOWS_CODESIGN_PASSWORD
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Get-SignToolPath {
    $signtool = Get-Command signtool.exe -ErrorAction SilentlyContinue
    if ($signtool) {
        return $signtool.Source
    }

    $candidates = Get-ChildItem -Path "C:\Program Files (x86)\Windows Kits\10\bin\*\x64\signtool.exe" -ErrorAction SilentlyContinue |
        Sort-Object FullName -Descending

    if ($candidates) {
        return $candidates[0].FullName
    }

    return $null
}

$resolvedFiles = foreach ($file in $Files) {
    if (-not (Test-Path $file)) {
        throw "Cannot sign missing file: $file"
    }

    (Resolve-Path $file).Path
}

if (-not $CertificateBase64 -or -not $CertificatePassword) {
    Write-Host "Skipping code signing because certificate secrets are not configured."
    if ($env:GITHUB_OUTPUT) {
        "signed=false" | Out-File -FilePath $env:GITHUB_OUTPUT -Encoding utf8 -Append
    }
    return
}

$signtool = Get-SignToolPath
if (-not $signtool) {
    throw "Signing was requested, but signtool.exe was not found."
}

$pfxPath = Join-Path ([System.IO.Path]::GetTempPath()) ("dirotter-codesign-" + [guid]::NewGuid().ToString("N") + ".pfx")

try {
    [System.IO.File]::WriteAllBytes($pfxPath, [Convert]::FromBase64String($CertificateBase64))

    foreach ($file in $resolvedFiles) {
        & $signtool sign /fd SHA256 /tr $TimestampUrl /td SHA256 /f $pfxPath /p $CertificatePassword $file
        if ($LASTEXITCODE -ne 0) {
            throw "signtool failed for $file"
        }
        Write-Host "Signed $file"
    }

    if ($env:GITHUB_OUTPUT) {
        "signed=true" | Out-File -FilePath $env:GITHUB_OUTPUT -Encoding utf8 -Append
    }
} finally {
    if (Test-Path $pfxPath) {
        Remove-Item -LiteralPath $pfxPath -Force -ErrorAction SilentlyContinue
    }
}
