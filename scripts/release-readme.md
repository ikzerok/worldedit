# 正式发行工作流

## 操作边界

此流程合并到 main 后，由维护者在**当前已验收且 CI 成功的 main 完整 SHA**创建普通分支 `release/vX.Y.Z` 触发 GitHub `create` 事件。它不是 tag，也不使用 push、PR 或 dispatch 触发。版本必须与 editor 的 Cargo.toml 完全一致；core 保持真实版本，不为配对强制升级。

产品版本按 `0.x.y` 递增；当前发行版本必须读取已验收的 Cargo.toml，不使用历史建议值。先合并产品修复、版本与本流程，等待两仓最终 main 的 `ci.yml` push CI 成功并关闭已验收工单，再创建发行分支。普通分支的 create 事件会跳过全部 job。

流程仅使用官方 GitHub Actions、GitHub CLI 与临时 GITHUB_TOKEN。全局/build 为 contents:read；gate 增加 actions:read/issues:read；只有最终 publish job 有 contents:write，另保留门禁必需的只读权限。所有 checkout 禁止持久化凭据。不新增 PAT、OAuth、密钥、环境 secret、id-token、packages 或 actions 写权限。

## 门禁与产物

- 验证事件仓库、branch 类型、严格 `release/vX.Y.Z` 格式与事件 SHA
- editor 的事件 SHA、发行分支 SHA、实时 main SHA 必须一致；兼容记录的 worldline SHA 必须等于其 main
- 分页读取两仓 open issues，剔除 PR 后必须为零
- 两仓 ci.yml 分页读取全部匹配运行；最新精确 SHA/main/push 运行必须 completed/success，不能用旧成功掩盖新失败。分页错误、重复或数量不完整均停止
- 已有同名 tag 或公开 Release 在只读 gate 提前停止；publish 的写 token 在创建 tag 前分页列出所有 Release，确认没有同名 draft/公开版本。404 之外的 API 错误不能当作不存在
- Windows runner 安装仓库固定 Rust 工具链与固定 Trunk 0.21.14，执行原有 package.ps1
- 从两仓指定 commit 的 git archive 快照构建，源码 ZIP 保留隐藏文件，只含已提交公开文件，并逐字节保留原始 Git 归档。提交中的 export-ignore 排除 worldline 根 examples/、spec/examples/ 和独立 eds10-visual-sample.html；不重写源码 ZIP 或打包本地目录。源快照提取前及所有最终 ZIP 使用同一安全检查：禁止分发这些样例路径；docs/qa 只接受 UTF-8 文字/结构化证据，拒绝媒体、改后缀二进制与常见编码媒体；拒绝 .tooling/.aws/.ssh/.env/凭据/密钥路径、链接及非规范路径。保留公开应用图标、spec/templates.catalog.json 产品字段模板、必要回归夹具及 core/examples/relations_profile.rs 开发性能工具，不能按任意 examples/fixtures 名称删除可构建、可测试源码。不使用编码容器搬运私有截图
- 附件固定为 Windows ZIP、Web ZIP、双仓源码 ZIP、release-pair.json、SHA256SUMS.txt；校验文件覆盖其余五个附件
- 从最终 Windows ZIP 重新提取 wl.exe，在发行目录之外的临时目录生成仅含 `event start` 与缩进 `-> END` 的最小 smoke.wl，运行 `wl.exe check <temporary>/smoke.wl --json`。要求 exit 0、ok:true、read_only:false、零诊断、一个事件及无 stderr，结果写入 release-pair.json 的 build.cli_smoke；临时输入和解压目录随后删除，不进入附件。另检查 Windows 可执行文件以及 Web index/JS/WASM 产物。此步骤不声称执行 Windows GUI 或完整 Web 交互验收
- publish 校验全部附件、配对 SHA/版本/根目录和校验清单，重查全部远端门禁，然后创建精确 SHA 的 tag 及 draft Release
- 上传后重新下载六个资产，逐字节比对、再次核对 manifest 与 ZIP；公开前重查全部门禁、tag、draft 唯一 ID 与资产身份，再公开并回读确认。资产上传下载始终由官方 gh 对固定仓库执行，不直接请求 API 返回的 URL

ZIP 中的 commit comment 只是来源标记，不是签名或独立真实性证明；来源约束依靠只读 build 检出精确 SHA、git archive、同一次 workflow 的 artifact 传递及最终回读校验。

本地 package.ps1 需要 Python 3.11+（CI 使用 3.12），复用 release-package.py 的筛选与审计。Git 检出在构建前及源码归档前检查已暂存、未暂存及未忽略的未跟踪更改，有更改即停止，避免工作树二进制与旧 HEAD 源码不匹配。从已导出的源码构建时没有 .git，源码辅助 ZIP 按相同策略复制，保留 worldline/worldedit 同级根目录；正式 build 最终使用的双仓源码 ZIP 仍是起初固定提交的原始 git archive。复制遇到符号链接、Windows junction 或任何 reparse point 都停止，不沿其读取工作区外内容。

源码归档排除样例后仍须作为验收输入：解压到同一父目录，再执行原有配对构建和测试。样例排除或 ZIP 审计通过不等于已通过编译、回归或原生交互验收。

GitHub 没有将跨仓 main/issue/CI 检查与 Release PATCH 合并为原子事务的接口。此实现于公开前立即重查，明显减少竞态；不能保证重查和 PATCH 之间的极短窗口无人修改仓库。发行期间应停止合并与改动发行分支。并发发行在本工作流内串行，不能拦截人工改动。

## 失败处理

任何门禁/构建失败都不发布。发布中途失败可能留下新 tag 与 draft；不自动删除、不覆盖、不重新使用，也不把失败状态伪装为成功。维护者应检查 Actions 日志、tag SHA、draft 附件与 SHA256SUMS，决定人工恢复或使用新版本；直接 rerun 只会因目标存在而安全失败。

只能对 worldedit 创建发行；GITHUB_TOKEN 不获得跨仓写权限，不创建 worldline tag。配对记录用不可变 worldline 完整 SHA 及其真实 Cargo 版本。

## 本地离线验证

```sh
python -X utf8 -X warn_default_encoding -W error scripts/release-test.py -v
python -X utf8 -X warn_default_encoding -W error -m unittest discover -s scripts -p 'test_check_pair.py' -v
python -m py_compile scripts/release.py scripts/release-build.py scripts/release-package.py scripts/release-test.py
```

PR/main CI 已设置 Python 3.12，独立运行 pair/release 两组离线测试；每组均立即传播非零退出码。

离线测试使用模拟 API 和临时文件，不联网、不访问凭据、不创建 tag/release。通过不代表 Windows runner 已完成实际构建，更不代表已经线上发行。

官方参考：[create 事件](https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows#create)、[Git refs API](https://docs.github.com/en/rest/git/refs#create-a-reference)、[Releases API](https://docs.github.com/en/rest/releases/releases#create-a-release)。
