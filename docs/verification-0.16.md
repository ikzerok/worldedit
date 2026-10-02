# 0.16 验证记录

本文件记录候选验证层级；最终公开发布需要另行确认。当前处于开发验收，不把 draft CI 当作固定 pair 的最终结论。

## 必需门禁

- 两仓格式与 Rust 源文件不超过600行
- worldline workspace 全量测试、全目标严格 Clippy、release构建
- worldedit 普通/原型全量测试、native/WASM严格 Clippy与构建
- compatibility.json 固定完整 core SHA，最终同一pair检查、打包与回读
- 源码生命周期正向/失败/取消/撤销/保存重开/存档兼容、typed dash保真/预算/能力保护、布局与公开内容白名单回归

## 已知测试层级

原生 Rust 测试包括 core/CLI/agent/runtime真实执行；egui注入事件测试是真实UI代码回归，但不等同桌面鼠标键盘验收。resvg像素比较可证明指定渲染尺度与输出保真，不等同物理高DPI屏幕。Node/DOM与WASM模拟如被执行将单列，不声明为浏览器测试。

实际桌面仅 dot Linux 隔离工程与独立配置窗口，app-bound截图。Windows运行测试依靠最终CI日志核验，Windows/macOS GUI、物理输入法、读屏、高DPI及真实file://浏览器必须实际执行才报告通过；没有验证的范围保持未验。

本地因磁盘容量限制设置 CARGO_INCREMENTAL=0、CARGO_PROFILE_DEV_DEBUG=0、CARGO_PROFILE_TEST_DEBUG=0；测试断言与debug优化层级不变，仅去DWARF符号/增量缓存，release配置未变。只清理可再生incremental缓存，原始证据和native候选保留。
