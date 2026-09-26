# 2026-09-26 ticket 实施记录

本轮请求是完成两仓所有 ticket；截至本记录尚未全部完成。初始开放 43 张，CAP-09C 已按用户要求删除；目前 worldline 开放 8 张、worldedit 开放 18 张。主实施分支为两仓各自的 `codex/complete-tickets-20260926`，原型与并行能力票保留独立工作树/分支，未合并或发布。

## 已实现、待完整验收

### worldline#13 / CAP-01A

- core 组合意图包含新建/复用对象、正文稳定引用和可选地图入口，候选 Project 全部检查后一次提交；返回变更文件、引用影响与新内容基线。
- 预览零写入，拒绝陈旧选区/基线、外部修改、未知能力、非法几何/锁层，保留原文未选部分和未知 JSON 字段。
- 8 项新增公开 API 回归覆盖 Unicode、同名不同身份、别名、注释误选、跨文件保存后撤销/重做及失败零部分更新。
- 首期仅进程内 API；CLI/RPC 组合入口按 ticket 允许的分期方式在协议明示。CAP-01B 的选文建档与 CAP-01C 的地图建档现已调用组合入口，CLI/RPC 配对回归仍待完成。

### worldedit#11 / CAP-02C

- 最多两个独立钉住面板，每个最多 64 条历史；宽屏并列窗口，1040×660 以切换面板退化，个人阅读状态不写工程。
- 同名按 kind+ID 区分；删除对象显示失效身份；工程切换清空面板并保护未提交表单；Web 离页提示覆盖纯表单草稿。
- 新增状态测试和 egui 点击回归，覆盖钉住、窄屏切换/关闭、源码入口、时段/实体草稿、失效对象与独立临时阅读窗口。
- 原生/Web 实际交互、键盘焦点与缩放恢复尚未验收，不能将 headless 测试写成实际作者验证。

### worldedit#13 / CAP-01B

- 正文源码中的 `@` 候选按 kind+ID 消歧，支持方向键/Enter 与 Esc；选文建档走 core 组合事务，正文链接可回到原编辑光标。
- IME 组合与外部刷新同时发生时保留本地草稿并阻止覆盖。egui 真实事件回归、123 项测试及 WASM 检查通过，提交 `e5eb266`。原生桌面在独立 `D:/Temp/worldedit-cap01b-gui-20260926` 作品实际输入 `@林舟`、看到人物/状态同名候选、Enter 生成 `[[character:lin|林舟]]`、保存后磁盘第 4 行含稳定引用。Web、真实 OS 输入法、返回光标及完整保存重开路径仍待验收，票保持开放。

### worldline#14 / CAP-02A

- 书稿 schema、能力、编排命令、源位置/统计/阅读顺序投影已在 core 提交 `89efd75`，独立构建与测试通过，票已关闭。worldedit#15 的编辑器工作台现已集成。

### worldedit#14 / CAP-01C

- 地图编辑展示模式新增“点落位后新建地点资料并放置入口”。落点保留地图、图层、坐标和内容基线；提交调用 core 组合事务，一次记录资料与入口，不推断关系语义。
- 真实 egui 点按测试覆盖新建、单步撤销/重做、保存/重开；陈旧基线拒绝且保留输入。提交 `50e84ec`、`4c3f222`。桌面/Web 端到端交互未验证，票保持开放。

### worldedit#15 / CAP-02B

- 书稿工作台支持创建、分节与章节、目标选择、状态、章节树/卡片/列表、正文草稿及 core 阅读投影；编排应用与撤销均经 core。
- 独立分支实现集成于 `64abcf4`，与 CAP-01B 的测试冲突已解，新增 7 项真实 egui 事件回归。配对 editor 116 项单元与 16 项集成测试、严格 Clippy、wasm32 check 通过；桌面/Web 端到端交互未验证，票保持开放。

### worldline#15 / CAP-03A

- core 组合查询、共享保存查询、只读待办投影以及复用同一 DTO 的 `wl catalog-query` / RPC `catalog.query` 已提交 `25f4436`，core 票关闭。游标预算、注册 ID 与文件 ID、扩展字段重排三个自审问题已修。
- core/CLI/RPC 公共回归、D5 本机样本和协议边界已记录。同步 CLI/RPC 不支持进行中中断，未宣称支持。worldedit#16 的筛选与待办界面已并入 `c63fdf0`，集成测试窗口冲突修于 `d63992d`；123 项编辑器单元与 16 项集成测试、严格 Clippy、格式检查通过。桌面/Web 交互、浏览器中途取消、收藏跨进程持久化与同一端到端修复回归仍缺，票保持开放。

### worldline#16 / CAP-04A 与 worldedit#17 / CAP-04B

- core 三方字段/段落差异、原文字节范围、冲突、引用影响、截断与可验证基线已提交 `163e0ff`，18 项 collaboration 回归通过，core 票关闭。
- editor `85a19f0` 显示三方差异与原文后备，窄窗用标签切换；预览按基线缓存，过期后须重新比较，安全采纳可一次撤销。egui 回归、Web release 打包与配对检查通过。逐项编辑解决方案及真实原生/Web 文本选择、IME、滚动和关闭路径仍待验收，editor 票开放。

## 研究交付

