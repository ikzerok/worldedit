# CLI 与编辑器功能覆盖

CLI **不能遥控运行中 worldedit 的全部功能**。worldedit 启动命令接受工作区目录（兼容根入口文件），没有 IPC、远程控制端口或编辑器命令队列。当前 `wl` 与 `wl-agent` 已提供语言分析、资料编辑、独立演练、矢量场景事务和静态站点发布；它们读取磁盘作品或自身 Project 会话，不能访问编辑器窗口内的未应用稿或未保存缓冲。

| 功能 | wl | wl-agent | 编辑器 / AI 可行方式 |
|---|---|---|---|
| 编译、诊断、统计 | check | compile | 同一 core 分析 |
| 正式对白 typed 计划 | dialogue query/preview/apply；apply 显式 --save | dialogue.query / dialogue.edit.preview / dialogue.edit.apply | 同 core 单语句、明确迁移与完整基线；只修改自己的 Project，不读取 GUI 草稿 |
| 同稿角色/locale 制作台本 | production-script query/export；--output 只写新文件 | production.script.query / production.script.export | 静态定义范围、精确分页与三格式字节；默认排除备注，不自动发送第三方 |
| 当前工程问题、范围筛选、主/关联来源 | problems | project.problems | 同一 core 只读报告；CLI/RPC使用自己的工程/会话，不能读取运行中编辑器的未应用草稿；不改既有运行/发布门禁 |
| 执行图与条件上下文 | graph | analyze / export | 共享结构数据；Mermaid 是文本输出 |
| 时段与先后关系 | timeline | analyze | 同一 timeline 数据 |
| 人物、标签、状态、锚点、素材、别名、正文链接 | catalog | analyze.catalog | 查找与源码定位 |
| 对象完整身份筛选与分页 | object-search --query；可加 --filter-json / --options-json | world.objects.search | 同一 core 类型/实体类型、别名、按需源码路径和分页预算；自己的已应用快照，不读编辑器未应用稿 |
| 新章及正式来源原子创建 | manuscript-chapter preview / apply --request-json；apply需 --plan-digest | manuscript.chapter.preview / manuscript.chapter.apply | 新/已有书稿，复用来源或新建空事件；同一 core 计划和保存基线保护，不自动连接执行路线 |
| 工程模板结构草稿和完整影响事务 | template draft / preview / apply --request-json；apply需 --plan-digest | template.draft / template.preview / template.apply | 同一 core 字段结构、完整性、两侧影响及原子计划；明确apply后仍须save，不读取UI未提交字段输入 |
| 完整范围书稿查询 | manuscript-query --query-json；可加 --drafts-json | manuscript.query | 自身快照上的筛选/分页；明确提供草稿DTO，不读取另一个编辑器实例 |
| 实际运行状态与真实观测对照 | play等待选择时输入 inspect 或 inspect JSON | session.inspect | 查询各自正在运行的Story，零推进；不读取或遥控UI试玩 |
| 1.10 实体目录与属性 | catalog --kind entity | project.open / project.analyze | 同一 core 目录；同名实体和旧词条保持独立 |
| 创建、修改、删除 1.10 实体 | entity create / update / delete | entity.create / entity.update / entity.delete | 显式1.10工程；按基线与引用保护写入磁盘，桌面随后刷新 |
| 工作区检查、地图与标记反查 | workspace check / maps list | workspace.check / maps.list | 核心分域诊断及地图索引；读取已保存作品，不连接编辑器缓冲 |
| SVG 安全预检 | scene svg-preview --source SVG文本 | scene.svg.preview | 返回同一 core typed 预检；没有工程副作用，不代表完成 UI 导入 |
| 原生矢量批次 | scene preview / apply --request-json SceneBatch | scene.preview / scene.apply | core 预览、基线与摘要核验；apply 保存自身 Project，不控制画布手势 |
| 全图矢量 SVG | scene export --map-id ID | scene.export | 输出 legacy + scene 安全 SVG，遵守文档默认显隐；不包含底图与完整作者元数据 |
| 静态世界站 v1/v2/v3 | reader-export preview / apply --selection-json DTO | reader.export.preview / reader.export.apply | 同一 core 白名单和计划；原生 apply 写工作区外的新目录，编辑器向导另负责 ZIP 交付 |
| 发布 profile 保存、升级与复用 | 无专用命令 | 无专用方法 | 编辑器发布向导或 Rust Project API；配置应用不自动发布 |
| 新建地点并绑定的组合事务 | 无专用命令 | 无专用方法 | 编辑器与 core 组合 API；分两次实体/地图命令不等同一个原子事务 |
| 独立关系查询与分页 | relations --target KIND:ID；续页 --offset | relation.query（offset） | 同一 core 邻接查询，保留修订和筛选；事件控制流仍使用 graph |
| 关系类型及实例编辑 | relation-type / relation create、update、delete | relation.type.* / relation.create、update、delete | 1.10 Project 事务、基线与引用保护 |
| 旧人物关系显式提升 | relations promote preview / commit | relation.promote.preview / commit | 预览新关系及旧项移除、保存兼容影响；提交时校验基线和预览一致性 |
| 正式人物/实体与稳定引用组合 | authoring-intent preview/apply --intent-json | authoring.intent.preview/apply | create_character 写正式 character；旧 apply 成功立即保存。该接口使用自身 Project，不能读取编辑器 WritingBuffer，也不等同 UI 的显式 1.10 迁移复合计划 |
| 稳定本地化目录与内存候选 | localization catalog；localization ids/edit/import-candidate preview/apply（--save 显式保存） | localization.catalog；localization.ids/edit/import_candidate.* | core 唯一目录和 typed 计划；RPC 使用自身 project_id 内存，不控制 UI；旧 import apply 仍立即保存 |
| 真实 locale 体验与身份恢复 | play --locale（可显式 --locale-fallback source）；replay/route-compare/playthrough-report | session.open localization + runtime.localization.v1；trace.replay | 同次源顺序求值，独立展示身份；从当前 Project 准备，旧 source-only 默认不变 |
| 试玩、选择、当前状态 | play | session.* | 独立会话，不连接 UI 当前试玩 |
| 当前稿真实双路线对照 | route-compare | project.compare_routes | 共用有界runtime DTO；只读各自工程，不遥控UI；对照source/back与取消仍由编辑器运行态处理 |
| 演练存读档 | play --load/--save | session.open/save | 各自会话存档 |
| 通用人物、事件和设定表单 | 无同等通用表单命令 | 无同等通用表单方法 | 编辑 `.wl` 或 Rust Project API；新章命令仅有明确的新建空事件选项，不等同全部事件编辑 |
| 新建/选择工作区 | 启动 worldedit DIR | 无 | 编辑器目录选择 |
| 递归源码搜索 | 无专门命令 | 无 | 编辑器搜索或读取工作区文本 |
| 保存缓冲、另存、完整工程导出 | 个别写命令显式 --save | project.save（自身会话） | 不读取运行中编辑器缓冲；编辑器另存与完整工程导出仍由 UI 负责 |
| 当前单文件源码结构与精确定位 | 无专用命令 | 无专用方法 | 编辑器本文件结构；Rust Project::source_outline / source_outline_range只读接口，不访问另一个进程的源码缓冲 |
| 切换视图、选中对象、资料阅读窗口 | 无 | 无 | UI |
| 卡片拖动、缩放、关系连线手势 | 无 | 无 | UI；语义修改可写源码 |
| 撤销、重做、未保存对话框 | 无 | 无 | UI，仅当前应用历史 |
| 最大化、关闭、窗口拖动 | 无 | 无 | UI / 系统窗口管理 |
| 浏览器目录授权、下载 | 无 | 无 | 浏览器 UI |

