# 2026-09-26 ticket 实施记录

本轮请求是完成两仓所有 ticket；截至本记录尚未全部完成。初始开放 43 张，CAP-09A 已完成范围决定，CAP-09C 已按用户要求删除，仍有 41 张开放票。实施分支均为 `codex/complete-tickets-20260926`，未合并或发布。

## 已实现、待完整验收

### worldline#13 / CAP-01A

- core 组合意图包含新建/复用对象、正文稳定引用和可选地图入口，候选 Project 全部检查后一次提交；返回变更文件、引用影响与新内容基线。
- 预览零写入，拒绝陈旧选区/基线、外部修改、未知能力、非法几何/锁层，保留原文未选部分和未知 JSON 字段。
- 8 项新增公开 API 回归覆盖 Unicode、同名不同身份、别名、注释误选、跨文件保存后撤销/重做及失败零部分更新。
- 首期仅进程内 API；CLI/RPC 组合入口按 ticket 允许的分期方式在协议明示。CAP-01B / CAP-01C 尚未接入。

### worldedit#11 / CAP-02C

- 最多两个独立钉住面板，每个最多 64 条历史；宽屏并列窗口，1040×660 以切换面板退化，个人阅读状态不写工程。
- 同名按 kind+ID 区分；删除对象显示失效身份；工程切换清空面板并保护未提交表单；Web 离页提示覆盖纯表单草稿。
- 新增状态测试和 egui 点击回归，覆盖钉住、窄屏切换/关闭、源码入口、时段/实体草稿、失效对象与独立临时阅读窗口。
- 原生/Web 实际交互、键盘焦点与缩放恢复尚未验收，不能将 headless 测试写成实际作者验证。

## 研究交付

- worldedit#32 / EDS-01：[官方交互观察与证据边界](design/editor-system/20260926/eds01-evidence.md)。四主样本各四条 DOC 观察卡及失败/恢复路径，五补充样本、12 模式反例和替代方案。GUI/RUN/USER 缺口明确保留，ticket 未关闭。
- worldedit#33 / EDS-02：[任务、对象与入口草案](design/editor-system/20260926/eds02-tasks-and-objects.md)。四条任务旅程、对象生命周期、五类结构和旧 12 页面映射；频率与角色标为假设，不冒充用户访谈。后台研究连接中断后由主任务依据已核查代码补齐，仍待作者验证。
- worldedit#34 / EDS-03：[编辑器状态、身份与事件路由契约](design/editor-system/20260926/eds03-state-contract.md)。定义六种“当前”、类型化身份、状态所属域、事务转换及 owner/IME/冲突负例；这是设计契约和验收清单，不宣称生产 UI 已全面实现。
- worldedit#35 / EDS-04：[六区域显隐与 A/B/C 比较](design/editor-system/20260926/eds04-layout-comparison.md)。三种结构用同一组任务与示例数据比较，B 暂列下一轮验证首选，完整自由 dock 暂不采用；抛弃式交互原型单独保存在 `codex/eds04-prototype` 的 `ae23509`，尚待实际作者评估。
- worldedit#36 / EDS-05：[跨投影交互契约](design/editor-system/20260926/eds05-cross-projection.md)。定义类型化导航与返回点、临时/固定资料库、拖放零写入、长标签文字后备与 core 实际 250 节点/500 边上限；仍需真实事件和作者可达性验收。
- worldedit#37 / EDS-06：[检查器、参数与正文草稿契约](design/editor-system/20260926/eds06-inspector-drafts.md)。列出跟随/固定及多选矩阵、默认/缺失/未知值、渐进分组、重置范围、三方比较与 IME/陈旧负例；这是设计，未声称生产界面完成。
- worldedit#38 / EDS-07：[时间、控制流、书稿与关系图视觉语法](design/editor-system/20260926/eds07-graph-grammar.md)。四种投影分开命名与图例，区分选择、预览和运行位置；为并行边、自环、长标签与截断定义文字后备和负例。未做作者理解测试。

## 已决定的排除

- 用户明确“不考虑游戏引擎适配器”，并要求直接删除对应 ticket。worldline#27 已删除（GitHub 返回 HTTP 410）；#24 的不采用决定已完成。
- worldedit#9 总览和 #28 已移除适配器实施范围与依赖；#28 改为“台词审阅与本地化工作台”，只依赖 worldline#26。本地化没有因排除引擎而被取消。
- 决策记录在配对 core 的 `docs/adr/0001-no-engine-adapter.md`。

## 审查与验证口径

用户确认以 core `889febe99f7999f8da4c28583320da61e6be13e2`、editor `9bef07c4b0e10196787ee0447b15be4261847903` 为审查基准，测试入口为公开 Project/core、CLI/RPC 与真实 egui 事件。

- Standards：两轮均无发现。
- Spec：core 引用影响缺项已修；editor 的时段草稿、Web 离页提示和跨阅读窗口关闭问题已修，并补回归。实际双端与作者测试缺口独立记录。
- 最终被检代码配对：core `0ba3e241af3a5f5746cfed92e2b5f8ce72909843`，editor `aa3940342d46dd29b5a9995d6da48dfc54e67f74`。完整 SHA、环境与每个命令日志在本地 `target/paired-check/20260926T1009336668816Z/`；这些构建/本机日志不提交源码。
- 检查脚本：`scripts/check-pair.ps1` 全部通过，分别指定两仓 Cargo.toml，覆盖格式、完整测试（core/CLI/runtime/agent 合计 333，editor 合计 110）、Clippy、原生构建及 WASM Clippy/构建。测试/编译不能代替实际 GUI/USER 证据。

## 尚待推进

其余开放 CAP 实施票仍按原依赖推进；EDS 研究、设计、原型与作者验证票保持开放。EDS-12 必须有真实参与者记录，不补造成功率、时间或满意度。未回答的参与者安排不构成已有批准。

用户原有未跟踪文件 `docs/planning/20260926/issues.json` 保持不动，未混入本轮提交。
