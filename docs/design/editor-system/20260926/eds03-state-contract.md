# EDS-03：编辑器状态、身份与事件路由契约

关联 worldedit#34。本文是供原型与实现审查的设计契约，不宣称现有 UI 已全部采用。依据[系统指南 §4](guide.md)、[EDS-01 证据](eds01-evidence.md)、[EDS-02 任务与对象](eds02-tasks-and-objects.md)；用户已排除游戏引擎适配器。

## 身份与六种“当前”

| 状态 | 唯一含义 | owner 与寿命 | 项目写入 |
|---|---|---|---|
| `hover_target` | 指针当前经过的命中对象 | 投影/控件，离开即清 | 无 |
| `selection` | 作者明确选中的对象集合 | 投影内；切投影可按稳定身份恢复 | 无 |
| `active_target` | 多选中的主要操作对象 | 与 `selection` 同域，必须属于集合 | 无 |
| `keyboard_focus` | 接收键盘及 IME 的控件 | egui 焦点；与所选对象无必然关系 | 无，除非控件明确提交草稿 |
| `inspected_target` | 检查器显示的对象 | 面板；`Follow(selection)` 或 `Pinned(id)` | 浏览无；显式编辑才可能写 |
| `preview_cursor` / `runtime_position` | 临时观察位置 / 运行会话实际位置 | 前者投影，后者 runtime 会话 | 均不改作者内容 |

身份在所属空间内类型化：`TargetRef(kind,id)` 是 core 内容身份；`MapPlacementRef(map_id,placement_id)` 是一张地图中的展示实例；`GraphEdgeRef` 需携带图/边的来源空间；`SourceRange(file,baseline,range)` 是有基线的临时文本位置；未来书稿条目只有 CAP-02A 明确的身份才能进入 UI。上述身份不由显示名、数组序号或屏幕坐标推断。跳转携带身份、投影、原焦点与滚动/镜头返回点；若目标失效，保留原身份并显示失效，不自动选择同名对象。源位置因编辑而变化时，应重新定位或提示，不能把旧行号当永久 ID。

`selection` 可以指地图标记，同时 `inspected_target` 固定在资料 B；`runtime_position` 可以是另一事件。这不是矛盾。页面标题、选中高亮、焦点环、检查器标题和演练指示须使用不同标记，并分别说出 owner。跨投影复用 `TargetRef` 时仍保留投影来源；地图入口与其关联资料是两个对象，删除命令也不同。

## 状态所属域与保存边界

| 域 | 本票状态或例子 | 写入与撤销 |
|---|---|---|
| 临时交互 | hover、指针拖动预览、菜单、`preview_cursor` | 不写 `Project`；取消丢弃候选 |
| 个人会话 | selection、active、焦点、跟随/固定检查器、阅读面板、返回栈、镜头 | 切工程清理或按工程隔离；不进入内容撤销 |
| 个人持久 | 面板布局、快捷键、文字缩放 | 本机偏好独立存储；失败退默认且不能报“作品已保存” |
| 编辑草稿 | 属性表单、正文未应用输入、连线意图 | 绑定工程、owner、原值与基线；取消不写；陈旧需重新比较 |
| 作者共享 | 源文、目录声明、关系、地图展示、显式共享布局 | 通过 core/Project 验证、应用、保存；参加内容撤销和冲突处理 |
| 运行/调试 | `runtime_position`、会话状态与重放结果 | runtime 拥有，标示使用的编译快照；浏览不推进 |

“应用到缓冲”与“保存到磁盘”分别报告。Web 下载或导出开始也不能冒充作品落盘。个人布局重置只碰个人域；检查器钉住、选中或查看诊断均不改变 `Project` 内容指纹。core 是结构、身份、校验和合并的唯一解释者；UI 不重做 parser、权限或冲突决策。

## 编辑事务状态表

一个事务绑定 `(project_identity, owner_id, source_baseline, persistence_domain)`。状态属于该事务，不是全局工作台模式。UI 即使切换投影，也不能把 A 的事务悄悄改绑到 B。

