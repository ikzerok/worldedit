# worldedit 使用文档

当前产品版本为 0.20.0；完整功能与验证状态见 [CHANGELOG](../CHANGELOG.md)。语言与资料解释来自配对 worldline core；默认语言仍为 1.9，最高 1.13。

## 开始创作

- [安装与启动](../README.md#安装与启动)、[工作区](workspace.md)：目录、素材、保存、冲突与完整备份
- [作者工作区](author-workspace.md)、[正文与书稿](manuscript-workspace.md)：写作、结构、源码、布局和草稿保护
- [显式创作与能力](explicit-authoring-0.14.md)：已应用稿、未应用稿、试玩与能力确认
- [查找与作者位置](search-author-context.md)、[逐处审阅后替换](selective-replace.md)、[资料旁查](reading-panels.md)、[作者反馈](author-feedback.md)

## 真实路线对照

- [作者操作](route-comparison.md)：同稿验证两条实际路径、停止状态、动作来源与边界

## 当前稿里的工程问题

- [工程问题工作台](project-problems.md)：筛选、详情、主/相关来源、当前问题摘要和作者返回
- [源码自动换行](source-wrap.md)：真实排版、物理行号、选区和阅读位置
- [0.19 版本说明](releases/v0.19.0.md)：可信来源、有界上下文、兼容和最终验收边界
- [上下文实现合同](diagnostic-source-workbench.md)：core 真源、装饰与选择、阅读字号和验收目标

## 0.15 矢量地图

- [作者工作流](vector-authoring-0.15.md)：拖绘、曲线/多子路径、文字、节点、组/图层、绑定、迁移、撤销与 SVG 交换
- [既有地图操作](maps.md)、[创建地图](map-creation.md)：旧标记、底图、文字、校准、导航和引用
- [核心 SVG 支持范围](../../worldline/spec/vector-scene.md#4-支持的-svg-profile)：受限形状/路径/文字/样式及拒绝项
- [后台计算与取消](web-worker.md)、[渲染依赖与约束](renderer-dependencies.md)

SVG 输入经预检后成为可编辑场景；不执行作者 SVG，不承诺所有 SVG/CSS。整图或当前选择的 SVG 导出只保留矢量交换范围，完整原稿和附件仍需工程备份。

## 发布离线世界站

- [四步使用说明](reader-publishing.md)：选择、资源核对、页面预览与确认发布
- [0.15 发布流程契约](reader-publishing-0.15.md)：profile、临时预览、取消、原子交付和未保存边界
- [完整世界站规范](../../worldline/spec/reader-site.md)、[旧读者包兼容](../../worldline/spec/reader-export.md)

v3 对象包含其别名与受限类型结构；属性、地图图元、章节和附件分别授权，选组不自动公开后代。静态分支说明不执行条件或状态。读者包不提供在线权限，也不替代完整工程备份。

## 资料与叙事

- [资料查询](catalog-views.md)、[Wiki](wiki.md)、[标签](tags.md)、[专题视图](topic-views.md)
- [时间约束与可信重放](replay-timeline-workflow.md)、[有界试玩](bounded-play.md)、[静态人物引用](static-authoring-0.10.md)
- [稳定 ID 重构](safe-id-refactor.md)、[本地化](localization-workbench.md)

## 界面、自动化与构建

- [共同视觉系统](visual-system-0.15.md)：导航、索引、内容、检查器、主题和长路径
- [CLI/RPC 覆盖表](cli-coverage.md)：哪些功能可独立调用，哪些仍属于运行中编辑器
- [配对构建](paired-ci.md)、[发布流程](release.md)、[性能目标与探针](performance.md)
- [worldline 规范索引](../../worldline/spec/README.md)：语言、地图、机器协议和公开选择的共同真源

当前 0.20 的完整门禁、性能与真实平台覆盖以版本说明为准；旧版结果不自动算作本版通过。自动测试、WASM/Worker 协议、offscreen 与真实原生/浏览器交互分别记录，不能替代系统 IME、读屏、高 DPI 或其他平台 GUI 验证。
