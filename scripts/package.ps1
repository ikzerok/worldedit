param([string]$OutputDirectory, [switch]$SkipWeb)
$ErrorActionPreference = 'Stop'
$env:NO_COLOR = 'true'
$editorRoot = Split-Path -Parent $PSScriptRoot
$combinedRoot = Split-Path -Parent $editorRoot
$languageRoot = Join-Path $combinedRoot 'worldline'
if (-not $OutputDirectory) { $OutputDirectory = Join-Path $combinedRoot 'releases' }
$OutputDirectory = [IO.Path]::GetFullPath($OutputDirectory)
New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
$buildRoot = Join-Path $OutputDirectory ('worldedit-' + (Get-Date -Format 'yyyyMMdd-HHmmss'))
New-Item -ItemType Directory -Path $buildRoot | Out-Null
$env:CARGO_TARGET_DIR = Join-Path $combinedRoot 'target'

function Run-Cargo([string[]]$Arguments) {
    & cargo @Arguments
    if ($LASTEXITCODE -ne 0) { throw "Cargo 失败：$Arguments" }
}

function Run-PackageHelper([string[]]$Arguments) {
    & python "$PSScriptRoot/release-package.py" @Arguments
    if ($LASTEXITCODE -ne 0) { throw "发行文件筛选/审计失败：$Arguments" }
}

Run-PackageHelper @('check-source', $languageRoot, $editorRoot)
Run-Cargo @('build', '--manifest-path', "$languageRoot/Cargo.toml", '--workspace', '--release', '--locked')
Run-Cargo @('build', '--manifest-path', "$editorRoot/Cargo.toml", '--release', '--locked')
$desktopRoot = Join-Path $buildRoot 'windows'
New-Item -ItemType Directory -Path $desktopRoot | Out-Null
foreach ($binary in @('worldedit.exe', 'wl.exe', 'wl-agent.exe')) {
    Copy-Item -LiteralPath (Join-Path $env:CARGO_TARGET_DIR "release/$binary") -Destination $desktopRoot
}
foreach ($directory in @('.agent', 'docs')) {
    Run-PackageHelper @('copy', '--source', (Join-Path $editorRoot $directory),
        '--destination', (Join-Path $desktopRoot $directory), '--prefix', "windows/$directory")
}
Copy-Item -LiteralPath "$editorRoot/README.md", "$editorRoot/LICENSE", "$editorRoot/assets/worldedit.ico" -Destination $desktopRoot
Copy-Item -LiteralPath "$editorRoot/assets/fonts/OFL.txt" -Destination (Join-Path $desktopRoot 'FONT-LICENSE.txt')
Copy-Item -LiteralPath "$editorRoot/assets/licenses/resvg-MIT.txt" -Destination (Join-Path $desktopRoot 'RESVG-LICENSE.txt')
Copy-Item -LiteralPath "$editorRoot/assets/licenses/self-cell-APACHE.txt" -Destination (Join-Path $desktopRoot 'SELF-CELL-LICENSE.txt')
$languageDocs = Join-Path $desktopRoot 'worldline'
New-Item -ItemType Directory -Path $languageDocs | Out-Null
foreach ($directory in @('spec', 'docs')) {
    Run-PackageHelper @('copy', '--source', (Join-Path $languageRoot $directory),
        '--destination', (Join-Path $languageDocs $directory), '--prefix', "windows/worldline/$directory")
}
Copy-Item -LiteralPath "$languageRoot/README.md", "$languageRoot/LICENSE" -Destination $languageDocs
Compress-Archive -LiteralPath $desktopRoot -DestinationPath "$buildRoot/worldedit-windows-x64.zip"

if (-not $SkipWeb) {
    $webRoot = Join-Path $buildRoot 'web'
    Push-Location $editorRoot
    try {
        & trunk build --release --locked --dist $webRoot
        if ($LASTEXITCODE -ne 0) { throw 'Web 构建失败' }
    } finally { Pop-Location }
    Copy-Item -LiteralPath "$editorRoot/assets/fonts/OFL.txt", "$editorRoot/LICENSE" -Destination $webRoot
    Copy-Item -LiteralPath "$editorRoot/assets/licenses/resvg-MIT.txt" -Destination (Join-Path $webRoot 'RESVG-LICENSE.txt')
    Copy-Item -LiteralPath "$editorRoot/assets/licenses/self-cell-APACHE.txt" -Destination (Join-Path $webRoot 'SELF-CELL-LICENSE.txt')
    Compress-Archive -LiteralPath $webRoot -DestinationPath "$buildRoot/worldedit-web.zip"
}

# Git 检出使用原始提交归档；无 .git 的已导出源码使用相同样例策略精确筛选。
foreach ($repository in @($languageRoot, $editorRoot)) {
    $name = Split-Path -Leaf $repository
    Run-PackageHelper @('source', '--source', $repository,
        '--destination', "$buildRoot/$name-source.zip", '--name', $name)
}
$archives = @(Get-ChildItem -LiteralPath $buildRoot -Filter '*.zip' | ForEach-Object { $_.FullName })
Run-PackageHelper (@('audit') + $archives)
Get-ChildItem -LiteralPath $buildRoot -Filter '*.zip' | Get-FileHash -Algorithm SHA256 |
    ForEach-Object { "$($_.Hash.ToLowerInvariant())  $([IO.Path]::GetFileName($_.Path))" } |
    Set-Content -LiteralPath "$buildRoot/SHA256SUMS.txt" -Encoding utf8
Write-Host "发布包：$buildRoot"
