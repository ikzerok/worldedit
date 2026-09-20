# CLI 与编辑器功能覆盖

结论：CLI **不能完全控制 worldedit 的全部功能**。当前 worldedit 命令行只接受工作区目录（兼容根入口文件），没有 IPC、远程控制端口或编辑器命令队列。wl 与 wl-agent 服务语言及演练层，不能访问运行中编辑器的未保存缓冲。

| 功能 | wl | wl-agent | 编辑器 / AI 可行方式 |
|---|---|---|---|
| 编译、诊断、统计 | check | compile | 同一 core 分析 |
| 执行图与条件上下文 | graph | analyze / export | 共享结构数据；Mermaid 是文本输出 |
| 时段与先后关系 | timeline | analyze | 同一 timeline 数据 |
| 人物、标签、状态、锚点、素材、别名、正文链接 | catalog | analyze.catalog | 查找与源码定位 |
| 1.10 实体目录与属性 | catalog --kind entity | project.open / project.analyze | 同一 core 目录；同名实体和旧词条保持独立 |
| 创建、修改、删除 1.10 实体 | entity create / update / delete | entity.create / entity.update / entity.delete | 显式1.10工程；按基线与引用保护写入磁盘，桌面随后刷新 |
| 试玩、选择、当前状态 | play | session.* | 独立会话，不连接 UI 当前试玩 |
| 演练存读档 | play --load/--save | session.open/save | 各自会话存档 |
| 创建/修改人物、事件和设定 | 无写入命令 | 无写入方法 | 编辑 `.wl` 或 Rust Project API；桌面刷新 |
| 新建/选择工作区 | 启动 worldedit DIR | 无 | 编辑器目录选择 |
| 递归源码搜索 | 无专门命令 | 无 | 编辑器搜索或读取工作区文本 |
| 保存缓冲、另存、完整工程导出 | 无 | 无 | 编辑器；Rust Project API 可供自建工具调用 |
| 切换视图、选中对象、资料阅读窗口 | 无 | 无 | UI |
| 卡片拖动、缩放、关系连线手势 | 无 | 无 | UI；语义修改可写源码 |
| 撤销、重做、未保存对话框 | 无 | 无 | UI，仅当前应用历史 |
| 最大化、关闭、窗口拖动 | 无 | 无 | UI / 系统窗口管理 |
| 浏览器目录授权、下载 | 无 | 无 | 浏览器 UI |

`wl-agent` 的完整方法表以 worldline/spec/agent-protocol.md 为准。`export` 的两个格式是 graph_mermaid、timeline_mermaid，不能导出工作区文件。当前没有“所有编辑器功能均可由 CLI 控制”的保证。

AI 创作的可用流程是：读取作品 → CLI 检查与反查 → 改写工作区文件 → CLI 复查 / 演练 → 桌面自动刷新 → 必要时真实 UI 验证 / 导出。浏览器目录是快照，需要重新导入外部变化。运行中编辑器有未保存修改时，应先合并，避免 AI 的磁盘稿与缓冲冲突。

## 验证依据

静态核对 worldedit/src/main.rs 的参数入口、app.rs 的 UI 操作、worldline/cli/src/lib.rs 的子命令分发和 worldline/agent/src/lib.rs 的 RPC 方法分发；CLI 与协议测试覆盖分析及会话。实体接口以配对 worldline/spec/agent-protocol.md 的参数和验收为准。此表不把“共享 Rust API”计作现成 CLI 能力。
