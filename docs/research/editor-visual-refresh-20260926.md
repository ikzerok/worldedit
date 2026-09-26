# 编辑器视觉优化：独立设计与验证记录

日期：2026-09-26。对应需求：研究编辑器设计审美并实际优化界面。与同目录 `product-capabilities-20260926.md` 的功能能力对标分别交付。

## 1. 设计研究

Linear 的 2026-03 视觉刷新强调统一页头/导航/视图控件、重绘图标并弱化侧栏的亮度，以便主要内容更突出。这是视觉层级的参考，不将 Linear 当作世界观创作能力竞品。
来源：https://linear.app/changelog/2026-03-12-ui-refresh
设计说明：https://linear.app/now/behind-the-latest-design-refresh

novelWriter 展示了正文查看、界面主题与专注模式可以分别设计；长文工作区应把阅读体验和辅助信息分开，而不是让每块区域都一样抢眼。
来源：https://novelwriter.io/features.html

文字可读性参考 WCAG 2.2 的常规文字对比度 4.5:1。此处仅用作本轮配色验收指标，不声称整个原生/Web 应用已经通过完整 WCAG 认证。
来源：https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html

## 2. 本轮实际修改

视觉系统：石墨灰主背景、较暗导航与标题栏、逐级抬高的面板/卡片；青绿只用于主要动作与选中状态。保留独立的蓝色、警告与错误颜色。
排版：标题 24、正文/按钮 14、辅助说明 12.5；缩小不必要的控件间隔和圆角，卡片使用 18 的内部留白。
导航：12 个既有页面按“创作 / 世界资料 / 探索与协作”分组；左对齐标签、统一图标尺寸、选中竖线、键盘焦点边框。未删除任何页面。
侧栏：整体可以滚动，文件区域不再被一长列导航永远挤出；长工作区名截断并保留悬浮完整名称。支持工具栏按钮或 Ctrl+Shift+B 收起/展开，状态只作用于本次应用会话。
工具栏：保存成为主操作，导出降为辅助操作；足够宽时显示当前页面提示；保留窗口控制、工程菜单、搜索和试玩入口。
状态栏：分离检查状态与保存状态；长操作消息按剩余宽度截断并通过悬浮显示，不挤占撤销/重做。
阅读：正文概览居中并限制阅读列最大 880 逻辑点；资料阅读窗口更宽，仍保留链接、历史和来源操作。窄区域按实际可用宽度收缩。

## 3. 不变量

本轮不改 worldline-core、语言规范、文件格式、关系语义或演练状态机。导航、收起侧栏、浏览和阅读排版不进入 Project 写入或撤销历史。
图形选中状态不代表新增内容关系；完整作者工程导出仍不被称为读者发布。未新增依赖、字体文件、云接口或网络访问。

## 4. 已完成的验证

编辑器 `cargo test --locked`：108 项通过，0 失败、0 忽略；其中本轮新增 5 项回归。
新增回归：12 个页面在分组中各出现一次；导航/收起/展开不改工程与历史；1040×660、1280×760、1700×1000 下主工具栏不侵入窗口按钮；正文/辅助文字与主要表面、主按钮的对比度；窄/宽阅读列边界。
`cargo fmt --all -- --check`、原生全目标严格 Clippy、wasm32 严格 Clippy 均通过。
原生 Release 和 wasm32 Release 构建通过；Trunk Release 静态包构建通过。

Windows 发布模式程序实际启动，使用独立的雾港样例工作区取得时间线与事件关系图截图。未将所有页面都描述为人工逐项验收。
Edge headless 加载本轮 Trunk 产物，入口/JS/WASM/icon 全部 HTTP 200，进程 exit 0；已查看实际渲染截图，不仅检查资源请求。浏览器导入/下载对话框不在本次新增烟测范围。
只对新的视觉系统与已有流程做回归；没有重新声称全部平台或全部无障碍要求通过。

## 5. 交付位置

源码主要在 `src/theme.rs`、`src/app/navigation.rs`、`src/app/workspace.rs`、`src/app.rs`、`src/chrome.rs`、`src/app/overview.rs`、`src/app/reading.rs`。
测试位于 `src/theme.rs`、`src/app/navigation.rs` 和 `src/app/authoring_ui_tests.rs`。
本机证据保存在组合目录 `target/ui-polish-20260926/`，包含 `results.json`、逐步构建日志、`browser-result.json`、原图和预览图。
截图包括 `before-timeline.png`、`after-timeline.png`、`after-event-graph.png`、`after-web.png`。图像来自实际编辑器，不是效果图。
本轮原生程序为 `worldedit/target/release/worldedit.exe`；Web 静态产物为组合目录 `target/ui-polish-20260926/web/`。
本轮不覆盖既有 v0.3.0 Release，不将构建缓存或截图提交到源码仓库。
