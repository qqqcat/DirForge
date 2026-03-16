param(
    [string]$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$errors = [System.Collections.Generic.List[string]]::new()
$checks = [System.Collections.Generic.List[string]]::new()

function Add-Error {
    param([string]$Message)
    $script:errors.Add($Message)
}

function Add-Check {
    param([string]$Message)
    $script:checks.Add($Message)
}

function Test-RequiredPath {
    param([string]$RelativePath)

    $fullPath = Join-Path $RepoRoot $RelativePath
    if (-not (Test-Path $fullPath)) {
        Add-Error "Missing required path: $RelativePath"
        return
    }

    Add-Check "Found $RelativePath"
}

function Test-ContentContains {
    param(
        [string]$RelativePath,
        [string[]]$Needles
    )

    $fullPath = Join-Path $RepoRoot $RelativePath
    if (-not (Test-Path $fullPath)) {
        return
    }

    $content = Get-Content -Raw -Path $fullPath
    foreach ($needle in $Needles) {
        if ($content -notmatch [regex]::Escape($needle)) {
            Add-Error "$RelativePath is missing expected content: $needle"
        }
    }

    Add-Check "Validated content markers in $RelativePath"
}

$requiredPaths = @(
    ".github/copilot-agent.md",
    ".github/devin-loop.md",
    ".github/project-context.md",
    ".github/task-queue.md",
    ".github/checklists/definition-of-done.md",
    ".github/checklists/recovery.md",
    ".github/instructions/autonomous.md",
    ".github/instructions/execution.md",
    ".github/instructions/planning.md",
    ".github/instructions/quality-gate.md",
    ".github/instructions/resume.md",
    ".github/memory/bugs.md",
    ".github/memory/decisions.md",
    ".github/memory/goals.md",
    ".github/memory/lessons.md",
    ".github/memory/progress.md",
    ".github/workflows/template-validation.yml",
    ".vscode/settings.json",
    "README.md",
    "docs/autonomous-workflow.md",
    "docs/data-contracts.md",
    "docs/engineering-requirements.md",
    "docs/quickstart.md",
    "findings.md",
    "progress.md",
    "task_plan.md",
    "scripts/validate-template.ps1"
)

foreach ($path in $requiredPaths) {
    Test-RequiredPath -RelativePath $path
}

$taskQueuePath = Join-Path $RepoRoot ".github/task-queue.md"
if (Test-Path $taskQueuePath) {
    $taskQueue = Get-Content -Raw -Path $taskQueuePath
    $taskPattern = '(?m)^- \[(todo|in_progress|blocked|done)\]\[(P[0-3])\] .+$'
    $taskMatches = [regex]::Matches($taskQueue, $taskPattern)
    if ($taskMatches.Count -eq 0) {
        Add-Error ".github/task-queue.md does not contain any valid queue items"
    } else {
        Add-Check "Parsed $($taskMatches.Count) queue items"
    }

    $inProgressCount = ([regex]::Matches($taskQueue, '(?m)^- \[in_progress\]\[P[0-3]\] .+$')).Count
    if ($inProgressCount -gt 1) {
        Add-Error ".github/task-queue.md has more than one in_progress item"
    } else {
        Add-Check ".github/task-queue.md in_progress count is valid"
    }

    foreach ($section in @("## Active Goal", "## Queue", "## Blockers")) {
        if ($taskQueue -notmatch [regex]::Escape($section)) {
            Add-Error ".github/task-queue.md is missing section: $section"
        }
    }
}

Test-ContentContains -RelativePath "README.md" -Needles @(
    ".github/task-queue.md",
    ".github/memory/progress.md",
    ".github/instructions/quality-gate.md",
    "docs/quickstart.md",
    "scripts/validate-template.ps1"
)

Test-ContentContains -RelativePath "docs/autonomous-workflow.md" -Needles @(
    ".github/task-queue.md",
    ".github/devin-loop.md",
    ".github/instructions/quality-gate.md",
    ".github/memory/",
    "scripts/validate-template.ps1"
)

Test-ContentContains -RelativePath ".github/memory/decisions.md" -Needles @(
    "## Template"
)

Test-ContentContains -RelativePath ".github/memory/progress.md" -Needles @(
    "## Current Sprint",
    "## Completed",
    "## Pending"
)

if ($errors.Count -gt 0) {
    Write-Host "Template validation failed:" -ForegroundColor Red
    foreach ($error in $errors) {
        Write-Host " - $error" -ForegroundColor Red
    }

    exit 1
}

Write-Host "Template validation passed." -ForegroundColor Green
foreach ($check in $checks) {
    Write-Host " - $check"
}
