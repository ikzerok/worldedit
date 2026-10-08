# 构建与发布

## 发行说明链接（自 0.31.0）

正式 Release 的正文直接采用当前版本 `docs/releases/v版本.md`。其中的产品用法和规范链接必须使用完整 HTTPS 地址；GitHub Release 页面不会把仓库相对路径解释成对应文档。离线发行回归会拒绝当前版本说明中的相对 Markdown 链接。公开前还须实际核对目标提交的文档地址可访问；离线格式检查不能替代联网验证，也不修改历史 Release 或 tag。

## 长期无样例分发（自 0.29.0）

Windows、Web 和双仓源码发行 ZIP 不包含 worldline 根 `examples/`、`spec/examples/` 或独立 `eds10-visual-sample.html`。历史演示仍保留在开发仓库；正式源码归档通过对应提交的 `.gitattributes export-ignore` 排除它们，不重写 `git archive` 的字节。产品字段模板已迁至 `worldline/spec/templates.catalog.json`，必要回归夹具、开发性能工具 `core/examples/relations_profile.rs`、应用资产与许可证继续保留。

从源码 ZIP 重建和运行测试仍是配对验收要求；仅通过文件排除审计不能宣称已完成构建或运行验收。将两个 ZIP 解压到同一父目录后，在 worldline 运行 `cargo test --workspace --locked`，在 worldedit 运行 `cargo test --locked` 和 `cargo test --locked --features eds11_prototype`；两边均可用 `cargo build --release --locked` 重建。源码 ZIP 不包含 Git 元数据，`check-pair.ps1` 的精确 HEAD 检查用于完整 Git 检出，归档的提交身份由 ZIP commit comment 和发行配对清单核对。新建世界为空白工程，不预置演示作品。此节说明分发策略，不代表当前候选已构建或已经发布。

## 0.14.0 更新

试玩/重放显式稿件范围、语义主题色、真实条件证据来源导航和既有语言能力启用作为同一版本交付。启用能力采用 core 全稿预览与精确计划校验，默认语言仍1.9，最高1.13；不开新DSL、不自动迁移或保存。作者说明见[0.14使用与兼容边界](explicit-authoring-0.14.md)。

## 0.10.0 更新

完整交付机器Schema支持集合一致性、顶层临时层无损Esc、目录/ZIP导出范围决定点，以及显式DSL1.13的跨period偏序与静态character引用。默认语言与旧作品不自动迁移，原直接时段rank含义不变，静态改动不扩大运行或读者发布范围。作者用法及边界见[0.10说明](static-authoring-0.10.md)。


## 0.7.0 更新

资料查询增加名称/对象类型单字段升降序与恢复默认顺序。宽窗使用名称、对象类型、来源的可扫描表头；窄窗保留明确排序菜单，命中原因按对象展开，导航继续绑定完整 TargetRef。排序改变从第一页重新查询，不能把当前页重排冒充全局排序。

名称按 ASCII 不区分大小写的 Unicode 顺序，非拼音或数字自然排序；缺值始终置后，降序只反转主键。显式排序查询使用 v2，默认查询保持 v1，page/cursor 版本不变。共享文档级能力保护旧版，恢复默认并保存只清除本功能字段和声明。浏览、分页、菜单关闭与取消不写作品；本版不含分组或任意属性排序。

### 条件求值证据

复合选择条件提供作者侧真实求值证据，保留括号与运算层级，分别显示已满足、未满足、
求值错误和因前序错误未求值的项。证据记录有界，超限明确省略，不改变实际执行。
当前语言的急切 and/or、随机数消费、稳定选择身份及 trace 结构保持不变。

试玩错误后冻结该次尝试，作者可重新开始；切换选择组或重启时清除旧证据，
修改源码但未重启时明确标示旧运行版本。查看证据不写入作者作品、运行存档或读者发布。
RPC `session.explain_choices` 默认维持原字段，显式 `include_evidence:true` 才返回实际证据；
使用严格封闭 schema 的消费者应在启用前适配可选字段。版本绑定的旧 runtime 轨迹
仍遵守原有兼容性检查，本版不承诺跨 runtime 版本重放。

本轮发行准备改用固定版本的官方 Trunk 预编译文件：安装前核对 ZIP 与 EXE 的固定 SHA256、
大小和条目，运行精确版本检查后再写入 PATH，后续步骤再次核对路径与版本。下载或验证失败
即停止，不回退为未验证安装。Windows CI 与正式发行共用同一入口；不新增权限或凭据。

## 0.3.0 更新

完成 M2–M4 作者工作台：通用实体与独立关系、局部关联网络和共享布局、跨视图重构保护、16 类可选模板、长文保真与性能门、作者范围/源码集、展示预设、批注与修改提案，以及 65 项最终验收。地图/网络浏览继续保持零语义副作用，旧 1.9 工程不自动升级。

本版本固定配对 worldline 0.3.0；最终双仓 SHA、Windows/Web 烟测、性能数据和发布包校验值写入发行目录 `release-pair.json`。完整验收见 [WP-17 全量验收](wp17-acceptance.md) 与 [0.2→0.3 兼容/迁移说明](migration-0.2.md)。

## 0.2.0 更新

Wiki 已集成到编辑器：可按关键词、ID 和别名查找资料，创建或修改独立词条、释义与别名。正文、资料、源码预览和演练中的关键词自动链接；同名词条提供候选，显式链接保留指定目标。注释索引显示全部出现位置并可跳回源码，查阅演练词条不会推进回合。

