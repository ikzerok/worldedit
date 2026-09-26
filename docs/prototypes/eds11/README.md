# EDS-11 抛弃式 egui 原型：技术记录

本分支只验证 [worldedit#42](https://github.com/ikzerok/worldedit/issues/42) 的工作台风险。默认构建和默认 Web URL 仍启动现有编辑器；只有 `eds11_prototype` feature 搭配原型入口才显示这个页面。样例不打开用户作品、不保存 Project。J3 的样例语言分析调用 `worldline_core::compile_source`；地图应用、书稿、审阅和演练会话均在页面中标为假数据。

运行方式：

```powershell
cargo run --manifest-path Cargo.toml --features eds11_prototype
$env:NO_COLOR='true'
trunk serve --features eds11_prototype --port 8788 --skip-version-check --disable-address-lookup
# 打开 http://127.0.0.1:8788/?eds11=1
```

2026-09-26 的已复现结果：

- Windows 原生窗口：中文 CJK 字体在安装项目内字体后可读。固定资料 B 后选择入口 P，检查器 owner 仍为 B；入口位置候选从 30 拖到 60，取消后回到 30，Project 写入计数显示 0。写作输入“雾港作者草稿🙂”后切到 J3，出现保留/取消对话框；保留后切回 J2，原草稿仍在。J3 的真实 core 编译样例显示 0 条诊断。
- 浏览器：`trunk serve` 构建并在浏览器实际打开 `?eds11=1`。J2 输入“雾港 Web 草稿🙂”后切 J3，会出现保留草稿对话框；这验证了 canvas 上的点击与键入，而不只是 HTTP 200。Web 版 emoji 字形显示为缺字方框，需补字体覆盖。
- 视口与缩放：浏览器 1024×640 和 700×640，`devicePixelRatio≈1`，canvas 的 `clientWidth×clientHeight` 与视口一致。截图：[1024×640](web-1024.jpg)、[700×640](web-700.jpg)。窄窗截图的文字和操作目标过小，当前设计不满足可用性，需提高 Web 字体/缩放并进一步验证响应式分区。原版 `index.html` 的 1040×660 最小 canvas 约束会导致滚动条，本分支仅对 `?eds11=1` 放宽为 640×480。
- 自动回归：原型局部状态覆盖一次预览/取消/应用/撤销、只读/旧基线/锁层拒绝和作品草稿隔离，`cargo test --features eds11_prototype eds11_prototype::tests` 3 项通过；WASM feature 目标 `cargo check` 通过。所有“应用”只改内存中的假数据，不能视为 Project 写入/撤销的生产验收。

尚未完成的 #42 验收：原生与 Web 的实际中文输入法 composition、键盘焦点环和抽屉退场负例；布局持久化失败注入；浏览器缩放档位；可信的输入到绘制延迟与峰值内存测量。当前没有这些数据，因此不能用本记录关闭 #42，也不应合并到默认界面。