| 当前状态 | 事件与条件 | 下一状态 | 必须可见的结果 |
|---|---|---|---|
| `Idle` | 显式开始编辑，owner 可定位 | `Previewing` | 显示 owner、目标域及候选值；Project 不变 |
| `Previewing` | 指针移动或输入、IME 组合中 | `Previewing` | 仅更新候选；不得触发一次提交或全局快捷键 |
| `Previewing` | 明确提交，IME 已完成 | `Validating` | 锁定本次候选与原基线，说明正在验证 |
| `Validating` | core 接受且目标基线匹配 | `AppliedToBuffer` | 一次原子应用、生成相应内容撤销记录、显示未保存 |
| `AppliedToBuffer` | `Project` 保存成功 | `Saved` | 报告实际持久化位置及新基线 |
| `Saved` | 新编辑 | `Previewing` | 新事务捕获当前基线 |
| `Previewing` / `Validating` | Escape/取消且无已应用写入 | `Cancelled` | 恢复原显示；草稿按明确“保留/放弃”选择处理 |
| `Previewing` / `Validating` | owner 删除、身份或来源基线变化 | `RejectedStale` | 保留输入，显示旧身份与重新定位/比较入口；不得强制覆盖 |
| `Idle` / `Previewing` / `Validating` | 只读、锁层或未知能力 | `ReadOnly` | 不写入；解释原因，仍允许检查 |
| `Validating` / `AppliedToBuffer` | 外部版本冲突或保存冲突 | `Conflict` | 保留本地候选/缓冲与双方基线，走 core 冲突流程；不默认选赢家 |
| `RejectedStale` / `Conflict` | 用户明确重新比较并通过验证 | `Previewing` | 新事务重新捕获基线；旧输入可复制，不暗中自动重放 |

验证错误（例如无效 ID/坐标）停留在可修复的 `Previewing`，携带字段诊断；不能丢输入后只弹 toast。`pointer_up` 至多发起一次验证与提交，`pointer_move` 只能预览。`AppliedToBuffer` 后取消若要回退应走内容撤销，不能伪称从未写入。保存失败停留在未保存/冲突状态，不能显示 `Saved`。

## 事件路由与切换保护

输入事件按以下顺序给一个 owner：模态操作 → IME/文本控件 → 有焦点编辑区 → 全局安全命令。命令描述包含 `id / label / owner / allowed_contexts / arguments / enabled_reason / preview / commit / undo / persistence_domain`；按钮、菜单和快捷键调用同一命令。Delete 在文本输入中只删字；IME 合成阶段不触发单键建节点或删除对象；Escape 先取消最内层预览，再处理表单或面板，不能绕过未提交输入。运行快捷键只有显式运行焦点/命令可推进 `runtime_position`。

检查器 `Pinned(B)` 后选择 A：检查器标题持续显示“已固定：B”及 B 的 kind/ID；编辑 B 的命令显示 B，提交摘要再次确认 B；选择 A 只改变 selection/active，不能改写 B 草稿 owner。若产品要编辑 A，必须显式解除固定或打开 A 的编辑入口。固定 B 删除/重命名/外部刷新时按身份刷新；失效则显示 B 已失效及定位/关闭，不自动跳到 A 或同名对象。

目标或工程切换前，先检查所有有输入的事务。作者明确选择应用、保留草稿、放弃或取消；若应用基线失效仍进入 `RejectedStale`/`Conflict`，确认框不能绕过。保留草稿须按工程和 owner 隔离；重新打开原工程也要重新核对基线。只读与未知能力可导航，但不能把禁用的提交伪装成功。专注模式、窄屏折叠和视图切换只改变个人会话/布局，不销毁表单。

## 现有实现审计与迁移接缝

当前 [app.rs](../../../../src/app.rs) 分别保存 `reading_target`、`catalog_target`、`network_selected` 和 `map_selection`；[地图实现](../../../../src/app/maps.rs) 另有画布选择和未提交操作保护；[表单守卫](../../../../src/app/authoring_forms.rs) 捕获工程内容基线/版本。`request_action` 和 `has_open_authoring_form` 为现有切换保护，`reset_views` 会清个人投影状态。这些是分散 owner 的事实，不意味着一个全局 `selected` 可以代替所有状态。本票只给迁移接缝：先标注每个字段的对象域、身份、寿命与写入权，再让新投影通过类型化导航意图互通；具体 UI 改造另行实施和验收。

## 验收路径与反例

| 路径 | 检查点 |
|---|---|
| 成功 | 浏览选择 A、固定 B、修改 B；标题和提交摘要均为 B，只有 B 被写；应用缓冲后另行保存 |
| 取消 | 地图入口拖动多次 `pointer_move` 后 Escape；几何、内容指纹与撤销栈均不变 |
| 草稿切换 | A 输入中切 B 或另一工程；明确保留/应用/放弃/取消，返回时 owner 和基线可核对 |
| 失效与冲突 | B 重命名仍以 ID 找到；删除/改 ID/外部刷新使旧目标失效，显示原身份；旧基线提交被拒并保留输入 |
| 输入法与键盘 | 中文组合输入中 Delete/Escape/单键快捷键由文本控件处理；焦点离开后再执行图对象命令 |
| 浏览零写入 | 搜索、选中、固定旁查、切投影、预览事件与改变镜头前后，`Project` 内容指纹相同，内容撤销栈不增加 |
| 运行分离 | 预览未走分支或查看时间偏序，`runtime_position` 不变；仅显式运行命令推进 |

这是验收清单，尚无真实双端操作或作者研究结果。CAP-02C 的阅读面板已有部分状态与 headless 测试；本票不把这些测试当作上述全部路径已通过。原型和后续 CAP-04B 应按真实 egui 事件及公开 Project/core 边界补证据。
