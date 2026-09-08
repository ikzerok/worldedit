# worldedit 仓库约束

本仓库是 Rust/egui 编辑器，依赖同级 worldline 仓库的 core/runtime。全部语言解释、结构编辑、诊断、关系图和时间线来自 core；UI 不另写解析器。语言语义修改先在 worldline/spec 更新契约。

工作区是唯一文件边界，桌面自动刷新保留未保存冲突，完整导出不得漏掉未引用文件。浏览器工作区是导入快照，不能宣称持续监视磁盘。详细边界见 docs/workspace.md。

检查命令：cargo fmt --all -- --check；cargo test --locked；cargo clippy --all-targets --locked -- -D warnings；cargo clippy --target wasm32-unknown-unknown --locked -- -D warnings。发布构建见 scripts/package.ps1。

文档中文，代码标识符英文。创作作品时读取 .agent/skills/worldedit-authoring/SKILL.md，技能说明磁盘创作、关联提醒和 CLI/UI 边界；开发编辑器时不套用作品创作流程。CLI 能力表见 docs/cli-coverage.md。
