# 构建与发布

## 0.3.0 更新

完成 M2–M4 作者工作台：通用实体与独立关系、局部关联网络和共享布局、跨视图重构保护、16 类可选模板、长文保真与性能门、作者范围/源码集、展示预设、批注与修改提案，以及 65 项最终验收。地图/网络浏览继续保持零语义副作用，旧 1.9 工程不自动升级。

本版本固定配对 worldline 0.3.0；最终双仓 SHA、Windows/Web 烟测、性能数据和发布包校验值写入发行目录 `release-pair.json`。完整验收见 [WP-17 全量验收](wp17-acceptance.md) 与 [0.2→0.3 兼容/迁移说明](migration-0.2.md)。

## 0.2.0 更新

Wiki 已集成到编辑器：可按关键词、ID 和别名查找资料，创建或修改独立词条、释义与别名。正文、资料、源码预览和演练中的关键词自动链接；同名词条提供候选，显式链接保留指定目标。注释索引显示全部出现位置并可跳回源码，查阅演练词条不会推进回合。

本版本需要同级 worldline 0.2.0。使用方式见 [Wiki 词条](wiki.md)，已完成的自动检查和桌面操作见 [验证记录](verification.md)。

## 源码仓库

worldedit 和 worldline 分别上传各自目录，克隆到同级位置。各仓库保留 Cargo.toml、Cargo.lock、rust-toolchain.toml、LICENSE、README、AGENTS、源码、测试和持续集成。worldedit 另保留图标、开放字体与许可证、Web 入口、启动脚本和 .agent 创作技能。

target、dist、releases、日志和临时文件不是源码，不上传。父目录旧研究、规划、截图与图标生成过程不属于这两个仓库。不能删除 core 编译时嵌入的 worldline/examples/harbor-world，也不能删除字体许可证。

## 门禁

在 worldedit 目录执行 `./scripts/check-pair.ps1`，分别检查两仓 fmt、test、clippy 并实际构建原生及编辑器 WASM 目标。工具链由各自 rust-toolchain 固定。CI 从 `compatibility.json` 读取 worldline 仓库及完整 SHA；变更托管位置时同步更新兼容记录和 workflow 的仓库校验。版本升级、回滚及日志归档见[配对构建与回归基线](paired-ci.md)。

## Windows 和 Web 包

在已安装 Rust、WebAssembly 编译目标和 Trunk 的机器运行：

```powershell
./scripts/package.ps1
./scripts/package.ps1 -OutputDirectory D:/发行包 -SkipWeb
```

脚本使用 --release --locked，输出独立带时间的目录，包含 Windows ZIP、可选 Web ZIP、两个源码 ZIP 与 SHA256SUMS.txt。Windows 包包含 worldedit.exe、wl.exe、wl-agent.exe、图标、项目及字体许可证、创作技能、使用说明、语言规范与示例。Web 包为静态 HTTP 文件，附 Noto 字体许可证；不能双击 HTML 运行。

当前脚本适用于 Windows x64 主机，未构建 macOS/Linux 安装器，也不签名或上传。GitHub 上传和 Release 发布由用户决定；本地打包成功不表示线上已发布。两个源码 ZIP 解压到同一目录即可得到同级 worldline 与 worldedit 目录；source/ 下也保留这两个待上传目录。源码打包递归排除缓存、版本库、本机配置、凭据扩展名和日志，遇到符号链接或目录联接会停止。

解压后验证 wl check worldline/examples/harbor-world --json，再打开编辑器验证工作区。Web 包用静态 HTTP 服务器运行。源码发布前确认 GitHub 页面包含以点开头的 .agent 与 .github；不要只拖动文件管理器当前可见文件。

## 公开上传范围

上传 source/worldline 的内容到 worldline 仓库，上传 source/worldedit 的内容到 worldedit 仓库；不要把 source 或本地组合父目录作为第三层套入仓库。两者默认分支使用 main，但编辑器 CI 只检出 `compatibility.json` 指定的 worldline SHA；先确保该提交在指定远端可获取，再上传 worldedit。保留隐藏的 .github、.agent、.gitignore、Cargo.lock 和兼容记录。父目录原有 .git 历史不在源码包中。

Windows/Web ZIP 与 SHA256SUMS.txt 作为 GitHub Release 附件；不提交到源码树。是否完成线上发布以 GitHub Release 页面及附件为准。

## WP-17 配对验收（2026-09-26）

M0–M4 的最终需求状态见 [WP-17 全量验收](wp17-acceptance.md)，升级/回退边界见 [0.2 格式升级说明](migration-0.2.md)。最终 core 文档基线为 `fe88d7f1447995e49bb4b54a20a53be0ad0b2db6`，编辑器 `compatibility.json` 固定该 SHA；最终 editor SHA 在源码提交后写入发行目录 `release-pair.json` 与配对 tag，不使用自引用占位符。

本轮完整 paired-check 的两仓 fmt/test/clippy、原生构建、WASM clippy/构建均为 exit 0；worldline 325 项、worldedit 103 项测试通过。D5 release 测量为网络帧 P95 11.073ms、暖态资料切换 P95 4.128ms。Windows release 实际创建窗口；Edge 153 headless 实际请求 Web 包 index/JS/WASM/icon 全部 HTTP 200。

发行包继续由 `scripts/package.ps1` 生成 Windows、Web 和双仓源码 ZIP 及 SHA256SUMS；最终上传前以配对 tag、`release-pair.json` 和 GitHub CI 结果共同核对两仓完整 SHA。