`wl-agent` 的完整方法表以 [机器协议](../../worldline/spec/agent-protocol.md)和[场景协议](../../worldline/spec/scene-protocol.md)为准。旧 `export` 方法仍只有 graph_mermaid、timeline_mermaid，不要与新增 `scene.export` 或独立 `reader.export.apply` 混同。后两者也不等于完整工程备份。

AI 创作的可用流程是：读取作品 → CLI 检查与反查 → 改写工作区文件 → CLI 复查 / 演练 → 桌面自动刷新 → 必要时真实 UI 验证 / 导出。浏览器目录是快照，需要重新导入外部变化。运行中编辑器有未保存修改时，应先合并，避免 AI 的磁盘稿与缓冲冲突。

## 0.15 预览与提交参数

场景预览使用 `wl scene preview 工程 --request-json SceneBatch --json`；应用重传同一 batch，并带 `--baseline` 与 `--plan-digest`。SVG 预检的文本选项是 `--source`，导出地图选项是 `--map-id`。这些入口当前没有 `--request-file` 或 `--source-file` 选项；完整 DTO 见[场景协议](../../worldline/spec/scene-protocol.md)。

RPC 的 `scene.preview/apply/export` 必须且只能提供 `path` 或 `project_id`；`scene.svg.preview` 只接收 `source`。有状态会话成功保存后推进 scene 修订，一次性 path 调用从默认修订开始，均不能跳过内容基线、文档 hash 和计划重算。初始化能力名为 `authoring.vector_scene.v1`。

