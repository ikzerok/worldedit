# worldedit 图标

图标采用青蓝色螺旋分支，背景为不透明的深石墨蓝。

上升螺旋表示世界线，多层分支与交织表示故事的分化与关联，圆形端点表示事件。背景与深色 GUI 配套，无金属材质或立体底座。

- `worldedit-icon-master.png`：最终带背景的原始设计，1254 × 1254。
- `worldedit-icon.png`：256 × 256，嵌入编辑器窗口。
- `worldedit.ico`：Windows 图标包，包含 16、24、32、48、64、128、256 像素尺寸，由 Windows 构建嵌入 `worldedit.exe`，也可用于快捷方式与发行打包。

PNG 和 ICO 均由同一原图缩放导出，七个尺寸均检查为完全不透明。图标使用时保持比例，避免裁切事件节点。窗口使用 PNG；自 0.36.1 起，Windows 构建由 `build.rs` 将既有 ICO 的七份图像及图标组写入标准资源并链接到 `worldedit.exe`。无需把 ICO 另放在程序旁边才能显示 EXE 图标；发行包仍保留资产文件。

MSVC 直接链接生成的 `.res`，不新增图标生成依赖或构建 crate。Windows GNU 构建需要 `windres`，可用 `WORLDEDIT_WINDRES` 指定其可执行文件。资源仅链接 worldedit，随包 `wl.exe`、`wl-agent.exe` 保持控制台工具身份。

在已构建的 Windows 产物上，可运行 `python scripts/verify-windows-app.py path/to/worldedit.exe --icon assets/worldedit.ico --cli path/to/wl.exe`，核对 GUI/控制台子系统和七尺寸图标原字节。该命令只做 PE 静态核验，不证明资源管理器、任务栏或真实窗口已经实测；版本验收边界见[0.36.1 说明](../docs/releases/v0.36.1.md)。
