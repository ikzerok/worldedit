# EDS-01：任务路径证据补充

访问日：2026-09-26。关联：[worldedit#32](https://github.com/ikzerok/worldedit/issues/32)。状态：DOC 研究交付；研究批准、GUI 实测和目标作者测试均未完成。本文件补充 [guide.md](guide.md)、[来源台账](sources.json) 和 [初始观察](observations.json)，不覆盖其历史记录。

## 证据边界和统一观察字段

本轮依据官方文档构造可复查的操作路径，记录的是“文档描述什么”，不是研究员已经执行什么。所有 OBS-E01-* 均为 DOC；机制解释与迁移建议为 HYP。IMG、RUN、USER、DEC 均未新增。未采购、未登录商业产品、未打开私人项目、未修改产品代码。

各卡共同字段：观察者为文档研究 agent；目标角色为需要跨对象旁查的创作者（研究设定，不是受试者）；熟练度 unknown；任务频率 unknown；实际前置文件 null；实际账户权限 unknown；逻辑尺寸、DPI、缩放、区域比例、截图路径、帧时间、hash、寻找时间、误操作和恢复计数均为 null。后文 A/B、照片和片段均为复现夹具要求，尚未创建或操作。键盘真实焦点和输入法组合态除文档明确说明外一律 unknown；hover 不视为 keyboard focus。所有卡的验证结果均为“DOC 支持所列规则，RUN/USER 未验证”。

“取消”区分撤销内容修改、退出观察状态、取消尚未确认操作。未说明的取消规则写缺口，不能以通用 Esc 猜测。选择/布局类任务的内容修改和内容提交标 N/A，理由是其任务终点仅为定位或恢复界面；磁盘持久化未观测，不推断其一定不写个人配置。

## SRC 复核台账

沿用来源编号，SRC-S02 表示来源台账中的 S02。以下页面均于本轮访问；标题为页面标题或台账同名标题。动态帮助页不等于已安装版本。

| SRC | 官方来源及本轮取得形式 | 产品/版本/平台/权限边界 |
|---|---|---|
| S02 | [Select layers and objects](https://help.figma.com/hc/en-us/articles/360040449873-Select-layers-and-objects)，正文 | Figma Design；版本 unknown；文档列 Windows/Mac；普通选择支持 all plans 的 can view/can edit，匹配选择需 can edit；实际权限 unknown |
| S03 | [Workspaces](https://docs.blender.org/manual/en/latest/interface/window_system/workspaces.html)，官方搜索索引正文 | 本轮 latest 标题为 5.2 LTS；桌面；安装版本 unknown；不能与 S04/S05 拼成单一实测版本 |
| S04 | [Areas](https://docs.blender.org/manual/en/5.0/interface/window_system/areas.html)，官方搜索索引正文 | 文档 5.0；桌面；直接抓取返回 402，未把抓取错误解释为产品收费 |
| S05 | [Properties Editor](https://docs.blender.org/manual/en/5.0/editors/properties_editor.html)，官方搜索索引正文 | 文档 5.0；桌面；直接抓取返回 402；没有原图证据 |
| S06 | [Workspace basics](https://helpx.adobe.com/lightroom-classic/desktop/workspace/workspace-basics.html)，正文 | Lightroom Classic；页面更新日 2024-02-21；Windows/Mac；安装版本、许可 unknown |
| S07 | [Work with the Develop module](https://helpx.adobe.com/lightroom-classic/desktop/process-and-develop-photos/develop-module-tools.html)，正文 | Lightroom Classic，非云版；版本、许可 unknown；Windows/Mac |
| S08 | [Arrange the main window](https://support.apple.com/en-ph/guide/final-cut-pro/ver2a27194eb/mac)，正文 | Final Cut Pro Mac，非 iPad；版本、许可 unknown |
| S09 | [Timeline index](https://support.apple.com/en-is/guide/final-cut-pro/ver4cdeffd2/mac)，正文 | Final Cut Pro Mac；版本、许可 unknown |
| S10 | [Playhead](https://support.apple.com/en-is/guide/final-cut-pro/verb97d10d4/mac)，正文 | Final Cut Pro Mac；版本、许可 unknown |
| S12 | [DaVinci Resolve](https://www.blackmagicdesign.com/products/davinciresolve)，官方产品页正文 | 本轮页面标 Resolve 21；实际安装/版本/许可 unknown；只引用 page 分工 |
| S14 | [Viewer Active](https://derivative.ca/UserGuide/Viewer_Active)，正文 | TouchDesigner；版本/实际平台/许可 unknown |
| S15 | [Unreal Editor Interface](https://dev.epicgames.com/documentation/unreal-engine/unreal-editor-interface)，正文 | 本轮标题标 UE 5.8；实际安装/平台/许可 unknown |
| S16 | [Intro to writing & editing](https://www.notion.com/help/writing-and-editing-basics)，正文 | Notion；动态帮助页；客户端版本/实际平台/权限 unknown |
| S18a | [Logic Pro Smart Controls interface](https://support.apple.com/en-gb/guide/logicpro/lgcp91601243/mac)，官方搜索索引正文 | Logic Pro Mac；版本/许可 unknown；原 S18 重定向后的长页检索失败，以同一官方指南的地区页面补证 |

## 四个主样本的 DOC 观察卡

### Figma：定位和检查对象

对象模型：frame/group 为包含容器，子 layer 是被选择的对象，Layers 与 canvas 为同一对象的两种入口。未核实内部稳定 ID。以下四卡共用 [S02](https://help.figma.com/hc/en-us/articles/360040449873-Select-layers-and-objects)；pin 与真实键盘焦点 unknown；内容写入/保存 N/A（选择任务），不是证明编辑事务的证据。

| OBS / 前置与成功终点 | 进入 → 选择/状态修改 → 确认 | 取消 / 失败 / 恢复 | 事实、HYP 与验证缺口 |
|---|---|---|---|
| OBS-E01-F1；嵌套 A/B；选到 B | canvas 点击先选父；双击或 Enter 下钻；owner 为最终 child | Esc 清空；误选父时继续下钻；可 Shift+Enter 返回父 | DOC：逐层选择。HYP：显示路径防误改 → PAT-01；未测输入框焦点下 Enter |
| OBS-E01-F2；目标在结构中；定位选中 | 展开 Layers 容器；hover 显示位置提示；点击名称才选择 | 无 hover 提示可检查 Highlight on hover；Esc 清空后重选 | DOC：悬停与选择分离。HYP：资料列表提供 reveal → PAT-01；未测遮挡和滚动恢复 |
| OBS-E01-F3；A/B 多选；得到准确集合 | canvas 选 A，Shift 点击 B；owner 是选择集合 | Shift 再点 B 移出；Esc 清空 | DOC：集合可增减。HYP：批改前显示范围 → PAT-02；批改/撤销不在本卡已核实范围 |
| OBS-E01-F4；锁定对象；可定位检查 | 正常左击不能选锁定对象；右键 Select layer 选择带锁条目 | hidden layer 不出现在该菜单；需恢复可见；Esc 退出选择 | DOC：锁定不等于完全不可定位。HYP：只读仍允许旁查 → PAT-01；不据此断言解锁权限 |

迁移建议：地图入口与其引用资料必须分别标 owner；不把同名当同一对象。不适用：多选能力未由 core 提供时不得照搬批量编辑；替代为逐个打开只读资料。F1–F4 只完成选择任务族 DOC 配额；属性输入期间切对象、云同步失败、撤销粒度仍为缺口。

### Blender：切换、固定、专注、取消布局操作

对象模型：Workspace 包含 Area，Area 承载 Editor；Properties 可固定 data-block，不能把面板 pin 与 data-block pin 混用。以下各卡的内容修改 N/A；布局保存另见 S03，不能推导 worldedit 布局应写作品。

| OBS / 前置与成功终点 | 进入 → 状态修改 → 确认 | 取消 / 失败 / 恢复 | 事实、HYP 与验证缺口 |
|---|---|---|---|
| OBS-E01-B1；已有两个工作区；返回原环境 | 点击任务 tab，再点原 tab；owner 为 workspace；selection/focus unknown | 最后一个 workspace 不能删除；本卡不执行删除 | [S03](https://docs.blender.org/manual/en/latest/interface/window_system/workspaces.html) DOC：工作区可随 blend 保存，Load UI 控制载入布局；HYP → PAT-03/10；跨区 scene/mode 不假定不变 |
| OBS-E01-B2；A/B；固定检查 A | Properties 显示 A；点击 pin；再选择 B，检查 owner 仍 A | 再点 pin 恢复跟随；真实编辑焦点 unknown | [S05](https://docs.blender.org/manual/en/5.0/editors/properties_editor.html) DOC：固定 data-block；HYP → PAT-02；A 删除后的处理未核实 |
| OBS-E01-B3；多区域；专注后恢复 | View > Area > Toggle Maximize Area；主区放大；owner 为 Area | 再切换或 Back to Previous 恢复布局 | [S04](https://docs.blender.org/manual/en/5.0/interface/window_system/areas.html) DOC：最大化保留顶部/状态栏；HYP → PAT-03；未測焦点恢复和屏幕移除 |
| OBS-E01-B4；区域角部；拆分或安全放弃 | 角部拖动开始 docking，预览分区；释放确认 | 释放前 Esc 或右键取消；快捷键很多依鼠标所在 editor 分派 | [S04](https://docs.blender.org/manual/en/5.0/interface/window_system/areas.html) DOC：可取消交互；HYP → PAT-10/11；不可据此推断布局撤销栈 |

迁移建议：先用有限预设与可见返回入口；中文输入时以真实键盘焦点路由命令。反例：全自由拆分造成窄区、误吞快捷键；替代为固定分区和重置个人布局。源文档允许工作区携带 scene/mode 设置，因此“切工作区绝不改变活动内容”是本项目拟定约束，不是 Blender 事实。

### Lightroom Classic：对照、恢复与参数分组

前置为已导入照片 A/B；owner 为 Active 照片，Reference 是对照角色。各卡实际许可、焦点和磁盘保存时机 unknown。选取的是参数与视图任务，不要求提供世界关系图（N/A：领域不相同）。

| OBS / 成功终点 | 进入 → 选择/修改 → 确认 | 取消 / 失败 / 恢复 | 事实、HYP 与验证缺口 |
|---|---|---|---|
| OBS-E01-L1；对照 B 调 A | 选 A，Open in Reference View；放入 B；右侧工具调整 Active | 选 Crop 会提示离开；Cancel 留在 Reference；D 返回 Develop 单图 | [S07](https://helpx.adobe.com/lightroom-classic/desktop/process-and-develop-photos/develop-module-tools.html) DOC；HYP → PAT-02/06；输入取消是否回滚需 RUN |
| OBS-E01-L2；辨别修改效果 | Develop 显示 Before/After；观察两版；不复制设置 | 返回单图；Before 可被复制设置改变，并非永久原件 | [S07](https://helpx.adobe.com/lightroom-classic/desktop/process-and-develop-photos/develop-module-tools.html) DOC；HYP → PAT-06；无额外内容提交；持久历史未验证 |
| OBS-E01-L3；恢复参数 | 已有调整；Edit > Undo 回退；或 Reset 回默认 | Reset 范围大于单个字段；事先保存 snapshot/preset 是手册建议 | [S07](https://helpx.adobe.com/lightroom-classic/desktop/process-and-develop-photos/develop-module-tools.html) DOC；HYP → PAT-05/06；不把 Done 等同撤销 |
| OBS-E01-L4；聚焦一个面板并找回工具 | 面板标题菜单选 Solo；只展开所需组；可用 Tab 隐藏侧栏 | 重切显隐找回；自动隐藏不适合时选 Manual | [S06](https://helpx.adobe.com/lightroom-classic/desktop/workspace/workspace-basics.html) DOC；HYP → PAT-03/05；内容修改 N/A，焦点/滚动恢复 unknown |

迁移建议：给地图外观即时预览，正文仍用受保护草稿；重置须写明范围。反例：自动折叠藏住错误，Before 被误认为不可变基线；替代为固定错误摘要及明确 base/current/proposed。尚未核实：精确数值输入、拖动中 Esc、批量同步范围、只读目录和崩溃恢复；不能用这些卡宣称本地写入安全已验证。

### Final Cut Pro Mac：索引、观察位置与区域恢复

前置为包含多个片段的测试 project。对象区分源素材、timeline 使用实例和位置状态；本轮文档不足以确认内部唯一 ID。focus/pin unknown，以下均不执行剪辑内容写入；内容确认/保存 N/A，个人布局持久化时机 unknown。

| OBS / 成功终点 | 进入 → 状态修改 → 确认 | 取消 / 失败 / 恢复 | 事实、HYP 与验证缺口 |
|---|---|---|---|
| OBS-E01-C1；文字定位片段 | Timeline 的 Index 打开索引；点击条目；playhead 跳到对应位置 | 误选后重新定位原位置；没有文档保证自动返回栈 | [S09](https://support.apple.com/en-is/guide/final-cut-pro/ver4cdeffd2/mac) DOC；HYP → PAT-07；不能把索引点击当无位置副作用 |
| OBS-E01-C2；临时扫看其他内容 | 开启 skimming 后移动指针；保留 playhead 的位置 | 关闭 skimming 或 skimmer 离开片段，默认位置回到 playhead | [S10](https://support.apple.com/en-is/guide/final-cut-pro/verb97d10d4/mac) DOC；HYP → PAT-09；两者同在片段时 skimmer 可成为播放/编辑默认位置，禁止照搬为 runtime 规则 |
| OBS-E01-C3；隐藏检查器后找回 | Inspector 按钮或 Command+4 切换显隐 | 再次切换恢复；browser 与 timeline 不可同时隐藏 | [S08](https://support.apple.com/en-ph/guide/final-cut-pro/ver2a27194eb/mac) DOC；HYP → PAT-03；未測未提交输入时隐藏 |
| OBS-E01-C4；给主区更多空间 | 拖动区域间边界；一侧增大另一侧缩小 | 反向调整恢复；精确旧尺寸恢复/拖动 Esc 未获证据 | [S08](https://support.apple.com/en-ph/guide/final-cut-pro/ver2a27194eb/mac) DOC；HYP → PAT-10；browser/viewer 共有底边联动，不能假定独立大小 |

迁移建议：文字索引共享对象身份，但 worldedit 的选中事件不能自动推进 runtime。反例：把书稿顺序当世界发生时间；替代为分别命名的编排列表与偏序图。源素材回溯、范围选择、磁性移动影响范围、撤销和保存失败仍需专门操作卡，不能由上表外推。

## 五个补充样本：限定机制观察

以下同样仅 DOC，未运行；取消/失败未由所引页面明确覆盖时保持缺口。每行事实与 HYP 分列。

| OBS / SRC | 官方可观察事实 | HYP 迁移与反例、替代 |
|---|---|---|
| OBS-E01-X1 / [S12](https://www.blackmagicdesign.com/products/davinciresolve) | Resolve 用不同 page 配置任务工具，如编辑、Fusion、调色、Fairlight、交付 | PAT-03：按任务组织工作环境；反例：作者来回跳任务而非顺序流水线；替代：保留快速返回和对象上下文。广告中的效率不当测量 |
| OBS-E01-X2 / [S14](https://derivative.ca/UserGuide/Viewer_Active) | Viewer Active 区分节点移动与内部预览交互；激活时仍可从名称处拖节点；Camera COMP 是内部移动不改参数规则的例外 | PAT-11/12：命中区/模式须可见；反例：内部预览并非普遍无副作用；替代：世界图节点正文另开阅读器。临时快捷键描述有多种形式，复现前核对目标版本 |
| OBS-E01-X3 / [S15](https://dev.epicgames.com/documentation/unreal-engine/unreal-editor-interface) | Content Drawer 失焦收起；Dock in Layout 建立常驻 Content Browser 实例；Viewport/Outliner 选择驱动 Details | PAT-01/04：临时查找和常驻查找分开；反例：输入/拖放中失焦误关；替代：固定侧栏。持久条件和未提交输入需 RUN |
| OBS-E01-X4 / [S16](https://www.notion.com/help/writing-and-editing-basics) | 块旁菜单与斜杠命令提供内容类型和块操作入口 | PAT-08/11：就地操作有多入口；反例：IME 组合输入、触屏和仅 hover 控件；替代：稳定菜单和键盘命令。选区、退出后光标与云保存失败未测 |
| OBS-E01-X5 / [S18a](https://support.apple.com/en-gb/guide/logicpro/lgcp91601243/mac) | Smart Controls 面向选中轨道；单控件可影响多个参数，检查器可查看映射 | PAT-05/11：常用操作可聚合但需解释影响字段；反例：一个开关暗改多个未知字段；替代：显式批量提案和原子事务。撤销粒度未测 |

## SRC → OBS → PAT 及反例替代登记

PAT 名称沿用指南；所有迁移规则仍是 HYP，尚无新增 DEC。TEST-E01-* 是待执行验收用例标识，不是已有测试文件或 PASS。

| PAT | SRC → OBS | 候选项目规则 | 必须检验的反例 / 替代方案 | 待执行 TEST |
|---|---|---|---|---|
| PAT-01 共享身份、多投影 | S02/S15 → F1–F4/X3 | 用稳定 TargetRef reveal | 同名不同对象 / kind+ID 列表消歧 | TEST-E01-01 同名、重命名、删除 |
| PAT-02 跟随与固定 | S05/S07 → B2/L1 | pin 独立 owner | 固定 B 却以 selection A 批改 / 显式单对象表单 | TEST-E01-02 A/B owner 与草稿 |
| PAT-03 任务预设、可逆专注 | S03/S04/S06/S08/S12 → B1/B3/L4/C3/X1 | 显式切换并恢复 | 表单随隐藏卸载 / 保留草稿、取消切换 | TEST-E01-03 专注前后状态 |
| PAT-04 临时库与常驻库 | S15 → X3 | 可固定检索入口 | 拖放时失焦 / 常驻侧栏 | TEST-E01-04 取消插入归还焦点 |
| PAT-05 渐进参数 | S06/S07/S18a → L3/L4/X5 | 常用项稳定、高级分组 | 错误藏在折叠区 / 固定错误摘要 | TEST-E01-05 未知值与错误展开 |
| PAT-06 即时预览、明确提交 | S07 → L1–L3 | 预览与提交分域 | Before 可变、误当基线 / 显式三方基线 | TEST-E01-06 取消与陈旧拒绝 |
| PAT-07 图形加索引 | S09 → C1 | 索引定位共享 ID | 排序伪装时间 / 标明列表排序依据 | TEST-E01-07 图与列表互定位 |
| PAT-08 正文就地工具 | S16 → X4 | 保存选区再开工具 | IME 触发错误 / 菜单后备 | TEST-E01-08 组合态、取消返回 |
| PAT-09 预览与执行分离 | S10 → C2 | 预览不推进 runtime | FCP skimmer 优先 / 显式运行命令 | TEST-E01-09 前后 runtime 相等 |
| PAT-10 可恢复布局 | S03/S04/S08 → B1/B3/B4/C4 | 有限分区、个人重置 | 屏幕移除导致窗口不可达 / 单窗默认布局 | TEST-E01-10 小窗、多 DPI 恢复 |
| PAT-11 一致命令、多入口 | S04/S14/S16/S18a → B4/X2/X4/X5 | 统一命令 owner 与启用条件 | 悬停路由抢中文输入 / 真实焦点优先 | TEST-E01-11 跨入口相同事务 |
| PAT-12 不同语义、不同视觉 | S09/S10/S14 → C1/C2/X2 | 编排、时间、控制流分视图 | 节点线条暗示因果 / 类型图例和文字说明 | TEST-E01-12 无图例理解负例 |

## 缺口、替代证据与交付判定

| 缺口 | 本轮替代证据 | 补齐动作与完成标准 |
|---|---|---|
| 四主样本无本轮 GUI 操作 | 16 条 DOC 任务卡和官方链接 | 在合法可用版本以自建夹具逐条复现；保留应用版本、权限、操作输入、前后帧和窗口逻辑尺寸；失败也记录 |
| 无截图前后帧和尺寸 | 本文件不新增 IMG，不借用旧台账图片冒充本轮截图 | 对每个触发留前后帧、帧时间和 hash；不得用一张截图证明保存/撤销 |
| Blender 全文抓取受限且 latest 会漂移 | S04/S05 固定 5.0 官方索引正文；S03 单独注明 5.2 LTS | 固定实际运行版本重新核对；版本差异另记，不回填旧观察 |
| 表单编辑、数值拖动取消、保存失败证据不足 | 文档明确的 Cancel/Undo/显示恢复；未知处逐卡保留 | 为修改、确认、取消、外部变化和权限失败补 RUN；观察缓冲/磁盘/撤销边界，不能用选择卡替代 |
| USER 数据全缺 | 只有机制假设和反例 | 目标作者完成相同任务；记录原始成功/错误/解释，再决定 A/B/C；不预填偏好或效率 |
| 本项目实现尚非本票证据 | 待执行 TEST 标识及 PAT 追踪 | 实施票使用用户批准的公开 Project/core、CLI/RPC、egui 真实事件边界验证；基准 worldline 889febe / worldedit 9bef07c |

可交付结论：已有四主样本各四条适用任务路径、对应取消/失败/恢复记录、五补充机制和 12 个候选模式的反例替代。它们足以支持下一轮原型设计与证据采集，不足以宣称 GUI、目标作者实验或完整编辑/保存闭环已通过。是否以 DOC 加显式缺口批准 EDS-01，由研究评审记录决定；本文件不自行批准、不关闭工单，不把后续实施或 CAP 能力算作交付。