读者站预览使用 `wl reader-export preview 工程 --selection-json DTO --json`；应用增加 `--plan-digest` 和 `--out 新目录`。RPC 为 `reader.export.preview/apply`，应用另需 `plan_digest` 与 `output`。选择 v3 必须显式包含 `reader.world_site.v1`；属性与静态条件说明各有额外授权。已有输出不覆盖，改变公开选择或原稿后重新预览。参数和公开边界见[世界站规范](../../worldline/spec/reader-site.md)。

核心计划一致不代表 UI 体验等价：独立命令没有画布选择、拖动、检查器输入、窗口取消、临时浏览器预览或编辑器撤销栈。运行中编辑器存在未保存稿时仍须按工作区冲突规则合并。

## 验证依据

静态核对 worldedit/src/main.rs 的参数入口、app.rs 的 UI 操作、worldline/cli/src/lib.rs 的子命令分发和 worldline/agent/src/lib.rs 的 RPC 方法分发；CLI 与协议测试覆盖分析及会话。实体与 reader 接口以配对 worldline/spec/agent-protocol.md 为准；scene 参数已对照 cli/src/lib/scene.rs、cli/src/lib.rs 与 agent/src/lib.rs 注册项和 spec/scene-protocol.md。此表是能力入口核对，不表示当前候选已完成实际交互验收，也不把“共享 Rust API”计作现成 CLI 能力。

## 0.9 约束与当前稿边界

持续schema诊断随check/workspace check/发布前检查生效；schema-index、schema-preview、schema-apply及RPC同名语义可独立使用。锁定choice通过CLI `--choice-presentation`与RPC session capability协商，旧choices索引不变。

编辑器尚未应用的WritingBuffer属于当前桌面会话；独立CLI不能读取其窗口内草稿。Find/Replace、焦点、专注布局与当前稿预览由编辑器调用同一core API，不宣称外部CLI遥控全部UI。本地化只交换显式白名单；runtime 默认源文，0.33 起仅在明确选择 locale 后使用验证过的译文快照。

## 0.18 工程问题与源码视图

`wl problems 工程 --json` 返回带报告身份、覆盖状态的有界问题页；严重性、域、路径、文本筛选由 core 完成，相关来源使用报告返回的 opaque ID。工程改变后的旧 ID／游标明确失效。RPC `project.problems` 支持自己的 Project 会话缓存与显式刷新，能力名为 `authoring.problems.v1`。详细参数和门禁边界见 [core问题指南](../../worldline/docs/problems.md)。

编辑器提供同一报告的问题列表、完整证据详情、F8/Shift+F8（原生）、来源返回与源码自动换行。换行仅改变排版、保留源字节和物理行号；这些视图操作没有独立CLI遥控入口。

### 0.28 通用对象使用处

通用对象阅读页的“使用处与相关上下文”消费同快照 core world-context，主动启用
静态规则/片段调用及全局读写，支持一/二跳与方向。结果披露未知总量、索引/候选/
显示预算和错误来源；未应用草稿不计入。来源定位重验当前稿，再复用作者返回历史。
CLI 的 `world-context --options-json '{"include_executable":true}'` 与 RPC
`world.context.options` 为同一投影；旧请求默认仍仅返回旧六类。静态写入不等于
试玩已实际执行的变量写入，不改变重命名/删除引用计数。

