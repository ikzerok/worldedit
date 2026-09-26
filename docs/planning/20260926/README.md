# 能力补齐与编辑器视觉优化：工单计划（2026-09-26）

两项独立需求分别跟踪：CAP 是同类产品能力差距；UX 是编辑器视觉与交互优化。
计划为 2 张总览 + 35 张子工单（CAP 29 / UX 6），按源码归属拆为 worldline 15 张、worldedit 22 张（含总览）。

## 状态与边界

- 所有新增票保持 OPEN、待评审；优先级是建议，没有负责人和截止日期。
- UX-01 有本地实现和验证，但没有合入 main 或覆盖 v0.3.0；其余不是已实现能力。
- P2 与 DOCX/EPUB、历法和引擎相关条目保留范围决策门，不把建票当作批准实现。
- 原有 WP 已关闭票不重复开启；新增能力不能套用旧 65 项 PASS。
- GitHub 地址与原生依赖发布后写入 issues.json；tickets.json 是本轮完整中文任务定义。

## 工单索引

| 编号 | 仓库 | 优先级 | 标题 | 依赖 |
|---|---|---|---|---|
| CAP-01A | ikzerok/worldline | P0 | 就地建档与稳定引用的组合事务 | 无新增票 |
| CAP-01B | ikzerok/worldedit | P0 | 正文内 @ 引用、选文建档与返回原编辑位置 | CAP-01A |
| CAP-01C | ikzerok/worldedit | P0 | 地图上新建地点资料并放置入口 | CAP-01A |
| CAP-02A | ikzerok/worldline | P0 | 书稿编排文档、章节引用与统计契约 | 无新增票 |
| CAP-02B | ikzerok/worldedit | P0 | 书稿与章节工作台：组织、状态、目标和阅读预览 | CAP-02A |
| CAP-02C | ikzerok/worldedit | P0 | 可钉住的多资料阅读面板与创作旁查 | 无新增票 |
| CAP-03A | ikzerok/worldline | P0 | 资料库组合查询、保存查询与待办投影 | 无新增票 |
| CAP-03B | ikzerok/worldedit | P0 | 资料库筛选器、常用集合与统一待办入口 | CAP-03A |
| CAP-04A | ikzerok/worldline | P0 | 提案逐字段/逐段落三方差异与引用影响 DTO | 无新增票 |
| CAP-04B | ikzerok/worldedit | P0 | 三栏审稿、差异定位与明确冲突处理 | CAP-04A |
| CAP-04C | ikzerok/worldline | P1 | 有容量限制的本地检查点与受保护恢复 | 无新增票 |
| CAP-04D | ikzerok/worldedit | P1 | 检查点历史面板、对比与恢复确认 | CAP-04C, CAP-04A |
| CAP-05A | ikzerok/worldline | P1 | 工程级自定义模板、类型校验与无损升级 | 无新增票 |
| CAP-05B | ikzerok/worldedit | P1 | 模板管理器与按字段类型渲染的创作表单 | CAP-05A |
| CAP-06A | ikzerok/worldline | P1 | 确定性叙事重放、检查点与条件解释协议 | 无新增票 |
| CAP-06B | ikzerok/worldedit | P1 | 叙事调试器：路径重放、状态对比与失败条件定位 | CAP-06A |
| CAP-07A | ikzerok/worldline | P1 | Markdown 资料迁移：预检、映射、稳定引用与原子应用 | 无新增票 |
| CAP-07B | ikzerok/worldedit | P1 | Markdown 导入向导与冲突/损失预览 | CAP-07A |
| CAP-07C | ikzerok/worldline | P1 | 独立静态阅读包与公开内容边界 | CAP-02A |
| CAP-07D | ikzerok/worldedit | P1 | 读者发布向导、隐私预览与静态包交付 | CAP-07C |
| CAP-07E | ikzerok/worldline | P2 | DOCX/EPUB 导入导出选型与保真原型评估 | 无新增票 |
| CAP-08A | ikzerok/worldline | P2 | 家族/组织与人物弧线的显式关系投影 | 无新增票 |
| CAP-08B | ikzerok/worldedit | P2 | 家族树、组织图与人物/地点历史专题视图 | CAP-08A |
| CAP-08C | ikzerok/worldline | P2 | 自定义历法与不确定日期的范围决策 ADR | 无新增票 |
| CAP-09A | ikzerok/worldline | P2 | 游戏生产方向与首个引擎接入的范围决策 | 无新增票 |
| CAP-09B | ikzerok/worldline | P2 | 稳定本地化字符串 ID 与翻译往返校验 | CAP-09A |
| CAP-09C | ikzerok/worldline | P2 | 首个引擎导出适配器与协议一致性校验 | CAP-09A |
| CAP-09D | ikzerok/worldedit | P2 | 游戏项目可选入口：台词审阅、本地化与引擎导出 | CAP-09B, CAP-09C |
| CAP-QA | ikzerok/worldedit | P1 | 能力路线分阶段集成、迁移与发布验收 | CAP-01B, CAP-01C, CAP-02B, CAP-02C, CAP-03B, CAP-04B, CAP-04D, CAP-05B, CAP-06B, CAP-07B, CAP-07D |
| UX-01 | ikzerok/worldedit | P0 | 已实现视觉基础改动的审阅与集成 | 无新增票 |
| UX-02 | ikzerok/worldedit | P1 | 12 页面与创作表单的视觉一致性收口 | UX-01 |
| UX-03 | ikzerok/worldedit | P1 | 图形工作区可读性：连线、标签、图例与选择层级 | UX-01 |
| UX-04 | ikzerok/worldedit | P1 | 长文排版、文字缩放与个人显示偏好 | UX-01 |
| UX-05 | ikzerok/worldedit | P0 | 键盘、无障碍与桌面/Web 响应式边界 | UX-01 |
| UX-06 | ikzerok/worldedit | P0 | 视觉回归基线、双端验收与独立发布门 | UX-01, UX-02, UX-03, UX-04, UX-05 |

## 原始研究与实现证据

- [能力调研](../../research/product-capabilities-20260926.md)
- [独立视觉报告](../../research/editor-visual-refresh-20260926.md)
- 核查 core：`889febe99f7999f8da4c28583320da61e6be13e2`。
- 核查 editor：`218d0ed5dbda70b4ce803aaae36197905d1a9708`。
- 已本地实现视觉提交：`357b1a17d96bddf08838ba7d66e94d93eb3239b7`；本机 108 tests 与原生/WASM 构建结果不能替代后续远端验收。
