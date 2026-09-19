# 配对构建与回归基线（WP-00 / worldedit #1）

## 固定版本与工具链

`compatibility.json` 是编辑器的 worldline 兼容记录。CI 读取仓库名和完整 SHA，检出该版本后实际构建桌面及 WASM；不跟随 worldline 的 main。编辑器版本由当前 CI 检出的提交确定（PR 时为 GitHub 的合并测试提交），日志登记完整 SHA。

设计冻结基线：worldline `dcb6479873f86b08354ba92b098dc260aa976624`，worldedit `b0955adcccd740d9b3b46cee1d0f503ad25f5257`。WP-00 实施起点：worldline `5443226192b424edb01c6d5644694188c3a8ef4c`，worldedit `700597d3c383080dedd262ef389df4d6051db7e5`。两起点相对冻结基线的源码、测试、样例、Cargo.toml、Cargo.lock 与工具链均未改变，因此本次复跑的就是原回归套件；不声称在历史日期执行过测试。

两仓库各自的 `rust-toolchain.toml` 固定 Rust 1.98.0，包含 rustfmt/clippy。Cargo 1.98.0；本次主机为 Windows x86_64 MSVC，profile 为 dev/test。锁文件使用 SHA-256：

| 仓库 | Cargo.lock SHA-256 |
| --- | --- |
| worldline | `31e3bbba6686e4cd48945e46d418062718927ff8c6ed96d90a1a23d839540023` |
| worldedit | `67b14b4b3d16004d98d23cd68a12427d147cd376dfa598bbf4482c989bab06eb` |

锁文件随各仓库提交保留；所有测试、检查和构建使用 `--locked`。CI 的操作系统镜像仍为 windows-latest，本项保证源码/依赖配对可复测，不保证二进制逐字节相同。

## 在独立目录复现

在空目录运行以下 PowerShell 命令，将 `<worldedit-sha>` 替换为要复测的完整编辑器提交 SHA。不要在有未保存修改的作品或开发目录切换版本。

```powershell
git clone https://github.com/ikzerok/worldedit.git worldedit
git -C worldedit checkout --detach <worldedit-sha>
$pair = Get-Content worldedit/compatibility.json -Raw | ConvertFrom-Json
git clone "https://github.com/$($pair.worldline.repository).git" worldline
git -C worldline checkout --detach $pair.worldline.sha
Set-Location worldedit
rustup show active-toolchain
rustup target add wasm32-unknown-unknown
./scripts/check-pair.ps1
```

脚本校验 worldline HEAD，分别指定两个 Cargo.toml，执行格式、完整测试、严格 clippy、原生实际构建及编辑器 WASM 检查和实际构建，任一步失败立即失败。`target/paired-check/` 保存操作系统、CPU、内存、GPU 清单、两仓 SHA、工作区修改状态、锁文件摘要及逐项日志；有工作区修改时只能视为开发验证，正式配对证据必须来自干净检出。此脚本用于 Windows 自动检查，不启动 GUI 或浏览器，浏览器版本与 DPI 明确记为未测；它们须在后续实际 GUI/浏览器验收时另行记录。

CI 无论成功失败均上传 `paired-check-<编辑器SHA>` artifact，保留 90 天；到期前从 Actions 下载长期所需的证据。原始本机日志只留 target，不上传源码仓库。没有执行到的检查不能视作成功。

## 验收口径

2026-09-19 本地验证结果：worldline 156 项（含 1 项文档测试）、worldedit 6 项测试通过，失败 0；两仓格式与严格 clippy、两仓原生构建、编辑器 WASM clippy 与实际构建全部通过。原始日志保存在上述本地证据目录；尚未推送本次变更，因此此处不声称 GitHub Actions 已运行通过。

- **Q-001 / T-Q-001**：冻结上述提交，以及 worldline 基线提交内的 `examples/harbor-world` 与既有测试夹具。原有 `.wl`、状态、链接、存档相关回归套件全部通过，零新增失败；测试日志与环境信息归档。本项不替代 M1 的桌面 GUI 手势、浏览器交互、性能和新地图用例验收。
- **Q-007 / T-Q-007**：编辑器 CI 从兼容记录读取 worldline 的完整 SHA，校验实际 HEAD，分别执行两仓测试并真正构建编辑器原生/WASM 目标。缺失提交、版本不匹配或检查失败必须阻断；浮动 main 不能作兼容版本。

## 成对升级与回滚

本次是首次兼容配对登记：worldline 固定到 `5443226192b424edb01c6d5644694188c3a8ef4c`，worldedit 为包含此记录并通过 CI 的提交（完整 SHA 写入每次 artifact 的 environment.txt）。之前的 CI 没有固定配对记录，本次未执行版本升级或回滚演练。WP-00 交付升级/回滚验收口径；后续每次实际操作必须记录旧、新两组成对 SHA、工具链/锁变更、检查 artifact 链接和结果，不能只记录操作意图。

升级时先选择已经提交的 worldline SHA，再在 worldedit 修改兼容记录；如果 Rust 或依赖有变化，同时提交相应工具链/锁文件和验收文档。运行配对检查，记录新的编辑器提交及 worldline SHA，并保存 CI artifact 后才将该组合视为已验证。

回滚时在独立目录检出先前通过验收的编辑器提交，并按其兼容记录检出 worldline；若需在主分支回退，则一并回退编辑器适配代码、兼容记录、工具链与锁文件，重跑配对检查。只改 worldline ref 不构成已验证回滚。
