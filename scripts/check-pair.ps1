# 在同级的两个仓库执行检查；不切换或修改用户的工作分支。
$ErrorActionPreference = 'Stop'
$editorRoot = Split-Path $PSScriptRoot -Parent
$languageRoot = Join-Path (Split-Path $editorRoot -Parent) 'worldline'
$runId = [DateTime]::UtcNow.ToString('yyyyMMddTHHmmssfffffffZ')
$evidenceRoot = Join-Path $editorRoot "target/paired-check/$runId"
New-Item -ItemType Directory -Force -Path $evidenceRoot | Out-Null
$pair = Get-Content (Join-Path $editorRoot 'compatibility.json') -Raw | ConvertFrom-Json
$actual = git -C $languageRoot rev-parse HEAD
if ($LASTEXITCODE -ne 0 -or $actual -cne $pair.worldline.sha) {
    throw 'worldline HEAD 与 compatibility.json 不匹配，请先在独立目录检出固定版本'
}
function Invoke-Recorded {
    param([string]$Name, [string]$Program, [string[]]$Arguments)
    $logPath = Join-Path $evidenceRoot "$Name.log"
    "$Program $($Arguments -join ' ')" | Set-Content $logPath
    & $Program @Arguments 2>&1 | Tee-Object -FilePath $logPath -Append
    if ($LASTEXITCODE -ne 0) { throw "$Name 失败，退出码：$LASTEXITCODE" }
    'exit code: 0' | Add-Content $logPath
}
Push-Location $editorRoot
try {
    @(
        "UTC: $([DateTime]::UtcNow.ToString('o'))"
        "OS: $([System.Runtime.InteropServices.RuntimeInformation]::OSDescription)"
        "worldedit: $(git rev-parse HEAD)"
        "worldline: $actual"
        "worldedit working tree: $(git status --porcelain)"
        "worldline working tree: $(git -C $languageRoot status --porcelain)"
        "profile: dev/test (Cargo defaults)"
        "CPU: $((Get-CimInstance Win32_Processor).Name -join '; ')"
        "Memory bytes: $((Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory)"
        "GPU inventory: $((Get-CimInstance Win32_VideoController).Name -join '; ')"
        'Browser version / DPI: not measured; headless Cargo checks, no browser or GUI acceptance performed'
    ) | Set-Content (Join-Path $evidenceRoot 'environment.txt')
    Copy-Item (Join-Path $editorRoot 'compatibility.json') $evidenceRoot
    Get-FileHash (Join-Path $editorRoot 'Cargo.lock'), (Join-Path $languageRoot 'Cargo.lock') -Algorithm SHA256 |
        Select-Object Hash, Path | ConvertTo-Json | Set-Content (Join-Path $evidenceRoot 'locks.json')
    Invoke-Recorded 'rustc' 'rustc' @('-Vv')
    Invoke-Recorded 'cargo' 'cargo' @('-V')
    $languageManifest = Join-Path $languageRoot 'Cargo.toml'
    $editorManifest = Join-Path $editorRoot 'Cargo.toml'
    Invoke-Recorded 'worldline-fmt' 'cargo' @('fmt', '--manifest-path', $languageManifest, '--all', '--', '--check')
    Invoke-Recorded 'worldline-test' 'cargo' @('test', '--manifest-path', $languageManifest, '--workspace', '--locked')
    Invoke-Recorded 'worldline-clippy' 'cargo' @('clippy', '--manifest-path', $languageManifest, '--workspace', '--all-targets', '--locked', '--', '-D', 'warnings')
    Invoke-Recorded 'worldline-build' 'cargo' @('build', '--manifest-path', $languageManifest, '--workspace', '--locked')
    Invoke-Recorded 'worldedit-fmt' 'cargo' @('fmt', '--manifest-path', $editorManifest, '--all', '--', '--check')
    Invoke-Recorded 'worldedit-test' 'cargo' @('test', '--manifest-path', $editorManifest, '--locked')
    Invoke-Recorded 'worldedit-clippy' 'cargo' @('clippy', '--manifest-path', $editorManifest, '--all-targets', '--locked', '--', '-D', 'warnings')
    Invoke-Recorded 'worldedit-build' 'cargo' @('build', '--manifest-path', $editorManifest, '--locked')
    Invoke-Recorded 'worldedit-wasm-clippy' 'cargo' @('clippy', '--manifest-path', $editorManifest, '--target', 'wasm32-unknown-unknown', '--locked', '--', '-D', 'warnings')
    Invoke-Recorded 'worldedit-wasm-build' 'cargo' @('build', '--manifest-path', $editorManifest, '--target', 'wasm32-unknown-unknown', '--locked')
} finally {
    Pop-Location
}