- worldedit#32 / EDS-01：[官方交互观察与证据边界](design/editor-system/20260926/eds01-evidence.md)。四主样本各四条 DOC 观察卡及失败/恢复路径，五补充样本、12 模式反例和替代方案。GUI/RUN/USER 缺口明确保留，研究票已按文档交付关闭。
- worldedit#33 / EDS-02：[任务、对象与入口草案](design/editor-system/20260926/eds02-tasks-and-objects.md)。四条任务旅程、对象生命周期、五类结构和旧 12 页面映射；频率与角色标为假设，不冒充用户访谈。后台研究连接中断后由主任务依据已核查代码补齐，仍待作者验证。
- worldedit#34 / EDS-03：[编辑器状态、身份与事件路由契约](design/editor-system/20260926/eds03-state-contract.md)。定义六种“当前”、类型化身份、状态所属域、事务转换及 owner/IME/冲突负例；这是设计契约和验收清单，不宣称生产 UI 已全面实现。
- worldedit#35 / EDS-04：[六区域显隐与 A/B/C 比较](design/editor-system/20260926/eds04-layout-comparison.md)。三种结构用同一组任务与示例数据比较，B 暂列下一轮验证首选，完整自由 dock 暂不采用；抛弃式交互原型单独保存在 `codex/eds04-prototype` 的 `5e44564`，尚待实际作者评估。
- worldedit#36 / EDS-05：[跨投影交互契约](design/editor-system/20260926/eds05-cross-projection.md)。定义类型化导航与返回点、临时/固定资料库、拖放零写入、长标签文字后备与 core 实际 250 节点/500 边上限；仍需真实事件和作者可达性验收。
- worldedit#37 / EDS-06：[检查器、参数与正文草稿契约](design/editor-system/20260926/eds06-inspector-drafts.md)。列出跟随/固定及多选矩阵、默认/缺失/未知值、渐进分组、重置范围、三方比较与 IME/陈旧负例；这是设计，未声称生产界面完成。
- worldedit#38 / EDS-07：[时间、控制流、书稿与关系图视觉语法](design/editor-system/20260926/eds07-graph-grammar.md)。四种投影分开命名与图例，区分选择、预览和运行位置；为并行边、自环、长标签与截断定义文字后备和负例。未做作者理解测试。
- worldedit#39 / EDS-08：[正文、临时侧览与可恢复专注](design/editor-system/20260926/eds08-writing-preview.md)。区分主编辑、临时/固定旁查和阅读投影，定义选文/IME/无鼠标、专注恢复、窄窗与长文验收；行宽仍待实际验证。
- worldedit#40 / EDS-09：[统一命令、焦点路由与撤销边界](design/editor-system/20260926/eds09-command-routing.md)。统一命令描述和按钮/菜单/快捷键语义，列出模态→IME→焦点区→全局矩阵、分层取消及非拖动替代；仍需生产接入与真实事件验证。
- worldedit#41 / EDS-10：[基于状态的视觉系统与组件规范](design/editor-system/20260926/eds10-visual-system.md)、[高保真样板和浏览器证据](design/editor-system/20260926/eds10-sample-evidence.md)。四任务、明暗/密度、八类组件、窄窗冲突与 11 张实际浏览器截图已交付，设计票关闭；真实原生控件、IME、读屏和完整无障碍验证仍缺。
- worldedit#42 / EDS-11：独立 `codex/eds11-egui-prototype` 分支提交 `20b57e7`，原生和 Web 实际界面已做范围内检查；字体、IME、响应性能及作者参与验证仍缺，票保持开放。
- worldedit#43 / EDS-12：[目标作者验证预注册方案](design/editor-system/20260926/eds12-validation-protocol.md)。列出交叉任务、匿名原始字段、安全停止门、平台/性能口径和采用/修改/拒绝决策流程；EDS-11 和真实参与者未完成，结果全部空白，票保持开放。

## 已决定的排除

- 用户明确“不考虑游戏引擎适配器”，并要求直接删除对应 ticket。worldline#27 已删除（GitHub 返回 HTTP 410）；#24 的不采用决定已完成。
- worldedit#9 总览和 #28 已移除适配器实施范围与依赖；#28 改为“台词审阅与本地化工作台”，只依赖 worldline#26。本地化没有因排除引擎而被取消。
- 决策记录在配对 core 的 `docs/adr/0001-no-engine-adapter.md`。

## 审查与验证口径

用户确认以 core `889febe99f7999f8da4c28583320da61e6be13e2`、editor `9bef07c4b0e10196787ee0447b15be4261847903` 为审查基准，测试入口为公开 Project/core、CLI/RPC 与真实 egui 事件。

- Standards：两轮均无发现。
- Spec：core 引用影响缺项已修；editor 的时段草稿、Web 离页提示和跨阅读窗口关闭问题已修，并补回归。实际双端与作者测试缺口独立记录。
- 最新不可变配对检查为 core `163e0ff77c4f86543fef5fdce2d4fbb0fad2df11`、editor `85a19f0b31ab577b9789d1ac9487ccaf3570ab5c`，完整日志在本地 `target/paired-check/20260926T1242102923154Z/`。脚本完成两仓格式、完整测试、严格 Clippy、原生构建与编辑器 WASM Clippy/构建。CAP-03B 集成后已有单仓回归；CAP-01A / CAP-04C / CAP-05A 后续集成时需重新配对检查；本机日志不提交源码。
- 检查脚本：`scripts/check-pair.ps1` 全部通过，分别指定两仓 Cargo.toml，覆盖格式、完整测试（core/CLI/runtime/agent 合计 333，editor 合计 110）、Clippy、原生构建及 WASM Clippy/构建。测试/编译不能代替实际 GUI/USER 证据。

## 尚待推进

其余开放 CAP 实施票仍按依赖推进；EDS-01 至 EDS-10 已作为研究/设计交付关闭，EDS-11 原型与 EDS-12 作者验证保持开放。EDS-12 必须有真实参与者记录，不补造成功率、时间或满意度。未回答的参与者安排不构成已有批准。

用户原有未跟踪文件 `docs/planning/20260926/issues.json` 保持不动，未混入本轮提交。
