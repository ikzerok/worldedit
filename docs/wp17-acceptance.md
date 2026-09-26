# WP-17 全量验收与配对发布

验收日期：2026-09-26。设计需求源共 65 项，本表逐项记录最终状态。

## 最终门证据

- 功能实现基线：worldline 7ea572a801c3eaf935a8f4d48e5dc80c7a60ebbc；worldedit 88de6966be45638682de0b5abaf5412b78c7733b。最终发布 core 在追加 WP-17 Context 后为 fe88d7f1447995e49bb4b54a20a53be0ad0b2db6；最终 editor 文档提交 SHA 由发布记录/tag 在提交后登记。
- paired-check：两仓工作树为空，fmt/test/clippy/原生构建/WASM clippy/WASM 构建全部 exit 0。
- 全量测试：worldline 325 项通过；worldedit 103 项通过；0 失败、0 忽略。
- D5：1,000 对象 / 3,000 关系，release 网络帧 P95=11.073ms，暖态资料切换 P95=4.128ms。
- 发布包：Windows、Web、worldline source、worldedit source 四个 ZIP 均生成并可打开，SHA-256 已登记。
- Windows 实际启动：release worldedit 创建非零窗口句柄；本机 AppliedDPI=168。
- Web 实际加载：Microsoft Edge 153.0.4234.48 headless 请求 index/JS/WASM/icon 均 HTTP 200，进程 exit 0。
- 设计契约：python docs/design/tools/validate_design.py 34 项通过。

## 迁移与负例

- 无清单/1.9 旧工程继续按旧模式；entity/relation 不隐式升级。
- 未知 language_version / unknown required_feature 保持只读，原字节不被覆盖。
- source_config 越界、归档 include、无效集合明确报错，不静默回退。
- 模板缺失/切换不删除正文、自定义字段、别名或未知属性。
- 损坏/重复键 GraphView、未知 GraphView 能力、陈旧 Revision/内容基线均拒绝写入。
- 协作同字段、删改、数组并发和正文并发冲突整批零写入；失锚批注不猜测重绑。

## 工作包证据索引

- **WP-00**：compatibility.json；docs/paired-ci.md；最终 paired-check
- **WP-01**：spec/schemas；validate_design.py 34 项契约检查
- **WP-02**：core Project/workspace_documents 生命周期回归
- **WP-03**：core presentation/map index/纯内容工程回归
- **WP-04**：worldedit 地图矢量/栅格/相机回归
- **WP-05**：worldedit 标记/图层/导航/撤销回归
- **WP-06**：core storage + worldedit archive/save_flow/package 回归
- **WP-07**：M1 集成既有证据 + 本轮全量回归
- **WP-08**：core entity 回归 + editor authoring UI
- **WP-09**：core relations/CLI/RPC 回归 + 关系规模证据
- **WP-10**：network_state + egui network UI + D5 release 性能门
- **WP-11**：core refactor + 删除影响 + egui 重命名/删除回归
- **WP-12**：content_templates + template UI + long_content
- **WP-13**：worldline/docs/wp13-evidence.md + release 回归
- **WP-14**：presentation_presets + preset UI 回归
- **WP-15**：source_scope + relation_context + scope/preset 回归
- **WP-16**：worldline/docs/wp16-evidence.md + collaboration/refactor/UI
- **WP-17**：本验收表 + paired-check + release package + 双端烟测

## 65 项最终状态