## 0.28 已验证试玩报告

编辑器“试玩路径报告…”、`wl playthrough-report` 与 RPC `project.playthrough_report`
共用 runtime 的真实重放验证及 Markdown 生产者。UI额外提供当前/已录制路径选择、预览、
未应用输入与作者私密范围确认、取消、复制、桌面新文件保存/Web下载、当前稿与外部观察守卫。
CLI/RPC维持同步有界查询，不替代UI确认，也不改变live session或保存基线。完整路径见
[试玩路径报告](playthrough-report.md) 与 worldline/spec/playthrough-report.md。

## 0.30 起笔与统一对象检索

`wl manuscript-chapter preview 工程 --request-json DTO --json` 返回实际原子计划；apply 重传同一请求并带 `--plan-digest`，默认只在短命 CLI 内存应用，必须显式 `--save` 才保存。预览不能带 save/digest，参数错误退出 2、业务拒绝退出 1；保存失败会明确 `applied:true` / `saved:false`，不能据此宣称磁盘未触及。RPC 的 preview/apply 必须使用自身已打开的 project_id，不接受 path；末端仍重建候选并核对修订、基线、全部文件清单与内容摘要。

`wl object-search 工程 --query 文本 --json` 支持 `--filter-json`、`--options-json` 与可选 `--expected-baseline`。空查询须明确传入空字符串；只读打开工程，不恢复或保存事务。RPC 方法是 `world.objects.search`，filter/options 与 core 使用相同 DTO；预算先针对完整 Catalog，不允许 UI 通过裁剪目录绕过限制。

完整新章请求/结果见[新章作者契约](../../worldline/spec/manuscript-authoring.md)，分页字段见[对象检索](../../worldline/spec/object-search.md)。正文空槽属于 core/UI 投影，机器后续编辑仍走已有 `source.edit`，没有新增远程键入或外观遥控协议。

## 0.31 模板、书稿与状态查询

模板draft/preview均不写工程；apply重算同一请求并核对plan_digest，CLI只在显式`--save`时落盘，RPC调用`project.save`保存自身会话。机器草稿限4MiB、计划限8MiB，适用实例/字段值在物化前限额，超限明确失败。高级模板JSON的未知可选扩展仍保留；机器DTO未知字段和重复键严格拒绝。具体参数见[机器协议](https://github.com/ikzerok/worldline/blob/main/spec/agent-protocol.md)。

书稿query在完整范围先筛选再分页，limit必须1至100，陈旧cursor不能重用。已生成不可变快照的查询零IO；生成仍复用core路径身份/注册/附件只读检查，不做缺失源码回退或提交预检。编辑器另外持有自己的未应用正文、编排和个人位置。

session.inspect与CLI暂停输入inspect只查询当前真实Story；首次/前次只能来自已记录观测，状态声明定位不表示最后写入的因果。CLI/RPC不提供检查窗口、选择焦点、设备外观或编辑器撤销的远程操作。

## 0.32 同一稿的四条作者路径

| 编辑器操作 | CLI | RPC |
|---|---|---|
| 普通外改三方核对、预览、内存采纳 | `wl reconciliation capture/preview/apply/save`，显式真实基线/本地材料 | `reconciliation.capture/preview/apply`，自身Project后续明确save |
| 当前正文草稿隔离试演 | `wl draft-rehearsal --request 文件.json`或`--request-json DTO` | `project.draft_rehearsal` |
| 当前书稿筛选的作者审稿本 | `wl manuscript-delivery --request-json DTO`，显式`--output`新文件 | `manuscript.delivery`，只返回材料 |
| 查询范围中的对象、地图与正式关系 | `wl catalog-scope --query DTO` | `catalog.scope` |

这些接口使用同一core/runtime，不遥控界面输入或读取另一个进程的WritingBuffer。
外改预览不采纳、采纳不等于保存；隔离试演不生成正式轨迹；全分支审稿不执行选择；
临时查询范围不生成世界事实或新集合文件。参数、字节预算和真实失败状态见
[0.32协议](https://github.com/ikzerok/worldline/blob/main/spec/agent-protocol.md)。
