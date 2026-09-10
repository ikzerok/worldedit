# 构建与发布

## 0.2.0 更新

Wiki 已集成到编辑器：可按关键词、ID 和别名查找资料，创建或修改独立词条、释义与别名。正文、资料、源码预览和演练中的关键词自动链接；同名词条提供候选，显式链接保留指定目标。注释索引显示全部出现位置并可跳回源码，查阅演练词条不会推进回合。

本版本需要同级 worldline 0.2.0。使用方式见 [Wiki 词条](wiki.md)，已完成的自动检查和桌面操作见 [验证记录](verification.md)。

## 源码仓库

worldedit 和 worldline 分别上传各自目录，克隆到同级位置。各仓库保留 Cargo.toml、Cargo.lock、rust-toolchain.toml、LICENSE、README、AGENTS、源码、测试和持续集成。worldedit 另保留图标、开放字体与许可证、Web 入口、启动脚本和 .agent 创作技能。

target、dist、releases、日志和临时文件不是源码，不上传。父目录旧研究、规划、截图与图标生成过程不属于这两个仓库。不能删除 core 编译时嵌入的 worldline/examples/harbor-world，也不能删除字体许可证。

## 门禁

分别在两个目录执行 fmt、test、clippy。worldline 使用 --workspace；worldedit 还执行 wasm32 clippy。工具链由各自 rust-toolchain 固定。CI 的 worldedit 流程从同一 GitHub 所有者的 worldline 仓库读取依赖；若托管位置不同，修改 checkout 的 repository 字段。

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

上传 source/worldline 的内容到 worldline 仓库，上传 source/worldedit 的内容到 worldedit 仓库；不要把 source 或本地组合父目录作为第三层套入仓库。两者默认分支使用 main，编辑器 CI 默认读取同一所有者的 worldline 默认分支，先上传 worldline，再上传 worldedit。保留隐藏的 .github、.agent、.gitignore 和 Cargo.lock。父目录原有 .git 历史不在源码包中。

Windows/Web ZIP 与 SHA256SUMS.txt 作为 GitHub Release 附件；不提交到源码树。是否完成线上发布以 GitHub Release 页面及附件为准。
