# worldedit 设计文档

这里保留早期编辑器产品与架构设计，以及后续工作台设计资料。它们按各自日期和冻结基线阅读；现行使用入口见[文档索引](../README.md)，语言与持久格式以当前配对 worldline 规范为准。

| 文档 | 用途 |
|---|---|
| [产品需求文档](PRD.md) | 地图画布、资料、网络视图和交互验收目标 |
| [系统架构文档](ARCHITECTURE.md) | 编辑器、快照、命令、渲染和双端边界 |

共同语言、展示文档和文件格式只在 worldline 的规范中维护：
[共同契约（历史设计基线）](https://github.com/ikzerok/worldline/blob/3f2255ef65a1da76dc34fed818f760cf9e8a8edc/spec/presentation.md)。

本组早期设计固定引用 worldline `3f2255ef65a1da76dc34fed818f760cf9e8a8edc`，用于保留当时依据，不等同于当前 [compatibility.json](../../compatibility.json) 的配对提交。复测当前产品应按兼容记录检出，再读取该提交的[规范索引](../../../worldline/spec/README.md)；不要用历史设计 SHA 替换当前配对。

[2026-09-26 工作台设计](editor-system/20260926/README.md)与[可选原型决定](ADR-20260930-optional-prototype.md)分别保留其原始范围；历史演示页面不随正式应用和源码归档分发。

主文档描述设计目标和验收口径；规范、Schema 或样例存在不代表编辑器功能已经实现。
