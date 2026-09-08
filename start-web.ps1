param([int]$Port = 8787, [switch]$NoOpen)
$ErrorActionPreference = 'Stop'
$env:NO_COLOR = 'true'
Set-Location -LiteralPath $PSScriptRoot
$localUrl = "http://127.0.0.1:$Port/"
try {
    $existingPage = Invoke-WebRequest -Uri $localUrl -UseBasicParsing -TimeoutSec 2
} catch {
    $existingPage = $null
}
if ($existingPage -and $existingPage.Content.Contains('id="worldedit-canvas"')) {
    Write-Host "worldedit 已在运行：$localUrl"
    if (-not $NoOpen) { Start-Process $localUrl }
    exit 0
}
foreach ($tool in @('cargo', 'rustup')) {
    if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) { throw '请先安装 Rust：https://rustup.rs/' }
}
if (-not ((& rustup target list --installed) -contains 'wasm32-unknown-unknown')) {
    & rustup target add wasm32-unknown-unknown
    if ($LASTEXITCODE -ne 0) { throw 'Web 编译目标安装失败' }
}
if (-not (Get-Command trunk -ErrorAction SilentlyContinue)) {
    & cargo install trunk --version 0.21.14 --locked
    if ($LASTEXITCODE -ne 0) { throw 'Web 构建工具安装失败' }
}
Set-Location -LiteralPath $PSScriptRoot
Write-Host "worldedit 浏览器版：http://127.0.0.1:$Port（首次编译需要几分钟，关闭本窗口可停止服务）"
$webArguments = @('serve', '--release', '--locked', '--port', "$Port", '--skip-version-check', '--log', 'info', '--disable-address-lookup')
if (-not $NoOpen) { $webArguments += '--open' }
& trunk @webArguments
if ($LASTEXITCODE -ne 0) { throw 'Web 服务启动失败，请查看上方信息' }
