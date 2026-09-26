# CAP-QA 第一阶段验收记录

本记录跟踪 worldedit #30 第一阶段，覆盖 CAP-01、03、04、05、06 的当前源码、跨仓配对、测试入口和自动化边界。验收以隔离工作树的提交为准；后续补丁必须重跑受影响的项目。此记录不关闭任何功能票。

## 冻结输入

| 项 | 值 |
|---|---|
| worldedit | `479b1b7ab24159e68dd59ed7aface6cf7e786883`，tree `f606f4b569e00eba587ae8bbdf713a84c9912359` |
| worldline 配对 | `c64a7899a3887b07ad6a68cdd8cfc67541c59e0e`，tree `0a83be095fe4699c4cc7f2a8af9cf81d407c181c`；包含 WASM 时钟修复 |
| `compatibility.json` | 本次 QA 工作树临时指向上述完整 core SHA；manifest 与依赖更新后的 editor `Cargo.lock` 不包含在本报告提交中。主线已另行 cherry-pick 同一修复为 `6398e03` |
| Cargo.lock SHA-256 | QA 配对 editor `C8AD8E9D8D6A463F61FFF4ABBAB1A84F5E18DC9D1CA882A7FA005ED067562313`；worldline `4366B4C126FBECBEC79B614BF8A2C8C45CF75CDB3E574AFD7A52FCBC342CE524` |
| 环境快照 | [`environment.txt`](../../../qa-capqa-stage1/evidence/environment.txt)；Windows 11 x64 MSVC，Rust/Cargo 1.98.0，WASM target、Trunk 与 headless Edge 已安装 |
| issue 输入 | [`evidence/`](../../../qa-capqa-stage1/evidence/) 下保存 worldedit #13/#14/#16/#17/#18/#19/#20/#30 和 worldline #13/#15/#16/#17/#18/#19 的原始 JSON；各文件 SHA 在 `ticket-snapshots.sha256` |

## 覆盖矩阵

