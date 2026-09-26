# EDS-11 命令验收记录

日期：2026-09-27（Asia/Shanghai）
编辑器基线：`20b57e70dbdff3ab7decb047a9220a3488fe967f`，分支 `codex/eds11-egui-prototype`。
配对 core/runtime：`0ba3e241af3a5f5746cfed92e2b5f8ce72909843` 的独立 worktree。
输入：[`input-manifest.json`](input-manifest.json)，其 SHA-256 见 [`SHA256SUMS.txt`](SHA256SUMS.txt)。

## 已验证

- `src/eds11_prototype.rs` 的 8 个 egui 事件回归覆盖 J1 候选预览/取消/应用一次/撤销一次、只读/旧基线/锁层拒绝，J2 作品与任务草稿隔离/切换确认/临时旁查焦点恢复，J3 配对 core 编译与预览/假运行分离，J4 假三方比较不采纳，以及 1024×640、700×640 点视口在 `native_pixels_per_point=1.5` 和 Context zoom=1.25 下的布局存在性。
- `cargo test --locked --features eds11_prototype` 全部通过：102 个 library tests、5 个 authoring_forms、6 个 map_navigation、4 个 network_state、1 个 reading_state，共 118 项。`fmt --check`、`git diff --check`、配对 core 的 native release build 与 `wasm32-unknown-unknown` check 也通过；逐条输出保存在同目录的 `cargo-test.log`、`fmt-check.log`、`diff-check.log`、`native-build-final.log`、`wasm-check.log`。
- Microsoft Edge 153 headless，默认 `devicePixelRatio=1`：完整脚本 [`headless-edge-smoke.js`](headless-edge-smoke.js) 跑完 J1–J4。1024×640 与 700×640 viewport 下，canvas CSS client、backing store 和 ResizeObserver 的 content/devicePixelContentBoxSize 一致；初始与结束 localStorage 均为空。实际事件截图以 `web-*-dpr1.png` 命名；临时侧览截图见 [`web-j2-temporary-reader-dpr1.png`](web-j2-temporary-reader-dpr1.png)。原始命令输出是 [`headless-edge-smoke.log`](headless-edge-smoke.log)。
- 书写测试向隐藏输入元素发送了合成 DOM `CompositionEvent` 序列，另外通过 Playwright 键入提交后的 CJK 字符并检查抽屉关闭后继续输入的路径。它证明的是浏览器合成事件/焦点代码路径，不是操作系统 IME 的真实预编辑。
- J3 画面显示配对 core 的编译样例为 0 条诊断。J3 运行操作、J4 比较数据和所有 J1“应用”均为原型内存假数据；画面状态持续标明 Project 写入数为 0。
- 1024×640、700×640 默认 DPR 截图及 DPR=1.5 模拟截图已保存。截图对应哈希列在 `SHA256SUMS.txt`。

## DPR 模拟观测

独立上下文的 [`edge-dpr-config.json`](edge-dpr-config.json) 将 headless Edge 的 DPR 配为 1.5。[`headless-edge-dpr-observation.log`](headless-edge-dpr-observation.log) 保存了原始浏览器值：`devicePixelRatio=1.5`，viewport、canvas CSS client、canvas bounding rect、canvas backing store、ResizeObserver contentRect/contentBoxSize 和 devicePixelContentBoxSize 都是 1024×640。原始截图是 [`web-1024x640-dpr1_5-emulated.png`](web-1024x640-dpr1_5-emulated.png)。

该驱动的 DPR 变化没有同步到 canvas backing / devicePixelContentBoxSize；截图中的 egui 内容因此放大并裁切。此处只记录 headless emulation 与 canvas 尺寸不一致的风险，不推断显示器物理 DPI，也不代表真实高 DPI 设备失败或通过。

## 原生窗口与未验收项

Release 原生二进制按独立配对 core 构建，日志为 [`native-build-final.log`](native-build-final.log)，窗口进程也曾通过 Win32 启动和读取窗口指标。启用 Per-Monitor V2 后，`GetDpiForWindow` 报告 168，桌面坐标报告 2560×1600；1024×640 物理像素 client 区约为 585×366 个 egui 点，而 1024×640 egui 点窗口需 1792×1120 物理像素。这些是 Windows API 观测值，不是面板物理 DPI 测量。尝试的原生屏幕截图混入了其他前台窗口，已从交付中剔除；因此没有把原生 J1–J4、焦点或输入结果计作验收。

仍待实机/参与者验收：真实 Windows 与浏览器中文 IME 预编辑、确认、候选窗和插入点；无遮挡原生窗口截图及原生 J1–J4；浏览器 chrome 缩放与真实设备 DPR；窄屏键盘可达性；刷新/重启草稿恢复；真实 Project 写入/撤销、布局失败注入；耗时/内存指标和作者参与测试。当前没有用户作品、真实持久化或参与者数据。

## EDS-12 可测试方案

使用全新的虚构作品和记录模板，邀请 5–8 位作者按 native/Web 顺序交叉完成四项任务：J1 固定资料 B、预览入口并取消/应用/撤销，再在只读及旧基线下尝试应用；J2 用中文 IME 输入草稿，打开临时旁查后返回继续输入，再切换作品/任务并恢复草稿；J3 区分 core 诊断、临时预览和明确运行；J4 对照 base/current/proposal 并拒绝默认赢家。每轮记录完整 editor/core SHA、OS 与窗口 DPI API 值、真实屏幕物理分辨率来源、浏览器版本/zoom/DPR、逻辑视口、截图、任务时长、失焦/误操作和草稿恢复结果。通过门槛是取消/只读/陈旧/假操作产生零 Project 写入，J2 无丢字且焦点恢复成功；其他未达项留作下一轮，不把小样本时间当作性能保证。
