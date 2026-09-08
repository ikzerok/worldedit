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

function Copy-PublicSource([string]$Source, [string]$Destination) {
    New-Item -ItemType Directory -Path $Destination -Force | Out-Null
    Get-ChildItem -LiteralPath $Source -Force | Where-Object {
        $_.Name -notin @('target', 'dist', 'releases', '.git', '.idea', '.vscode', '.mimosa', '.zcode', 'node_modules', '__pycache__', '.DS_Store', 'Thumbs.db') -and
        $_.Name -notlike '.env*' -and $_.Name -notlike '*.save.json' -and
        $_.Extension -notin @('.log', '.tmp', '.bak', '.swp', '.pyc', '.pem', '.key', '.pfx')
    } | ForEach-Object {
        if ($_.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw "源码目录不能包含链接：$($_.FullName)" }
        $destinationPath = Join-Path $Destination $_.Name
        if ($_.PSIsContainer) { Copy-PublicSource $_.FullName $destinationPath }
        else { Copy-Item -LiteralPath $_.FullName -Destination $destinationPath }
    }
}

Run-Cargo @('build', '--manifest-path', "$languageRoot/Cargo.toml", '--workspace', '--release', '--locked')
Run-Cargo @('build', '--manifest-path', "$editorRoot/Cargo.toml", '--release', '--locked')
$desktopRoot = Join-Path $buildRoot 'windows'
New-Item -ItemType Directory -Path $desktopRoot | Out-Null
foreach ($binary in @('worldedit.exe', 'wl.exe', 'wl-agent.exe')) {
    Copy-Item -LiteralPath (Join-Path $env:CARGO_TARGET_DIR "release/$binary") -Destination $desktopRoot
}
foreach ($directory in @('.agent', 'docs')) {
    Copy-Item -LiteralPath (Join-Path $editorRoot $directory) -Destination $desktopRoot -Recurse
}
Copy-Item -LiteralPath "$editorRoot/README.md", "$editorRoot/LICENSE", "$editorRoot/assets/worldedit.ico" -Destination $desktopRoot
Copy-Item -LiteralPath "$editorRoot/assets/fonts/OFL.txt" -Destination (Join-Path $desktopRoot 'FONT-LICENSE.txt')
$languageDocs = Join-Path $desktopRoot 'worldline'
New-Item -ItemType Directory -Path $languageDocs | Out-Null
foreach ($directory in @('spec', 'docs', 'examples')) {
    Copy-Item -LiteralPath (Join-Path $languageRoot $directory) -Destination $languageDocs -Recurse
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
    Compress-Archive -LiteralPath $webRoot -DestinationPath "$buildRoot/worldedit-web.zip"
}

# 源码包递归排除缓存与本机配置；解压后的根目录名可直接作为同级路径依赖。
foreach ($repository in @($languageRoot, $editorRoot)) {
    $name = Split-Path -Leaf $repository
    $sourceRoot = Join-Path $buildRoot "source/$name"
    Copy-PublicSource $repository $sourceRoot
    Compress-Archive -LiteralPath $sourceRoot -DestinationPath "$buildRoot/$name-source.zip"
}
Get-ChildItem -LiteralPath $buildRoot -Filter '*.zip' | Get-FileHash -Algorithm SHA256 |
    ForEach-Object { "$($_.Hash.ToLowerInvariant())  $([IO.Path]::GetFileName($_.Path))" } |
    Set-Content -LiteralPath "$buildRoot/SHA256SUMS.txt" -Encoding utf8
Write-Host "发布包：$buildRoot"