| 功能 / ticket | editor 实现提交 / 当时配对 SHA | 当前 spec 与 core 覆盖 | CLI / RPC 覆盖 | egui 真实事件回归 | 失败与未闭环项 |
|---|---|---|---|---|---|
| CAP-01B #13 源码引用/建档 | `e5eb266` / `89efd75221684df2ff7cb6410e7abdcc8c5b0a17` | `spec/workspace.md`、`spec/syntax.md`、`spec/agent-protocol.md`；`core/tests/authoring_intents.rs` 覆盖同名对象、显式目标、取消/陈旧基线、跨文件撤销与未知能力 | `cli/tests/cli.rs::authoring_intent_cli_preview_is_read_only_and_apply_is_all_or_nothing`；`agent/tests/protocol.rs::authoring_intent_rpc_preview_and_apply_are_atomic` | `src/app/authoring_ui_tests.rs` 中候选键盘提交、Esc、IME 后提交、刷新冲突保留草稿、选中文本建档/应用/取消、链接返回光标 | 自动回归有覆盖；操作系统级 IME、真实参与者和发布版桌面手工路径未验收 |
| CAP-01C #14 地图位置原子建档 | `50e84ec` / `89efd75221684df2ff7cb6410e7abdcc8c5b0a17` | `spec/workspace.md`、`spec/agent-protocol.md`；`core/tests/authoring_intents.rs` 与 `core/tests/map_creation.rs` 覆盖地图+对象事务、无效/陈旧零写入、未知字段保留 | 经 CAP-01A 的 CLI/RPC 原子意图入口；未找到独立地图创建 CLI/RPC 方法 | UI map/place 创建与撤销用 `egui::Context::run` 事件测试覆盖；见后续实际日志 | 原生/Web 包内目录选择与真实工程持久化仍需实机复验 |
| CAP-03B #16 目录查询 | `dde29ed` / `163e0ff77c4f86543fef5fdce2d4fbb0fad2df11` | `spec/catalog.md`、`spec/agent-protocol.md`；`core/tests/catalog_queries.rs` 覆盖空集、组合/否定、未知类型、稳定分页、陈旧 cursor、预算与取消、只读待办 | `cli/tests/cli.rs::catalog_query_cli_uses_core_query_dto_and_snapshot_cursor` 和 validation/budget 负例；`agent/tests/protocol.rs` stale cursor/read-only 边界 | 打开查询、组合/移除条件、空结果、陈旧页、共享定义保存/本地收藏隔离、待办定位 | D5 忽略性能 profile 单独执行；多人协作和真实大型用户工程尚未验收 |
| CAP-04B #17 提案差异审阅 | `85a19f0` / `163e0ff77c4f86543fef5fdce2d4fbb0fad2df11` | `spec/collaboration.md`、`spec/agent-protocol.md`；`core/tests/collaboration.rs` 覆盖三方冲突、UTF-8/CRLF 段落范围、预算、原始回退、引用影响与零写入 | 此 Project 提案审阅/应用功能没有对应 CLI/RPC 方法；协议清单中不得把 runtime replay RPC 算作提案 API | 审阅/批注/冲突保留和 stale guard 由 app 测试事件驱动；见后续实际日志 | 手工键盘/复制/IME 与高负荷冲突修复体验未验收 |
| CAP-04D #18 检查点历史 | `689a3a7` / 当时 `ad2edff`；最新 editor 配对见下 | `spec/workspace.md` 检查点契约；`core/tests/checkpoints.rs` 覆盖快照字节、影响预览、撤销、陈旧/外部冲突、限额、损坏记录和只读目标；CAP08C `646ee53` 按浏览器会话隔离 `/world`；WASM-safe 时钟补丁 `c64a789` | 没有 Project 检查点 CLI/RPC API；`session.checkpoint` 是 runtime 演练会话能力，不等同工程历史 | `checkpoint_history_*` egui 事件回归覆盖预览、取消、确认、stale、窄窗、配额失败、删除与不可用记录；headless Edge 实测打开历史并创建标签 `capqa-clock`，UI 显示非零 UTC 时间戳，0 console/page errors | CAP08C 已修复共享 `/world` 的串读/删除风险。`browser_checkpoint_history_isolated_by_session_at_a_shared_mount_root` 在 native 运行；`same_saved_session_keeps_history_but_a_new_project_at_world_cannot_restore_it` 仅 `wasm32` cfg，本机 locked test 不运行。边界必须区分：同一 WASM 模块内关闭/重开 Project 可沿用 session id；整页刷新/浏览器重启会清空 `thread_local` 历史。后者目前不持久，符合 `spec/workspace.md` 当前承诺，却不满足 #18 明写的 Web 重启流程，需先定规格/实现范围。checkpoint 预览是文件/对象影响投影，不是三方文本差异；Web 配额/持久化失败后的救援流程仍未验收 |
| CAP-05B #19 模板管理 | `ece8c22` / `c03016be02cdb6716fe6eacb2bdc8a27759fad91` | `spec/templates.md`、`spec/workspace.md`、`spec/syntax.md`；`core/tests/project_templates.rs` 覆盖坏文档隔离、替换影响/陈旧基线、实例不变、未知字段、只读能力、字段验证、object_ref 重命名/删除保护 | 没有模板管理 CLI/RPC 方法；模板契约由公开 Project API 提供 | 模板浏览、替换预览/确认、按类型表单、只读、未知字段、陈旧表单均由 egui 事件覆盖 | 带真实 ZIP/目录工程的浏览器导入和跨刷新保存重开未由本轮用户工程验收 |
| CAP-06B #20 轨迹调试 | `3e02fa7` / `c03016be02cdb6716fe6eacb2bdc8a27759fad91` | `spec/replay.md`、`spec/agent-protocol.md`；`runtime/tests/replay.rs` 覆盖重放/纯解释/陈旧选择/预算与取消；core 负责读取语言快照 | `cli/tests/cli.rs::play_can_write_a_seeded_trace_that_cli_replays`；`agent/tests/protocol.rs::replay_rpc_exposes_trace_checkpoint_and_pure_choice_explanations` 及 DTO/故事错误区分 | 轨迹重放、条件解释、旧选择失配定位、取消长回放、窄视图、超限导入与访问覆盖均用 egui 事件测试 | 真实长故事用户输入、真实浏览器下载/导入和发布包跨重启需补测 |

