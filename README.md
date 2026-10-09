# worldedit（世界编辑系统）

worldline 的 Rust / egui 作者工作台。0.33 连接译文制作与状态巡检、同一运行时的真实译文体验，以及正文中的正式人物和实体关联创作。Studio 默认，五种结构与十组十七套配色继续独立组合。语言语义、来源与保护由 core/runtime 统一提供。

本版功能与验证范围见[0.33版本说明](docs/releases/v0.33.0.md)，公开发行状态以对应 GitHub Release 为准。默认语言 1.9、最高既有显式版本 1.13 保持，不新增 DSL，也不自动升级作品。

升级边界：运行轨迹与运行检查点（ReplayTrace / ReplayCheckpoint）继续精确匹配 runtime_version；0.32 及更早记录不能在 0.33 重放、比较或生成已验证审阅，应重新录制。合法旧轨迹可只读导入查看，读取成功不代表可执行。普通 Story Save 继续按格式、能力和指纹独立校验；locale 记录还需匹配其展示身份。升级前保留完整工程和原记录。

## 0.33 译文与正文关联

- [译文工作台](docs/localization-workbench.md)：稳定目录、源译编辑、后台预览、一次应用与独立保存
- [正文资料关联](docs/manuscript-workspace.md)：就地引用、正式人物/实体复合创建、旁查返回与迁移预览
- 真实试玩、草稿试演、路径回放、双路线和审阅报告共用同一 locale 身份与 runtime

## 0.32 同一当前稿的作者闭环

- [普通外改协调](docs/workspace-reconciliation.md)：基线、本地、磁盘三方完整身份，明确逐项决定，候选重验、撤销和另行保存
- [未应用正文隔离试演](docs/draft-rehearsal.md)：实际草稿编译和独立运行会话，真实状态与来源，返回原稿
- [筛选范围审稿本](docs/manuscript-delivery.md)：同一书稿查询的连续全分支审阅和作者私密 Markdown 交付
- [查询范围巡检](docs/catalog-scope.md)：相同不可变 typed 范围里的对象、地图绑定和正式关系，返回与过期保护

## 0.31 作者工作流

- [可视化模板设计](docs/template-designer-0.31.md)：字段与分组、表单试填、两侧影响、明确应用及草稿保护
- [章节检索与导航](docs/manuscript-navigation-0.31.md)：完整范围查章、树/列表/卡片、可选列、分页与正文往返
- [真实运行状态检查](docs/state-inspector-0.31.md)：查值与变化、首次/前次真实观测、声明回源及证据边界

## 0.30 作者工作流