本版本需要同级 worldline 0.2.0。使用方式见 [Wiki 词条](wiki.md)，已完成的自动检查和桌面操作见 [验证记录](verification.md)。

## 源码仓库

worldedit 和 worldline 分别上传各自目录，克隆到同级位置。各仓库保留 Cargo.toml、Cargo.lock、rust-toolchain.toml、LICENSE、README、AGENTS、源码、测试和持续集成。worldedit 另保留图标、开放字体与许可证、Web 入口、启动脚本和 .agent 创作技能。

target、dist、releases、日志和临时文件不是源码，不上传。父目录旧研究、规划、截图与图标生成过程不属于这两个仓库。源码分发保留 core 编译所需字段模板、必要测试输入及字体许可证；不把用于开发的性能工具或回归夹具误当作演示工程删除。

## 门禁

在 worldedit 目录执行 `./scripts/check-pair.ps1`，分别检查两仓 fmt、test、clippy 并实际构建原生及编辑器 WASM 目标。工具链由各自 rust-toolchain 固定。CI 从 `compatibility.json` 读取 worldline 仓库及完整 SHA；变更托管位置时同步更新兼容记录和 workflow 的仓库校验。版本升级、回滚及日志归档见[配对构建与回归基线](paired-ci.md)。

## Windows 和 Web 包

在已安装 Rust、WebAssembly 编译目标、Trunk 和 Python 3.11+（CI 为 3.12）的 Windows x64 机器运行：

```powershell
./scripts/package.ps1
./scripts/package.ps1 -OutputDirectory D:/发行包 -SkipWeb
```

脚本使用 --release --locked，输出独立带时间的目录，包含 Windows ZIP、可选 Web ZIP、两个源码 ZIP 与 SHA256SUMS.txt。Windows 包包含 worldedit.exe、wl.exe、wl-agent.exe、图标、项目及字体许可证、运行依赖许可、创作技能、使用说明和语言规范；不复制演示工程或规范样例。Web 包为静态 HTTP 文件，附字体与运行依赖许可；不能双击 HTML 运行。

当前脚本适用于 Windows x64 主机，未构建 macOS/Linux 安装器，也不签名或上传。GitHub 上传和 Release 发布由用户决定；本地打包成功不表示线上已发布。两个源码 ZIP 解压到同一目录即可得到同级 worldline 与 worldedit 目录；脚本不另外保留 source/ 工作目录。Git 检出在构建前与归档前要求没有已暂存、未暂存或未忽略的未跟踪更改，源码使用原始 HEAD 归档，避免二进制与源码版本错配。无 .git 的已导出源码采用同一精确样例筛选策略，并排除缓存、本机配置、凭据扩展名和日志。复制拒绝符号链接、Windows junction 和所有 reparse point，不沿其读取目录外内容；所有 ZIP 在计算校验值前再次审计。

正式 release-build.py 从最终 Windows ZIP 解出 wl.exe，在发行目录外生成只含 `event start` 和缩进 `-> END` 的临时 smoke.wl，运行 `wl.exe check <temporary>/smoke.wl --json`，核对成功状态、零诊断和一个事件，再删除临时输入与解压目录。该烟测的实际结果写入 release-pair.json，不再依赖随包演示工程。原生编辑器工作区交互须另行验证。Web 包用静态 HTTP 服务器运行。源码发布前确认 GitHub 页面包含以点开头的 .agent 与 .github；不要只拖动文件管理器当前可见文件。

## 公开上传范围

公开仓库维护各自的 worldline、worldedit 源码及历史，正式源码附件则是相应提交排除样例后的归档。不要用分发 ZIP 回写并删减开发仓库历史，也不要把本地组合父目录作为第三层套入仓库。两者默认分支使用 main，但编辑器 CI 只检出 `compatibility.json` 指定的 worldline SHA；先确保该提交在指定远端可获取，再上传 worldedit。保留隐藏的 .github、.agent、.gitignore、.gitattributes、Cargo.lock 和兼容记录。版本库元数据不在源码包中。

Windows/Web ZIP 与 SHA256SUMS.txt 作为 GitHub Release 附件；不提交到源码树。是否完成线上发布以 GitHub Release 页面及附件为准。

## WP-17 配对验收（2026-09-26）

以下为当时版本的历史记录，不是 0.29 候选的构建、测试或交互验收结果。

M0–M4 的最终需求状态见 [WP-17 全量验收](wp17-acceptance.md)，升级/回退边界见 [0.2 格式升级说明](migration-0.2.md)。最终 core 文档基线为 `fe88d7f1447995e49bb4b54a20a53be0ad0b2db6`，编辑器 `compatibility.json` 固定该 SHA；最终 editor SHA 在源码提交后写入发行目录 `release-pair.json` 与配对 tag，不使用自引用占位符。

本轮完整 paired-check 的两仓 fmt/test/clippy、原生构建、WASM clippy/构建均为 exit 0；worldline 325 项、worldedit 103 项测试通过。D5 release 测量为网络帧 P95 11.073ms、暖态资料切换 P95 4.128ms。Windows release 实际创建窗口；Edge 153 headless 实际请求 Web 包 index/JS/WASM/icon 全部 HTTP 200。

发行包继续由 `scripts/package.ps1` 生成 Windows、Web 和双仓源码 ZIP 及 SHA256SUMS；最终上传前以配对 tag、`release-pair.json` 和 GitHub CI 结果共同核对两仓完整 SHA。