### 完整实现与配对 SHA

下表中的“历史配对”取 editor 功能提交自身的 `compatibility.json`。本轮隔离 QA 冻结测试配对为 worldedit `479b1b7ab24159e68dd59ed7aface6cf7e786883` + worldline `c64a7899a3887b07ad6a68cdd8cfc67541c59e0e`；editor compatibility/lock 在该工作树临时更新后运行。主线随后将同一 core 改动 cherry-pick 为 `6398e03` 并更新 editor compatibility/lock；不要把本轮隔离 SHA 与主线 SHA 混写。

| ticket | editor 实现 SHA | worldline 对应能力 SHA | editor 提交当时配对 SHA |
|---|---|---|---|
| #13 CAP-01B | `e5eb26650b30ef86e20d71825aaca656e825c799` | `b7f489b27b6469e4d076f1a98c727c62203fbb6d`；CLI/RPC 扩展 `e5f563be6de6c45f1214dedaa8a9053252e8499e` | `89efd75221684df2ff7cb6410e7abdcc8c5b0a17` |
| #14 CAP-01C | `50e84ecf3d5301db3e280b6e880440e22018f5d3` | 复用 CAP-01A Project 原子意图 API `89efd75221684df2ff7cb6410e7abdcc8c5b0a17` | `89efd75221684df2ff7cb6410e7abdcc8c5b0a17` |
| #16 CAP-03B | `dde29ed99183ba59f8731f27dfbccf1bcdd42cf5` | CAP-03A `25f44363a99d4ef2f56d96222f0d548f0c15cdd6` | `163e0ff77c4f86543fef5fdce2d4fbb0fad2df11` |
| #17 CAP-04B | `85a19f0b31ab577b9789d1ac9487ccaf3570ab5c` | CAP-04A `163e0ff77c4f86543fef5fdce2d4fbb0fad2df11` | `163e0ff77c4f86543fef5fdce2d4fbb0fad2df11` |
| #18 CAP-04D | `689a3a7c2cbddccf86c0efdc75df6907ee28bf8e` | CAP-04C store `c24625b6caec2a9580e2d2ecd600a5b77fac3b4b`；CAP08C isolation `646ee53e64c48a7fcbc22d6e9a8037a74743c467`；WASM clock `c64a7899a3887b07ad6a68cdd8cfc67541c59e0e` | `ad2edfffc9225192ae751ccc96a5c9bd80c66dd4` |
| #19 CAP-05B | `ece8c22a571c13b2a68622105eb358e1ef5acdd9` | CAP-05A templates/object refs `aeeab1b6aa6ce4e297010effdf81076888faa3d6`, follow-ups `2ab7b94c3e742459ad8f13bd47f9443c3442b553` / `4195a15641f5bf98dcb30dc7213e3d33f74b0951` | `c03016be02cdb6716fe6eacb2bdc8a27759fad91` |
| #20 CAP-06B | `3e02fa7edd11159dd787984aa0246ac4dac7c6b7` | CAP-06A replay `55c7018d9c8989e9857f193c1e67fe39e438bd17`, CLI/RPC `cb0a7811d484c8d3a1cda38b4386b1e725634507`, boundary fix `661ca3953c6edd249007fd85e5a70ade0de2a731` | `c03016be02cdb6716fe6eacb2bdc8a27759fad91` |

## 当前命令与证据