- [新外观与个人设置](docs/appearance-0.30.md)：Studio 默认；结构、配色、密度与字号独立，预览后应用或取消
- [全功能工作台](docs/workbench-shell-0.30.md)：导航、工具与内容分层；窄窗、Focus 抽屉和 Ledger 重排仍保留操作入口
- [开始写作](docs/start-writing-0.30.md)：书名/章名优先，核对正式来源与原子改动，直接进入空正文
- [统一对象筛选与分页](https://github.com/ikzerok/worldline/blob/main/spec/object-search.md)：完整身份、类型约束、路径与别名、准确总数和来源保护

## 0.28 作者工作流

- [静态可执行依赖](https://github.com/ikzerok/worldline/blob/main/spec/executable-context.md)：查看调用、读写和语境，回到真实源码
- [可读试玩审阅](https://github.com/ikzerok/worldedit/blob/main/docs/playthrough-report.md)：重新验证单条路径，预览后明确复制或导出作者报告
- [地图作者位置](https://github.com/ikzerok/worldedit/blob/main/docs/map-author-context.md)：资料、源码与地图间的有效身份返回
- [平行连接审阅](https://github.com/ikzerok/worldedit/blob/main/docs/parallel-edges.md)：准确数量、完整分支条件与逐条来源

## 使用入口

- [连续键盘试玩](https://github.com/ikzerok/worldedit/blob/main/docs/keyboard-play.md)：成功推进后的可选项与结束动作焦点，不替作者继续选择
- [资料约束影响完整性](https://github.com/ikzerok/worldline/blob/main/spec/schemas.md)：缺失源码、已知影响与实例违规分别说明

- [真实变量写入与回源](https://github.com/ikzerok/worldedit/blob/main/docs/variable-write-evidence.md)：两条路线的前后值、实际动作与来源返回

- [当前源码行列定位](https://github.com/ikzerok/worldedit/blob/main/docs/source-coordinates.md)：物理位置、当前稿预览、准确跳转与返回

- [当前源码结构与可信定位](https://github.com/ikzerok/worldedit/blob/main/docs/source-outline.md)：当前文件声明、精确范围、键盘跳转与作者位置返回

- [实体资料移源](docs/entity-source-move.md)：明确身份与目标、两侧预览、一次事务、撤销及真实来源

- [逐处审阅后安全改稿](docs/selective-replace.md)：当前项与勾选集合、命中上下文、前后预览、一次撤销与保存

- [可信全分支审稿](docs/manuscript-review.md)：静态条件与选择边界、人物身份、当前稿回源和完整性提示

- [世界资料批量导入与修订](docs/catalog-import.md)：文件快照、列映射、逐行审阅、原子应用、一次撤销与回源


- [真实路线对照](docs/route-comparison.md)：选择A/B → 验证当前稿 → 结果与动作 → 安全回源与返回

- [全局工程问题](docs/project-problems.md)：筛选、真实来源精度、部分覆盖、后台刷新与键盘定位
- [源码自动换行](docs/source-wrap.md)：长行折行、物理行号与有效作者位置保持
- [人物焦点与时间问题](docs/focused-author-workbench.md)：有类型局部关系、真实来源、闭环证据和键盘返回

- [安全源码路径](docs/source-lifecycle.md)：预览正式引用与资源变化，整批应用、一次撤销，保存才落盘
- [内容优先工作台](docs/content-first-workbench-0.16.md)：地图空间、固定操作区与对象来源层级
- [地图与虚线](docs/maps.md)：受控样式、能力保护与安全 SVG 边界

- [矢量地图](docs/vector-authoring-0.15.md)：绘制与节点编辑、组和图层、对象绑定、撤销与保存、SVG 预览和交换
- [发布给读者](docs/reader-publishing.md)：选择内容 → 核对资源 → 预览页面 → 确认生成；字段和附件须分别授权
- [共同界面布局](docs/visual-system-0.15.md)：主导航、索引、内容、检查器和参考区的职责及主题规则
- [后台计算与取消](docs/web-worker.md)：桌面线程与同源 Web Worker 的进度、资源限制和过期结果保护

从[时间约束与可信重放](docs/replay-timeline-workflow.md)了解双路线改稿；跨仓相对链接用于 `worldline` / `worldedit` 同级检出。普通试玩见[有界试玩](docs/bounded-play.md)，键盘与正文教学见[作者反馈](docs/author-feedback.md)；既有[跨季偏序与人物资料引用](docs/static-authoring-0.10.md)继续支持。

0.29 起，应用包和源码归档不再分发样例工程、`spec/examples/` 或独立视觉演示 HTML。新建作品只有可直接试玩至结束的空白 `world.wl`；字段模板和必要的回归测试输入仍保留。历史设计与版本记录中的演示链接用于完整 Git 检出，不表示这些文件存在于发行归档。
[稳定ID重构与存档边界](docs/safe-id-refactor.md)说明逐处预览、原子提交和state所属实体的安全拒绝；[查找与作者位置](docs/search-author-context.md)说明模式保持、源码回退与返回保护。

## 安装与启动

下载发行包并解压，运行 worldedit.exe，选择作品目录。空目录自动建立只有 `world.wl` 的空白作品；已有作品需要根目录 `world.wl`。也可以传目录：

```powershell
./worldedit.exe "D:/作品/我的世界"
```

工作区递归索引全部 `.wl`，允许子目录分区；桌面自动刷新外部文件变化，同文件未保存冲突会提示。附件须先放入工作区再引用。导出保留完整目录文件，包含未引用素材、README 与隐藏的 `.agent`。规则见 [工作区说明](docs/workspace.md)。

“发布给读者”按显式选择生成离线静态包，与完整工程备份分开。选择、预览和安全边界见[静态读者包发布](docs/reader-publishing.md)。

## 创作功能

- 资料结果排序：按名称或对象类型升降序，跨页保持稳定；可恢复默认顺序并保存共享定义。详见[资料查询视图](docs/catalog-views.md)。

- 时段包含：大时段可包含多层子时段，时间线按父子层级展示；点击“＋ 子时段”创建，双击标题修改上级。
- 分支决策：事件详情顶部直接添加/修改/删除选项，编辑条件、一次性标记、选后正文和末尾目标，跨线目标自动选择漂流。
- 条件证据：试玩中按真实求值查看复合条件各项值、括号层级及未满足原因；错误冻结本次尝试，查看不消耗随机数或修改作者工程。
- 时间线与执行关系图：编辑事件、连接、前置要求和效果；时间 follows 与执行路径分开。
- 人物与世界：静态属性、人物关系、别名、事件反查、完整资料阅读。
- 多资料旁查：钉住最多两个独立对象面板，分别导航和返回；窄窗口切换查看。见[资料旁查](docs/reading-panels.md)。
- 资料与状态：查看全部标签、按名称/ID 搜索多选、就地创建并添加、给标签添加标签与递归查询；状态集合、变化出处、独立锚点及关联素材。
- 标签条件与变化：选取状态和标签生成包含/不包含、全部/任意条件；为选后正文及事件效果添加替换、增加、移除状态标签的动作。详见 [标签操作](docs/tags.md)。
- 正文：跨文件阅读、对象链接、源文件高亮、诊断定位与工程搜索。
- Wiki：关键词、别名与释义维护；正文、资料及试玩中的关键词自动链接，同名词条可选择，出现位置可反查并定位。详见 [Wiki 词条](docs/wiki.md)。
- 工程：新增/引用源码、保存全部、另存、完整导出、撤销重做和未保存保护。
- 演练：选择、变量、访问次数、实际状态与历史；不把分支源码变化当成唯一当前事实。
- 地图画布：旧点线面、文字、底图与测量继续可用；原生矢量支持曲线、文字、组、变换、节点编辑和对象绑定。浏览与文档编辑有独立入口，见[矢量地图工作流](docs/vector-authoring-0.15.md)和[既有地图操作](docs/maps.md)。

Ctrl/Cmd+S 保存全部，Ctrl/Cmd+O 选择工作区，Ctrl/Cmd+Shift+E 打开导航与文件，Ctrl/Cmd+F 当前稿查找，Ctrl/Cmd+Shift+F 工程搜索；其余命令见“编辑”菜单。修改前后应检查工程诊断。

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
python scripts/check-source-lines.py
cargo build --release --locked
cargo test --locked
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo run -- /path/to/your/workspace
```

Rust 源码按职责拆分；`check-source-lines.py` 对纳入 Git 的源码和测试执行单文件 600 物理行上限，配对检查也会对 worldline 执行同一规则。

本仓库通过 `../worldline/core` 和 `../worldline/runtime` 路径依赖语言仓库，无需父目录 Cargo.toml。分开上传时保留各自 Cargo.lock、工具链、许可证、源码和测试，排除 target/dist/releases。发行包内附语言工具，无需用户安装 Rust。

## 浏览器版

在本目录运行 start-web.cmd 或 `./start-web.ps1`，脚本准备缺失的 Web 工具并启动本地服务。可传 `-Port 8788 -NoOpen`。手动发布构建：

```powershell
rustup target add wasm32-unknown-unknown
trunk build --release --locked
```

静态产物在 dist，必须经 HTTP 服务打开；浏览器需要 WebAssembly 与 WebGL。网页直接运行同一个 WorldeditApp，共用语言分析与业务界面。字体只嵌入仓库中的 Noto Sans SC，许可证见 assets/fonts/OFL.txt。

打开文件夹导入全部子目录与素材，也可打开 ZIP 工程包。浏览器只持有授权快照，不能自动观察磁盘后续变化；需重新导入。保存下载 ZIP 并尝试存入浏览器本地存储，达到配额会保留未保存标记。完整工程 ZIP 限制为 4096 文件、64 MiB；读者站使用独立的 10000 文件、128 MiB 原始内容与 128 MiB ZIP 预算，不能混作工程备份额度。没有云端同步；关闭浏览器标签页的操作由浏览器管理。

## 发布与边界

运行 [scripts/package.ps1](scripts/package.ps1) 完成 Windows 与 Web 打包；细节见 [发布说明](docs/release.md)。静态结构检查不判断自然语言设定真假，时段不计算历法，协作采用文件/Git 合并，尚无云端实时共同编辑。许可证见 [LICENSE](LICENSE)。


## 0.8.0 作者工作区

本版配对显式语言1.11，新增纯规则、可返回共享片段、类型化state集合和角色台词，默认1.9及旧1.10作品不自动升级。正文/结构/源码共用core源缓冲，书稿直接进入章节正文，编排删除不删除来源。个人布局、阅读宽度/字号/行距、明暗主题、双参考和最近位置留在设备，资料列仅改变显示。

阅读发布逐项显示实际页面文本与空页提示，property字段默认未选，引用不会自动公开目标。详见[作者工作区](docs/author-workspace.md)、[正文与书稿](docs/manuscript-workspace.md)及配对worldline的[语言1.11规范](https://github.com/ikzerok/worldline/blob/a13cdef2dc04ca4ce97bf38d0fb00aad100611f5/spec/language-1.11.md)。

**兼容提醒：** 负数rnd的旧错误结果在本版按规范纠正，不保证旧错误轨迹跨版本重放相同；保存的既有值与RNG状态不重算，合法非负seed序列保持。CI构建不等于各平台原生交互已经验收，具体交互范围以独立验收报告为准。

作者工作区的个人布局、停靠参考、快速命令与草稿退出保护见 [作者工作区说明](docs/author-workspace.md)；正文中心编辑与书稿重组见 [书稿写作工作区](docs/manuscript-workspace.md)。

矢量地图作者工作流与资源限制见 [0.15 作者契约](docs/vector-authoring-0.15.md)；固定 5000 节点 release headless 探针命令和原生验收边界见 [性能记录](docs/performance.md)。
