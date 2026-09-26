# WP-10 完成交付记录

对应 `ikzerok/worldedit#6`「网络与通用资料 UI」。本文件保留早期分片实现历史，并在末尾记录最终补全证据；最终全阶段验收见 `docs/wp17-acceptance.md`。
开发分支：worldedit `ticket/wp10-world-associations`，worldline `ticket/wp10-graph-views`。

## 已接入的作者功能

在「资料与状态」页使用「＋ 通用资料」新建实体，填写名称、分类、正文及自定义属性。
分类有地点、组织、物品、概念、历史条目、物种和自定义值；实体不要求先放到地图上。
目录和统一阅读页均能打开实体表单；编辑写回真实来源文件，不会写入当前偶然选中的文件。
目录列表显示对象类型和 ID，同名对象不被合并。已有 ID 在普通资料表单中保持不变。

通过「关系类型」菜单新建或编辑名称、方向、反向读法与可选端点类型限制，再使用
「＋ 独立关系」明确选择类型和两端对象。关系说明、来源、范围和自定义属性完整保留；
同一对端点可以有多条独立 ID 的关系。没有自动生成反向、传递或从正文提及推断的关系。

表单捕获打开时的版本和完整内容基线；工程变化后保留草稿并禁止旧输入覆盖新内容。
应用操作通过 core 事务提交，一次进入撤销历史；取消或失败不增加历史，不保存文件。
删除先显示正文、地图和共享网络引用及诊断，检查完整、无引用且明确勾选后才能删除。
删除资料不等于隐藏显示；应用仍需作者按正常「保存全部」流程写入磁盘。

旧 1.9 工程不会被自动升级。实体及独立关系功能要求显式选择语言 1.10；关系写入仍由
core 校验工作区声明的 `content.relations.v1` 能力。不会默认接受未授权的隐式迁移。

## core 支撑接口

`worldline-core::graph_views` 提供注册共享布局的读取、保存与删除事务，复用安全路径、
Project 文档缓冲、Revision 与内容基线，不由 UI 拼写 JSON。只提高展示修订，不改变
源码或运行指纹；删除布局不删除实体或关系。未知可选字段保留，未知必需能力只读。
损坏 JSON、重复键、无效坐标、歧义注册路径、缺失引用与暂无法解析的引用有独立诊断。
布局中的中心、坐标位置、隐藏关系进入删除影响；筛选使用的关系类型也受到删除保护。
多关系类型 OR 筛选及继续查询沿用 core 的局部关系查询，不扩大到全量图推断。
契约与诊断定义见同级 worldline 的 `spec/presentation.md`、`spec/diagnostics.md`。

## 早期分片验证（历史）

core 全工作区测试 307 项通过；编辑器测试 93 项通过，没有失败或忽略。
本次新增 core 14 项、表单模型 4 项、真实 egui 按钮交互 4 项，合计 22 项回归。
按钮回归覆盖应用、陈旧表单禁用、删除勾选确认及撤销/重做；这是无显示环境下的
真实 egui 事件测试，不等价于已显示桌面或浏览器的人工可视化验收。
两仓格式检查、core/编辑器原生严格 clippy、编辑器 WASM 严格 clippy 均通过。
core 全工作区和编辑器原生 release 构建通过。
编辑器 WASM release 构建也已通过，未执行浏览器打包部署或人工可视化验收。

执行日志保留在组合目录 `target/wp10-check/`：
`core-tests-final.log`、`editor-tests-final.log`、`authoring-ui.log`、
`graph-hardening-green.log`、`core-clippy.log`、`editor-clippy-final.log`、
`editor-wasm-clippy.log`、`core-release.log`、`editor-native-release.log`、
`editor-wasm-release.log`、`final-checks.json` 和 `release-checks.json`。
这些是本次机器上的本地证据，不宣称远程 CI 已执行。

## 历史阻塞（已解除）

早期远程写入曾阻塞网络状态/画布文件，因此当时保持工单打开。2026-09-26 重试后 `network_state.rs`、`network.rs`、共享布局保存、分页/筛选/导航及 D5 性能门均已进入正式测试目标并通过；该段仅作为开发历史，不再表示当前缺口。

## 本地交付

WP-10 最初共享网络 core 提交为 `5db4076`，后续功能随 WP-11–16 继续累积；最终发布配对以 `compatibility.json` 与 `docs/wp17-acceptance.md` 为准。

## 2026-09-26 续接：个人网络浏览状态

新增 `src/app/network_state.rs`，消费 core 的受限查询和布局草稿接口。
包含确定性初始布局、类型/方向/深度筛选缓存、受限分页、隐藏/恢复、
节点拖动与 Esc 取消所需状态、缩放锚点、个人镜头与最近访问返回。
这些操作不接收 Project，不写作者文件，不创建或删除语义关系。

`tests/network_state.rs` 的 4 项回归通过，包含 600 条关系的完整分页检查、
100 次重复查询缓存检查、拖动取消、基线更新取消旧拖动、镜头返回与布局读取。
本次编辑器完整测试共 97 项通过；格式与原生全目标严格 Clippy 通过。
日志：组合目录 `target/ticket-completion-20260926/editor-state-tests.log`、
`editor-state-clippy.log`。

## 2026-09-26 重试结果：WP-10 网络页已接通

此前的写入拦截已通过分块文件写入绕过。新增 `src/app/network.rs` 并把
“世界关联”加入主导航；资料页“查看关联”进入同一局部网络。界面支持 1/2 层、
出向/入向/双向、关系类型筛选、受限分页、节点拖动、缩放/平移、双击换中心、
返回个人访问路径、隐藏/恢复关系，以及正式关系与对象资料互访。

浏览、拖动、缩放和隐藏只改 `NetworkState`；真实 egui 回归验证连续浏览不改变
Project 基线和撤销历史。只有“保存共享布局”调用 core `graph_views` 展示事务，
写入注册的 GraphViewDocument 并进入一次撤销历史；语义关系未被复制进布局。

完整 worldedit 回归现为 98 项通过、0 失败、0 忽略。原生和 wasm32 严格 Clippy
均通过，原生/WASM Release 构建通过。D5 数据为 1,000 对象 / 3,000 关系；
Release 160 个暖态样本实测网络帧 P95=10.935ms、最大=11.245ms，
暖态资料切换 P95=4.112ms、最大=5.383ms，分别低于 33ms / 200ms 门槛。
这是无显示 egui 渲染实测，不冒充人工桌面/浏览器视觉验收。

## WP-17 最终确认

最终全量回归为 worldline 325 项、worldedit 103 项测试通过。D5 release 网络帧 P95=11.073ms、暖态资料切换 P95=4.128ms；Windows 发布版实际创建窗口，Edge 153 headless 实际加载 Web 包的 JS/WASM。WP-10 现已作为 65 项总验收的一部分通过，最终证据见 `docs/wp17-acceptance.md`。
