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

## 0.15 矢量作者 headless 探针

`dense_scene_headless_release_profile` 为默认忽略的 release-only 回归。使用固定、明确记录身份的含 5000 个 scene 节点的工程，不在测试内临时生成另一份负载：

```sh
WORLDEDIT_DENSE_FIXTURE=/path/to/fixture/world.wl cargo test --release --locked dense_scene_headless_release_profile -- --ignored --nocapture
```

未设环境变量时只查配对工作目录的 `native-fixture/authoring/world.wl`，缺失会明确失败。记录候选 SHA、fixture 文件 hash、机器、DPI 和运行命令。探针用 976×768 地图工作区，先记录加载/冷帧，待纹理就绪后分别测画布平移与对象树滚动，每项三轮、每轮 60 帧，输出 update 与 update+tessellate 的 p50/p95/max。它不包含原生窗口 chrome、GPU 或呈现等待，不能冒称真实原生 FPS，也不替代正式 5000 节点 CPU p95 与至少 20 分钟作者闭环验收。

多图层并发/内存是另一项边界；单层 dense 性能通过不能证明资源安全。`actual_many_layer_renderers_queue_then_report_workset_capacity` 使用 5000 个 renderer 验证实际任务数与容量反馈；取消、切图、实际 native 线程许可、内部隔离表面及上传缓存预算也须通过各自回归。限制与行为见 [矢量作者契约](vector-authoring-0.15.md)。

固定作者负载由纯标准库脚本生成：`python3 scripts/make-world-authoring-fixture.py --output ../native-fixture`。脚本拒绝覆盖非空目录，`--check` 只读比较原字节。输出包含 `fixture-manifest.json`（逐文件 SHA-256、负载数量、生成器 hash）与独立 `machine.json`；负载数据确定性，机器描述不作为内容 hash。主入口在 `authoring/`，默认 1.9 与真正空目录新建用例隔离，防止递归混扫源码。

探针先要求连续三帧纹理 Ready，再采样；每帧测量后按约 60Hz 留出后台执行时间，等待不计 CPU。每轮 update 与 update+tessellate 的 p95 都须≤33ms；通过仍只代表此 headless 指标。独立 5000 层全视口压力图预期触发明确容量错误；浏览模式可临时全部隐藏，再显示少量层确认资源恢复，工程字节应不变。