成对检查使用仓库脚本：`worldedit/scripts/check-pair.ps1`。它在执行前核对 `compatibility.json`，分别对两仓运行 fmt、locked 全量测试、严格 Clippy 和 native build，再对 editor 运行 WASM Clippy/build。原始进程输出与脚本逐项日志保存在 [`evidence/`](../../../qa-capqa-stage1/evidence/)。发布包另用 `worldedit/scripts/package.ps1 -OutputDirectory <evidence>/packages`，包含 Windows 与 Web release 包；构建包不等同人工桌面/Web 验收。

| 检查 | 状态 | 原始证据 |
|---|---|---|
| worldline fmt/test/clippy/native build | 通过；workspace 451 passed、0 failed、1 ignored | `paired-check-20260926T1527357244766Z/worldline-*.log` |
| worldedit fmt/test/clippy/native build | 通过；171 passed、0 failed、0 ignored | 同一 paired-check 目录下 `worldedit-*.log` |
| worldedit WASM clippy/build | 通过；locked WASM checks/build 结束无 error | 同一 paired-check 目录下 `worldedit-wasm-*.log` |
| release Windows/Web package | 通过；四个 ZIP 内容完整，未包含 `target`、`dist`、`releases` 或 `docs/planning` | `package-audit-clock.log`、`packages/worldedit-20260926-232943/SHA256SUMS.txt` |
| Edge headless 加载与检查点创建 | 通过；Edge 153.0.4234.48，1280×720 WASM canvas，创建记录显示 `2026-09-26 15:42:10 UTC`，0 page/console errors | `browser-startup-red.log`、`browser-smoke-final.log`、`browser-checkpoint-final.log` 和对应截图 |
| D5 查询 profile（1000 objects / 3000 relations） | 本切片未运行，后续单独补充 | 尚无 `d5-profile.log` |

### Web 时钟缺陷的红绿证据

旧配对 `479b1b7 + 646ee53` 在 headless Edge 中未能挂载。WASM panic 指向 `std::time::SystemTime::now`，调用链由 checkpoint session/id 初始化进入 `Project::new`；另一个相同调用用于创建 manifest 时间。core 修复 `c64a7899a3887b07ad6a68cdd8cfc67541c59e0e` 使用 `web-time`，并增加 native 唯一 ID/非零时间戳和 WASM manifest 时间断言。修复后 release Web 包正常启动；通过画布鼠标/键盘事件创建检查点，记录详情显示 UTC 时间。原始红/绿日志和截图保存在本地外部 `qa-capqa-stage1/evidence/`，不随本次 editor 文档/脚本提交入仓。

可复现入口：在配对后的 editor 根目录运行 `pwsh -NoProfile -File .\scripts\package.ps1 -OutputDirectory ..\evidence\packages`；再以新生成包内的 `web` 目录启动 `python -m http.server 18765 --bind 127.0.0.1 --directory <package>\web`。运行 `playwright-cli -s=capqa-stage1 open http://127.0.0.1:18765/ --browser msedge`，随后执行 `playwright-cli -s=capqa-stage1 run-code --filename='docs/qa/capqa-browser-smoke.cjs'` 与 `playwright-cli -s=capqa-stage1 run-code --filename='docs/qa/capqa-browser-checkpoint.cjs'`。环境、Edge/Playwright 版本、原始运行日志及截图在外部 `qa-capqa-stage1/evidence/`。harness 使用固定 1280×720 画布坐标，可自动重放；这不等同不同 DPI 的桌面人工验收。

本阶段没有真实参与者或手工桌面会话记录。egui 事件测试和 Edge headless 交互是自动化证据，不是不同 DPI 的桌面人工验收。原始日志/截图与包文件在 `D:\Desktop\Code\work-worldline\qa-capqa-stage1\evidence\`，本地工作区可共享，但不随本次 editor 文档/脚本提交入仓；本报告保留了日志目录和包 SHA-256。CAP07、读者导出、发布 CI、不同 DPI 桌面完整会话、D5 release profile 及项目隔离回归合并后的复验留在后续阶段；在这些门未闭环前，worldedit #30 与尚未完成人工验收的子票保持 OPEN。
