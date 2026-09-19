# worldedit（世界编辑系统）

worldline 的 Rust / egui 作者工作台，用于人物资料、世界设定、多文件故事、关系图、时间偏序、状态和叙事锚点创作。语言解释与结构修改统一由 worldline-core 完成。

## 安装与启动

下载发行包并解压，运行 worldedit.exe，选择作品目录。空目录自动建立雾港示例；已有作品需要根目录 world.wl。也可以传目录：

```powershell
./worldedit.exe "D:/作品/我的世界"
```

工作区递归索引全部 `.wl`，允许子目录分区；桌面自动刷新外部文件变化，同文件未保存冲突会提示。附件须先放入工作区再引用。导出保留完整目录文件，包含未引用素材、README 与隐藏的 `.agent`。规则见 [工作区说明](docs/workspace.md)。

## 创作功能

- 时段包含：大时段可包含多层子时段，时间线按父子层级展示；点击“＋ 子时段”创建，双击标题修改上级。
- 分支决策：事件详情顶部直接添加/修改/删除选项，编辑条件、一次性标记、选后正文和末尾目标，跨线目标自动选择漂流。
- 时间线与执行关系图：编辑事件、连接、前置要求和效果；时间 follows 与执行路径分开。
- 人物与世界：静态属性、人物关系、别名、事件反查、完整资料阅读。
- 资料与状态：查看全部标签、按名称/ID 搜索多选、就地创建并添加、给标签添加标签与递归查询；状态集合、变化出处、独立锚点及关联素材。
- 标签条件与变化：选取状态和标签生成包含/不包含、全部/任意条件；为选后正文及事件效果添加替换、增加、移除状态标签的动作。详见 [标签操作](docs/tags.md)。
- 正文：跨文件阅读、对象链接、源文件高亮、诊断定位与工程搜索。
- Wiki：关键词、别名与释义维护；正文、资料及试玩中的关键词自动链接，同名词条可选择，出现位置可反查并定位。详见 [Wiki 词条](docs/wiki.md)。
- 工程：新增/引用源码、保存全部、另存、完整导出、撤销重做和未保存保护。
- 演练：选择、变量、访问次数、实际状态与历史；不把分支源码变化当成唯一当前事实。
- 地图画布：浏览工程中已注册的地图、PNG/JPEG 栅格图层及点线面标记；缩放、平移、图层显隐和查看资料不会修改工程。见[地图浏览](docs/maps.md)。

Ctrl+S 保存全部，Ctrl+O 选择工作区，Ctrl+Shift+F 搜索。修改前后应检查工程诊断。

## AI 创作

创作技能位于 [.agent/skills/worldedit-authoring/SKILL.md](.agent/skills/worldedit-authoring/SKILL.md)。将技能交给 AI 或复制到作品的 `.agent/skills/`；是否自动发现取决于使用的 agent，不假定所有客户端自动扫描 `.agent`。技能要求先反查关联人物、事件、状态与锚点，并向作者提醒本次改稿影响。

[CLI 功能覆盖表](docs/cli-coverage.md) 明确区分可自动分析/演练的功能与仍需界面的功能。当前没有远程控制运行中编辑器全部功能的 CLI。

## 源码构建

版本选择与复测请先阅读[配对构建与回归基线](docs/paired-ci.md)；worldline 必须检出本仓库 `compatibility.json` 指定的完整 SHA。

把两个 GitHub 仓库克隆到同级目录，名称保持如下；worldline 的实际 GitHub 地址由仓库所有者提供：

```text
父目录/
  worldline/Cargo.toml
  worldedit/Cargo.toml
```

在 worldedit 目录执行：

```powershell
cargo build --release --locked
cargo test --locked
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo run -- ../worldline/examples/harbor-world
```

本仓库通过 `../worldline/core` 和 `../worldline/runtime` 路径依赖语言仓库，无需父目录 Cargo.toml。分开上传时保留各自 Cargo.lock、工具链、许可证、源码和测试，排除 target/dist/releases。发行包内附语言工具，无需用户安装 Rust。

## 浏览器版

在本目录运行 start-web.cmd 或 `./start-web.ps1`，脚本准备缺失的 Web 工具并启动本地服务。可传 `-Port 8788 -NoOpen`。手动发布构建：

```powershell
rustup target add wasm32-unknown-unknown
trunk build --release --locked
```

静态产物在 dist，必须经 HTTP 服务打开；浏览器需要 WebAssembly 与 WebGL。网页直接运行同一个 WorldeditApp，共用语言分析与业务界面。字体只嵌入仓库中的 Noto Sans SC，许可证见 assets/fonts/OFL.txt。

打开文件夹导入全部子目录与素材，也可打开 ZIP 工程包。浏览器只持有授权快照，不能自动观察磁盘后续变化；需重新导入。保存下载 ZIP 并尝试存入浏览器本地存储，达到配额会保留未保存标记。ZIP 限制 4096 文件、64 MiB。没有云端同步；关闭浏览器标签页的操作由浏览器管理。

## 发布与边界

运行 [scripts/package.ps1](scripts/package.ps1) 完成 Windows 与 Web 打包；细节见 [发布说明](docs/release.md)。静态结构检查不判断自然语言设定真假，时段不计算历法，协作采用文件/Git 合并，尚无云端实时共同编辑。许可证见 [LICENSE](LICENSE)。
