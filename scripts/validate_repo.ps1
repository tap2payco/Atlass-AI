param (
    [switch]$Verbose
)

$ErrorActionPreference = "Stop"
$failures = 0

function Assert-Exists {
    param([string]$Path, [string]$Type)
    if (-not (Test-Path $Path)) {
        Write-Host "[FAIL] Missing $Type : $Path" -ForegroundColor Red
        $script:failures++
    } else {
        if ($Verbose) { Write-Host "[OK] Found $Path" -ForegroundColor Green }
    }
}

Write-Host "--- Atlas Repository Validation ---"

# Layer 1A
$requiredFiles = @(
    "README.md", "LICENSE", ".gitignore", ".gitattributes", 
    ".editorconfig", ".pre-commit-config.yaml", "CODE_OF_CONDUCT.md", 
    "CONTRIBUTING.md", "SECURITY.md", "CHANGELOG.md", "VERSION",
    "IMPLEMENTATION_GUIDE.md", "ENGINEERING_BOARD.md",
    "atlas-metadata.yaml", "repository.manifest.yaml"
)

foreach ($file in $requiredFiles) {
    Assert-Exists -Path $file -Type "File"
}

# Layer 1B
$requiredWorkspaces = @(
    "Cargo.toml", "pyproject.toml", "package.json", "rust-toolchain.toml"
)

foreach ($ws in $requiredWorkspaces) {
    Assert-Exists -Path $ws -Type "Workspace File"
}

$requiredDirs = @("configs", "scripts", "docs", "apps", "packages", "schemas", "tests")
foreach ($dir in $requiredDirs) {
    Assert-Exists -Path $dir -Type "Directory"
}

if ($failures -gt 0) {
    Write-Host "`nValidation Failed: $failures issues found." -ForegroundColor Red
    exit 1
} else {
    Write-Host "`nValidation Passed! Repository structure matches baseline." -ForegroundColor Green
    exit 0
}
