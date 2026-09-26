# EDS-10：高保真候选样板与浏览器证据

2026-09-26；对应 [worldedit #41](https://github.com/ikzerok/worldedit/issues/41)。本文件补充视觉系统研究，供审查选择，不代表导航、交互架构或全部样式已经获批量产。

## 交付与运行边界

[打开独立 HTML 样板](eds10-visual-sample.html)。无需依赖和构建，可直接打开，或从本目录运行 `python -m http.server 8768 --bind 127.0.0.1`，访问 `http://127.0.0.1:8768/eds10-visual-sample.html`。上方选择任务、亮度、密度、失败情境；「组件样板」切换状态目录。

所有数据均为合成内容，未加载、创建或改写用户作品，源码仓库不是作品目录。本样板没有 Project/core/runtime 接线；地图组合事务、正文语义插入、确定性重放、提案字段合并均不是已实现能力。操作只更新本页提示或视图；文本草稿在本页切换任务时保留，刷新后重置。取消按钮说明保留输入，不假称撤销了事务；取消预览只记录请求，不假称 runtime 有变化。地图非拖动按钮展示后备设计说明，尚未实现坐标编辑。

四项候选视觉决定（V10-* 是本文件候选编号，不替代正式设计决议）：

|候选决定|任务/模式依据|具体样板与回退|
|---|---|---|
|V10-01 四类状态独立编码|J1/J3，PAT-05；选择、工具、焦点与未保存可同时存在|蓝色边标/圆、紫色菱形、青色外框、琥珀点与文字；若颜色难辨仍有形状和标签，不复用一个高亮|
|V10-02 保留失败与恢复信息|J2/J4，PAT-07|错误块有文字、边线和恢复按钮；过期采纳禁用且保留原因；紧凑模式不能隐藏风险|
|V10-03 亮度与信息密度分离|J1–J4，PAT-12|浅/深 × 舒适/紧凑独立切换；紧凑不缩小到低于命中下限；正文仍保持独立行距|
|V10-04 窄窗先保留任务与输入|J2/J4，PAT-07/12|侧栏移至主区下方，三列改显式列切换；导航暂收起但任务选择仍在；失败提示位于操作前。导航数量仍待上游决定|

## Token 清单与禁止混用

以下数值均为 CSS 候选，不能直接视作 egui point。CSS 中 `:root` 与主题/密度覆写为可运行真源。

|组/变量|浅色|深色|用途|
|---|---|---|---|
|surface：canvas / surface / subtle / raised|#edf0f4 / #ffffff / #f4f6f9 / #ffffff|#111b27 / #1b2939 / #223447 / #263a50|画布、工作面、分组、抬升面|
|text / muted|#263342 / #536375|#edf2f7 / #bbcfe1|正文、次要说明；错误原因不得只用弱化文字|
|selection / selection-bg|#17559a / #e6f0ff|#a6c9ff / #29496d|对象选择，不表示键盘焦点或提交成功|
|focus|#006f80|#7ee4ee|独立 3px 外轮廓，offset 3px；不能替代选择|
|active / dirty|#6438a0 / #875000|#d5b6ff / #ffd185|工具 / 未保存；未应用与已落盘文案分开|
|error / error-bg|#a52c35 / #fff0f1|#ffacb5 / #482d38|失败、冲突与恢复；不可用正文色不是唯一错误线索|
|preview / success|#665092 / #236644|#d5b6ff / #97ddb8|预览 / 运行位置。深色 preview 与 active 同色但组件形状及标签不同，不能仅依颜色判断|
|button / on-button|#17559a / #ffffff|#a6c9ff / #172b43|主命令；禁用时回退 subtle/muted 并保留原因|
|edge：line|#aeb9c6|#7f96ad|分隔边线；不能替代必须辨识的状态轮廓|

edge 语义：控制流为实线箭头，时间偏序为虚线箭头，语义关系为实线圆端；预览是附加状态标签，不把控制流改画成时间边。地图区域虚线属于几何范围，不在逻辑图中复用其含义；所有图保留图例。

spacing：舒适 gap 20、control 40、row 48；紧凑 12、32、38（CSS px）。固定间距 8/10/12/14/16/18/24 为本轮细部候选，尚未统一成最终间距阶梯。type：界面 15/1.6，次文 13，标签 11–12；h1 24/1.3，长文标题 26；长文 17/1.95 或 16/1.8，最大宽 65ch，系统字体回退 Microsoft YaHei，身份使用等宽字体。motion：背景反馈 120ms，`prefers-reduced-motion: reduce` 时为 0；错误与内容不依靠动画出现。hit_target：常规控件至少 32px 高，舒适 40px，地图标记至少 44px；正文内联文字若成为链接需单独按例外或间距评估，不能把静态示意卡的边框视作实际命中区。

## 组件与状态覆盖

组件目录提供命令按钮、检查器字段、资料候选行、阅读标签、诊断行、地图标记、图节点/边、正文链接共八类。每类可切换 default、hover、focus、selected、disabled、read-only、dirty、preview、error、loading、empty、conflict 共十二种视觉状态，包含选择+焦点+未保存叠加样例。浏览器逐项切换，均得到八张非空卡。

目录是静态外观标本，不声称其中的 div 是真实可交互控件。状态应用边界如下，避免把全排列当成产品语义：

|组件|直接状态|属于 owner/容器而非该控件的状态|
|---|---|---|
|命令按钮|default/hover/focus/disabled；toggle 可 selected|read-only、dirty、preview、error、loading、empty、conflict 描述命令所作用的上下文，不把按钮标为“已保存”|
|检查器字段|default/hover/focus/read-only/disabled/dirty/error/conflict|selected 表示对象选择；preview/loading/empty 由资料区解释|
|候选行/阅读标签|default/hover/focus/selected/disabled|dirty/read-only/preview/error/conflict 属于对象或文档；loading/empty 属于集合|
|诊断行|default/hover/focus/selected；明确 error/conflict 文案|disabled/read-only 不抹掉诊断；dirty/preview/loading/empty 属于诊断来源和列表|
|地图标记/图节点与边|default/hover/focus/selected/preview/error；锁定则 read-only|dirty/conflict 属于草稿；disabled 说明操作不可用；loading/empty 属于图层。边焦点须由真实可访问入口支持，样板未实现|
|正文链接|default/hover/focus，失效则 error 并保留原文字|selected 是文字选区；dirty/conflict/read-only 属于文档；preview 属于预览面；loading/empty 属于查找结果，不作为链接新含义|

任务视图中的真实 HTML 控件另行检查键盘操作。hover/focus 目录状态是显式示意，不能当作鼠标悬停或焦点测试证据。

## 截图索引（RUN，不是 USER）

2026-09-26 使用本机 Edge 浏览器扩展会话，经 browser skill 驱动正常页面操作。图片为浏览器实际 JPEG 截图，不是生成图片。宽高表示设置的 CSS viewport，部分 full-page 截图像素高度会更长。截图右侧可能出现浏览器翻译扩展浮层，不属于样板。截图保留当时页面滚动位置，不能用单张图推断页面全部内容可见。

|证据|视口/配置|检查重点|
|---|---|---|
|[J1](eds10-evidence/j1-light-comfortable-wide.jpg)|1440×1000，浅/舒适|地图、同名身份、四类状态、固定 B|
|[J2](eds10-evidence/j2-dark-compact-wide.jpg)|1440×1000，深/紧凑|长文、选文示意、草稿、独立行距|
|[J3](eds10-evidence/j3-light-compact-wide.jpg)|1440×1000，浅/紧凑|运行位置、选择、预览分离，控制流图例|
|[J4](eds10-evidence/j4-dark-comfortable-wide.jpg)|1440×1000，深/舒适|base/current/proposal 与审阅草稿|
|[J2 失败](eds10-evidence/j2-light-compact-narrow-conflict.jpg)|760×900，浅/紧凑|冲突与失效引用、恢复入口、禁用应用|
|[J4 失败](eds10-evidence/j4-dark-comfortable-narrow-conflict.jpg)|760×900，深/舒适|列切换、冲突保留、禁用采纳|
|[中等宽度](eds10-evidence/j4-dark-compact-1040-conflict.jpg)|1040×660，深/紧凑|三列仍在，无横向溢出；长内容允许纵向滚动|
|[浅色组件焦点](eds10-evidence/components-light-focus-wide.jpg)|1440×1000，浅/紧凑|八类组件及叠加状态|
|[深色组件冲突](eds10-evidence/components-dark-conflict-wide.jpg)|1440×1000，深/紧凑|冲突不靠颜色单独传意|
|[J2 键盘](eds10-evidence/j2-narrow-keyboard-focus.jpg)、[J4 键盘](eds10-evidence/j4-narrow-keyboard-focus.jpg)|760×900|真实 Tab 后焦点位于不可用操作，仍能查原因|

## 可访问性抽样与重现

Web 参考而非原生认证：[普通文字 4.5:1](https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html)，大文字可 3:1；[目标至少 24×24 CSS px 或符合明确例外](https://www.w3.org/WAI/WCAG22/Understanding/target-size-minimum.html)；[键盘焦点不能完全被作者内容遮挡](https://www.w3.org/WAI/WCAG22/Understanding/focus-not-obscured-minimum.html)。本轮不借用目标间距、内联文字等例外为未测控件宣称通过。CSS px 与原生 point/物理像素须在移植后分别测量。

从浏览器 `getComputedStyle(document.documentElement)` 读取 token，以 sRGB 线性化和相对亮度公式 `(L高+0.05)/(L低+0.05)` 计算。以下为前景对 surface 的抽样比值（显示保留两位；全部远离 4.5 阈值）：

|前景|浅色|深色|
|---|---:|---:|
|text|12.85|13.10|
|muted|6.16|9.22|
|selection|7.49|8.73|
|focus|5.86|9.99|
|active|8.03|8.41|
|dirty|6.61|10.34|
|error|6.97|8.31|
|preview|6.71|8.41|

error 对 error-bg：浅 6.30、深 6.92。此为特定配对，不包含全部背景、透明叠加、禁用态、选中文本、边线、按钮或图形对比度，不能外推全应用合规。

实际操作与结果：

1. J2 深色紧凑 1440×1000：检查可见控件矩形，高度至少 32 CSS px，抽样最窄控件约 85.14px；页面无横向溢出。textarea 高约 175px。
2. J4 深色舒适失败，760×900：填入「核实潮汐时间，保留东侧通道。🌌」，点击「提案」，DOM 的输入 `.value` 保持完全相同，显示列为 proposal；点击恢复说明，status 显示保留输入并重新打开当前内容后合并。
3. 同场景从审阅输入 Tab 到「采纳提案（样例）」；该控件 aria-disabled=true 但可聚焦以了解原因。矩形约 x36.57/y484/w145.14/h40，完全在视口中；实际 outline 为青色 solid 约 2.85714px（源 CSS 3px）。本机缩放导致测量小数，不把声明值当实测值。
4. J2 浅色紧凑失败，760×900：输入「中文输入与 🌌 草稿保留检查」，Tab 到「应用草稿（样例）」；矩形约 x67.63/y596.94/w153.14/h41.94，焦点轮廓可见；无横向溢出。切 J1 再回 J2，`.value` 保留同一字符串。此为自动填入 Unicode，不是中文 IME composition 验证。
5. J4 深色紧凑失败，1040×660：base/current/proposal 三列 DOM 均可见，无横向溢出；可见 button/input/textarea/select 的标签启发式扫描未发现无名控件。该扫描不是完整可访问树/读屏审核。
6. 十二种目录状态逐个切换，每种均呈现八张非空标本卡；宽屏目录无横向溢出。运行后恢复了浏览器 viewport 设置。

焦点截图是两个代表性 Tab 转移的证据，尚未穷举整页 Tab 顺序、遮挡、焦点恢复或所有键。aria-live/alert、readonly、aria-disabled 有标记，但尚未由读屏器核实。减少动态效果仅有 CSS 规则，尚未模拟系统偏好。200% 文本缩放、强制颜色、触屏、色觉差异、真实中文输入法、不同系统字体和 egui 原生界面均未验证。没有用户参与测试或完整无障碍认证。

## 评审出口

可以审查四任务的视觉层级、明暗与密度、失败状态、颜色含义、组件标本和本页局部行为；不能据此关闭上游架构/能力票。若采用候选，下一步是选定 token 与真实组件适用状态，补完实际背景配对与完整键盘/读屏检查，再在原生控件中测量 point 和缩放。任何样式与安全操作冲突，应回退样式并保留风险及恢复入口。本交付不关票、不批准导航数量、不加入游戏引擎适配器。
