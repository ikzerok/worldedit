# 当前稿里的问题上下文合同（0.19）

状态：2026-10-03 17:10 UTC 实施前联合冻结合同；本文不声明实现或验收已通过。core真源为同级worldline的spec/diagnostic-sources.md与spec/problem-source-context.md。

## 目的与真源

复用现有工程问题列表、详情、F8/Shift+F8、主/相关来源和作者返回历史。只补“看见问题 → 理解原因 → 回到当前稿修改”的上下文；不增加右栏、第二个问题列表、自动修复、永久 issue ID 或语言解析。

编辑器只消费 core 的 `ProblemLocation`、精度与可选 source-context。文件身份、1 起 Unicode scalar 行列、UTF-8 半开原稿范围、摘录及命中角色由 core 同一编译快照投影。UI 不解析 message、不全稿搜同名词、不补缩进、不把合法但不相关的坐标当成证据。

## 当前问题身份与生命周期

- 只在显式成功定位后保存当前报告版本、条目 ID、主/相关来源身份及 core 位置；摘要不得靠 TextEdit 选区推断问题身份
- 来源页标题下显示紧凑、可折叠摘要：形状和中文严重性、原因摘要、精度/命中角色、返回既有详情动作。全文、说明、建议与技术信息仍在现有详情
- 主/相关来源导航加入既有作者历史；返回恢复原视图、未保存内容、选择方向、滚动与撤销链。当前问题上下文只可在相同报告及来源基线恢复，不跨报告猜测匹配
- 变稿、外部来源变化、检查失败/取消、过滤后丢失条目或新报告替换时，不再绘制旧来源强调；保留清楚的“待重检/需重新选择”说明，不宣称已解决
- 后台结果、主题/字号/行距/软换行变化不请求编辑焦点，不恢复已消费的跳转，不复活旧选区
- IME 组合、未应用正文/表单和外部冲突遵守现有保护；摘要不把受保护草稿当当前报告来源。绘制不写稿、不应用表单、不保存

## 来源装饰与人工选择

- 只有经 core 当前位置验证的 span 可获得轻量行侧锚点和温和命中强调；document/unavailable 明确降级，不绘制猜测范围
- 行侧锚点和范围位置由本帧真实 TextEdit galley 的字符映射得到，兼容软换行、中文、emoji、组合字、CRLF、空行、长行与字号变化，不按固定行高猜测
- 装饰独立于用户编辑选区。诊断程序定位仍可按现有行为选择文字，但该次选择不自动显示“从选中文字建档”建议；返回恢复同一程序选择仍受抑制
- 抑制绑定具体编辑器/内容/选择来源，不绑定问题面板是否打开；真实鼠标或键盘人工选择立即恢复既有建议、批注、Ctrl+Enter。显式 Ctrl+Enter 的作者动作不被被动建议抑制代替
- 光标移动不取消当前问题身份；作者可继续修订。文字更改当帧撤除旧强调，等待重检

## 详情阅读与颜色

详情顺序：问题 → 说明/建议 → 来源证据 → 技术信息。原因、说明、建议、摘录和当前摘要共用个人阅读字号与行距；技术 code/路径允许次级字号，不能用固定微小文字承载主要原因。

摘录只显示 core 给出的有界原文。可选 context 提供原文范围、摘录内命中范围、角色与裁切/partial 标记；省略符为 UI 独立标识，不混入定位字节。没有 context 的旧报告可走旧纯文本只读显示，导航仍必须经 core 严格刷新/重建校验，不能忽略新字段。

检查实际 normal/hover/selected/focused 表面：普通阅读文字对比至少 4.5:1，必要焦点/状态边界至少 3:1；严重性始终有形状和中文标签。沿用视觉系统，不整段铺红、不堆卡片。

## core DTO 与兼容联调需求

DTO 与 core `spec/problem-source-context.md` 联合冻结；编辑器不自行发明第二个 DTO。拟定 `ProblemLocation.context: Option<ProblemSourceContext>`：`version=1`、`role`（Target/Expression/Statement/Declaration/Document/Unavailable）、可选 `text`、全稿 `slice_byte_range/slice_char_range`、局部可见交集 `hit_byte_range/hit_char_range`、`visibility`（Full/Partial/NoText）及 `prefix_clipped/suffix_clipped`。必须覆盖：

1. `ProblemLocation` 上缺省为 `None` 的可选 context；主/相关来源共用
2. 原摘录 UTF-8 半开范围、摘录内命中范围、命中角色、前后裁切、命中仅部分可见；0/极小预算仍保留权威位置与角色
3. 原位置、摘录及局部范围共同绑定 content_baseline/source_observation，参与 report 摘要与整个位置 DTO 重建判等
4. 保留旧 Diagnostic 字段形状、ProblemPrecision 三值、旧请求未知字段拒绝、报告 schema 1 接受条件；旧无 context 数据可读，但不能按旧证据跨版本导航
5. 既有 512 字节 excerpt、32 MiB report、1 MiB page 上限不放宽；新字段和重复 excerpt 计入真实序列化预算。worker/native 还必须验证完整 WorkOutput JSON ≤32 MiB，不仅裸报告
6. 加法能力仅属工具/报告能力，不写入作品语言 required_features 或 Story save feature 集合

## 有界工作与验收

绘制只访问当前有界摘要/context、可见问题行及当前源码 galley；不逐帧遍历全部问题构造统计/副本，不为了筛选、分页或绘制 compile。源码文件不超过 600 行。

必须回归：主→related→Back；程序选择无建档浮层；真实鼠标/键盘选择仍可建档/批注/Ctrl+Enter；Esc/关闭/再次打开；改稿/IME/外部冲突；主题/字号/软换行重排不抢焦点；旧报告只读及严格导航拒绝；worker schema 1 旧/新 payload 双向验证与完整外壳预算。

视觉覆盖 1040×660、1280×800、1600×1000，16/28 字号，暗/亮/系统主题，软换行开/关、长中文混合文本、Tab/Shift+Tab、Enter/Space。保留上方正文至少 240 点和底部工具不超过可用高度 55% 的既有预算。

性能依同轮批准范围：129 源码/2,076,104 字节/3072 问题 fixture 不变；绘制预热 60 帧后 p95 ≤40ms，装饰相对无装饰峰值 RSS 增量 ≤32 MiB。实际 native、offscreen 与真实 Web 分别记录；未测 Win/macOS/Web/IME/读屏/高 DPI 不写成通过，颜色计算也不等同无障碍认证。
