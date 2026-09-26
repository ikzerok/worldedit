# 编辑器设计系统研究包 · 2026-09-26

从编辑范式、对象、任务和状态出发重建 worldedit 设计指导，而非沿用旧视觉外壳。

主文档：[编辑器交互架构与设计研究指南](guide.md)。\
证据：[22 个官方来源](sources.json)、[4 条初始观察](observations.json)。\
模板：[观察卡](templates/observation.md)、[决定卡](templates/decision.md)、[工单](templates/ticket.md)。\
工单状态与替代关系见 `tickets.md` 和 `tickets.json`（发布后生成）。

本轮没有商业 GUI 全量实操、目标用户实验或新界面生产实现；证据缺口在指南和观察卡中明确。旧视觉路线可替代，能力路线不合并、不撤销。前轮视觉代码保留在原分支，主线和已发布版本不改动。

运行 `python validate.py` 验证文档 ID/来源/观察/工单引用的一致性；通过只表示文档结构一致，不表示设计或产品已经通过验收。