| ID | 阶段 | 状态 | 工作包 | 测试 ID | 需求 |
|---|---|---|---|---|---|
| BND-001 | M0 | PASS | WP-00, WP-01 | T-BND-001 | 创作与展示的范围契约 |
| WL-001 | M1 | PASS | WP-03 | T-WL-001 | 统一对象引用 |
| WL-002 | M1 | PASS | WP-02 | T-WL-002 | 多类型编辑缓冲 |
| WL-003 | M1 | PASS | WP-02, WP-03 | T-WL-003 | 不可变工作区快照 |
| WL-004 | M1 | PASS | WP-02, WP-06 | T-WL-004 | 带前置条件的编辑事务 |
| WL-005 | M1 | PASS | WP-02, WP-06 | T-WL-005 | 外部变化与三方冲突保护 |
| WL-006 | M1 | PASS | WP-06 | T-WL-006 | 保存失败与恢复 |
| WL-007 | M1 | PASS | WP-03 | T-WL-007 | 地图文档契约 |
| WL-008 | M1 | PASS | WP-03, WP-11 | T-WL-008 | 跨视图引用与反查 |
| WL-009 | M1 | PASS | WP-06 | T-WL-009 | 工作区包完整性 |
| WL-010 | M1 | PASS | WP-03 | T-WL-010 | 诊断分域 |
| WL-011 | M1 | PASS | WP-03, WP-05 | T-WL-011 | 运行指纹隔离 |
| WL-012 | M1 | PASS | WP-01, WP-02 | T-WL-012 | 格式版本与能力协商 |
| WL-013 | M2 | PASS | WP-09 | T-WL-013 | 可扩展关系类型 |
| WL-014 | M2 | PASS | WP-09 | T-WL-014 | 一等语义关系 |
| WL-015 | M2 | PASS | WP-09 | T-WL-015 | 旧人物关系兼容 |
| WL-016 | M2 | PASS | WP-09 | T-WL-016 | 局部关系查询 |
| WL-017 | M2 | PASS | WP-08 | T-WL-017 | 通用内容实体 |
| WL-018 | M3 | PASS | WP-12 | T-WL-018 | 可选模板与校验 |
| WL-019 | M3 | PASS | WP-12, WP-13 | T-WL-019 | 自由正文与未知内容 |
| WL-020 | M4 | PASS | WP-15 | T-WL-020 | 作者范围与专题选择 |
| WL-021 | M3 | PASS | WP-12 | T-WL-021 | 出处、陈述性质与创作状态 |
| WL-022 | M3 | PASS | WP-12 | T-WL-022 | 历史、故事与锚点关联 |
| WL-023 | M4 | PASS | WP-16 | T-WL-023 | 持久批注锚定 |
| WL-024 | M4 | PASS | WP-16 | T-WL-024 | 修改提案与语义差异 |
| WL-025 | M4 | PASS | WP-15 | T-WL-025 | 草稿与活动源码配置 |
| WL-026 | M2 | PASS | WP-09 | T-WL-026 | 命令行工作区检查和查询 |
| WL-027 | M2 | PASS | WP-11 | T-WL-027 | 跨文件重命名与删除方案 |
| WL-028 | M1 | PASS | WP-03, WP-06 | T-WL-028 | 纯内容工程合法性 |
| WL-029 | M1 | PASS | WP-03, WP-04 | T-WL-029 | 线面与导航的格式契约 |
| WE-001 | M1 | PASS | WP-05 | T-WE-001 | 地图工作区入口 |
| WE-002 | M1 | PASS | WP-04 | T-WE-002 | 受限栅格图层加载 |
| WE-003 | M1 | PASS | WP-04 | T-WE-003 | 地图浏览镜头 |
| WE-004 | M1 | PASS | WP-05 | T-WE-004 | 点标记与资料入口 |
| WE-005 | M1 | PASS | WP-05 | T-WE-005 | 浏览和编辑展示分离 |
| WE-006 | M1 | PASS | WP-05 | T-WE-006 | 拖拽预览与撤销 |
| WE-007 | M1 | PASS | WP-05 | T-WE-007 | 图层、锁定与图例 |
| WE-008 | M1 | PASS | WP-05 | T-WE-008 | 搜索、定位与反查 |
| WE-009 | M2 | PASS | WP-10 | T-WE-009 | 子地图与返回路径 |
| WE-010 | M2 | PASS | WP-10 | T-WE-010 | 跨类型局部关系图 |
| WE-011 | M2 | PASS | WP-10 | T-WE-011 | 关系编辑器 |
| WE-012 | M2 | PASS | WP-10 | T-WE-012 | 图布局与共享专题 |
| WE-013 | M2 | PASS | WP-10 | T-WE-013 | 连接来源可辨识 |
| WE-014 | M1 | PASS | WP-05, WP-10 | T-WE-014 | 统一导航与资料侧栏 |
| WE-015 | M2 | PASS | WP-10 | T-WE-015 | 通用内容创作页面 |
| WE-016 | M3 | PASS | WP-12 | T-WE-016 | 模板栏目与问题引导 |
| WE-017 | M3 | PASS | WP-12 | T-WE-017 | 不确定、非地理内容展示 |
| WE-018 | M1 | PASS | WP-04, WP-05 | T-WE-018 | 基础线面绘制 |
| WE-019 | M4 | PASS | WP-14, WP-15 | T-WE-019 | 专题/时期/章节预设 |
| WE-020 | M4 | PASS | WP-16 | T-WE-020 | 对象和地图讨论 |
| WE-021 | M4 | PASS | WP-16 | T-WE-021 | 审阅面板 |
| WE-022 | M1 | PASS | WP-05, WP-06 | T-WE-022 | 局部故障和冲突界面 |
| WE-023 | M1 | PASS | WP-06 | T-WE-023 | 桌面/浏览器同格式保存 |
| WE-024 | M3 | PASS | WP-12 | T-WE-024 | 历史与故事互访 |
| WE-025 | M2 | PASS | WP-11 | T-WE-025 | 引用影响面板 |
| WE-026 | M1 | PASS | WP-05 | T-WE-026 | 键盘与低视觉负担 |
| Q-001 | M0 | PASS | WP-00, WP-07, WP-17 | T-Q-001 | 旧工程回归基线 |
| Q-002 | M1 | PASS | WP-05, WP-07, WP-10, WP-17 | T-Q-002 | 非污染性质测试 |
| Q-003 | M1 | PASS | WP-01, WP-03, WP-07 | T-Q-003 | 契约与往返测试 |
| Q-004 | M1 | PASS | WP-04, WP-06, WP-07 | T-Q-004 | 不可信工程与资源边界 |
| Q-005 | M2 | PASS | WP-09, WP-10, WP-13 | T-Q-005 | 明确性能预算 |
| Q-006 | M1 | PASS | WP-06, WP-07, WP-17 | T-Q-006 | 跨平台与异常验证 |
| Q-007 | M0 | PASS | WP-00, WP-17 | T-Q-007 | 配对提交的持续集成 |
| Q-008 | M3 | PASS | WP-13 | T-Q-008 | 长文与模板保真 |
| Q-009 | M4 | PASS | WP-16, WP-17 | T-Q-009 | 协作冲突与恢复验证 |

## 平台范围

Windows 桌面和本机 Edge/WASM 做了实际启动/加载烟测；Cargo 同时验证原生与 wasm32。
没有在本轮实际运行 macOS/Linux，因此不把这些平台写成实测通过。

完整日志和本机原始证据保留在组合目录 target/ticket-completion-20260926/、
worldedit/target/paired-check/ 与 releases/；这些生成物不进入源码提交。
