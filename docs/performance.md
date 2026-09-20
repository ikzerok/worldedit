# worldedit 性能记录

worldedit 默认不记录性能数据。桌面开发或发布候选测试可以设置
`WORLDEDIT_PROFILE_CSV`，例如在 PowerShell 中：

```powershell
$env:WORLDEDIT_PROFILE_CSV = 'D:\Temp\worldedit-frame.csv'
.\target\release\worldedit.exe D:\Temp\d5-workspace
```

输出文件只在进程退出或达到 65,536 条样本时写出，并以 `create_new` 创建，已有文件不会被覆盖。建议把路径放在工程目录之外并为每轮测试使用新文件。写入失败只报告到 stderr，不伪造成功；没有设置环境变量时不创建采样器、不扫描输入，也不产生文件。WASM 构建不启用此入口。

CSV 列为：

`frame_index,cpu_ms,map_active,input_active,pixels_per_point,elapsed_ms`

`cpu_ms` 来自 eframe 公开的 `Frame::info().cpu_usage`，表示上一帧的 CPU 秒数换算为毫秒，包含 `App::update` 和渲染但不包含等待 vsync；首个 `None` 样本跳过，非有限或负值拒绝。其余字段取同一上一帧保存的地图状态、输入事件、像素比例和进程启动后的毫秒数。输入标记覆盖指针移动/按钮、滚轮、缩放、键盘和文本事件，便于后处理筛出交互帧。

这是一项 CPU 帧指标，不能代表 GPU、present 或完整墙钟帧延迟。D5 应使用 release 构建，在固定机器和固定负载下分开记录冷启动与暖态，保存原始样本后离线计算 P50、P95 和最大值；命令行或工具往返耗时不属于帧样本。
