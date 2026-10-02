# worldedit 0.15 验证事实摘要（开发候选）

核对时间：2026-10-02 17:40 UTC。本文区分自动回归、headless 测量与真实平台交互。本轮 Linux 自动检查、对应源码的 Windows 配对 CI、21 分钟原生功能会话及修补构建的局部验证已完成。真实浏览器验收受阻，产品仍为未正式发行的开发候选，不能据此宣布 0.15 整体验收完成。功能见 [CHANGELOG](../CHANGELOG.md)，边界见[矢量作者](vector-authoring-0.15.md)、[读者发布](reader-publishing-0.15.md)和[后台计算](web-worker.md)。

本文指向 worldline 的跨仓相对链接仅用于 `worldline` / `worldedit` 同级检出；Windows CI 链接为对应 GitHub 运行。

## 已完成的自动检查

| 检查 | 已核实结果 | 适用范围 |
| --- | --- | --- |
| Linux 完整测试 | 552 通过，0 失败，3 ignored | 535 单元测试加 5 / 6 / 4 / 2 个集成测试；包含 reader 输入保留、Windows 测试期望修正及单 tooltip 修复 |
| 格式、native 严格 Clippy | 通过 | 当前冻结候选 |
| WASM 严格 Clippy | 通过 | 编译/静态检查，不是浏览器 Worker 执行 |
| Linux native release | 构建成功 | 当前冻结候选，产物身份见下文 |
| Trunk release | 构建成功 | Trunk 0.21.14，当前冻结候选，产物身份见下文 |
| 静态 Web 包核对 | 入口、JS、WASM、worker 和图标闭包检查通过；SRI / JS 语法 / WASM 导出已检查 | 尚未执行真实页面加载 |
| Node WebAssembly 烟测 | typed SVG 预览、曲线保留、320000 字节渲染结果、不安全 SVG 拒绝通过 | 无 DOM 的 Node 环境；不是浏览器 Worker |
| 可选 `eds11_prototype` 矩阵 | 567 测试通过、0 失败、4 ignored；native/WASM Clippy 和 debug 构建通过 | 与默认套件重复的用例不相加，亦不把可选原型视为默认启用 |
| Windows exact 配对 CI | 默认 553 通过、0 失败、3 ignored；prototype 568 通过、0 失败、4 ignored | 以下 `09e826…` / run `37038196976`；两种配置的 native/WASM Clippy 及 debug 构建全部成功 |

最后一处跨平台 binding 微修后，以上普通完整测试 552/0/3 和 prototype 完整矩阵 567/0/4 已再次执行通过，本地配对 core 为合并后的 `ed77d092abe72b6913dfac98ff71b37ca5cc57c7`；两种配置的 unit 数分别为 535 / 550，各加 17 个 integration 测试，不把重复项相加。该微修仅让 Unix 的 `DirBuilder` 保持 mutable、其余平台使用 immutable，保留 Unix mode(0700) 与 create 顺序。reader 定向回归另为 48 通过、0 失败、1 ignored，普通与 prototype 的 native/WASM 严格 Clippy 均通过；从 Linux host 对官方 `x86_64-pc-windows-msvc` 标准库执行两种 feature 配置的 all-targets cross-Clippy 也通过。交叉检查属于静态验证，不替代真正 Windows CI 运行。

配对 core 的 Windows 检查已完成，准确身份和结果见[worldline 验证摘要](../../worldline/docs/verification-0.15.md)及 [run 37030107038](https://github.com/ikzerok/worldline/actions/runs/37030107038)。本轮也保留两个已修复的 Windows 失败，避免只记录最终成功：编辑器旧 head `3d9565d2ab05d22ca0533203f9e5db48bbe8f371` 固定配对 core `90000754940c9b8630d294732912c9a9daea79d3`；[PR run 37030576559 / job 110916065920](https://github.com/ikzerok/worldedit/actions/runs/37030576559/job/110916065920) 实际测试合并提交 `997774ed4a97c3470ed3e6cb21e63757e80271b7`，当时失败。

失败位于 `completed_atomic_commit_is_not_reported_as_cancelled`：完成回执返回成功路径，但测试把 Windows 规范化的带 `\\?\` 前缀路径与原生短写路径直接比较，断言不等。该轮编辑器单元测试 535 通过、1 失败、3 ignored；脚本失败即停，后续步骤未执行。测试改为文件成功创建后与 `target.canonicalize()` 比较，保留成功回执、完成态、取消提交点、文件内容和暂存数量断言，实际路径安全逻辑未变。

修补 head 为 `3f79c268aad6faa3294d634168ada5e234de9ea1`，配对 core 不变；改动限于单 tooltip 及相关测试、上述 Windows 测试期望。[PR run 37034314932 / job 110928620570](https://github.com/ikzerok/worldedit/actions/runs/37034314932/job/110928620570) 实际测试合并提交 `82fe63ddf6df1e3a5d1cfeeccbf700e1eadf3a77`，Windows 测试为 553 通过、0 失败、3 ignored（536 单元测试及 17 个集成测试），原 canonical 路径测试明确通过。

该运行当时仍为 failure：随后 native 严格 Clippy 在 `browser_preview.rs` 的 `DirBuilder` 声明报告 `unused_mut`，因其可变操作仅在 Unix 分支存在。后续 native 构建、WASM 和 prototype 配对步骤当时没有执行；随后用局部 cfg binding 修正，未关闭警告或改变目录权限与路径安全。

最终受测源码 head 为 `09e826f9d9cf58b05b358b10f3cfa5fce10092b1`，固定配对 core `90000754940c9b8630d294732912c9a9daea79d3`；[PR run 37038196976 / job 110941498056](https://github.com/ikzerok/worldedit/actions/runs/37038196976/job/110941498056) 的实际合并提交为 `726682ca5a928051801c2c54de95e072719efadb`，最终 success。Windows 默认测试为 536 unit + 17 integration，prototype 为 551 unit + 17 integration；原路径测试在两套中均通过，native/WASM 严格 Clippy 与 debug 构建全部完成。环境为 Windows Server 2025，镜像 `windows-2025-vs2026` / `20260925.250.1`，Rust `1.98.0 (88d9e12ae 2026-08-18)`。CI 没有启动 GUI。

最终配对记录固定 core main `ed77d092abe72b6913dfac98ff71b37ca5cc57c7`，与上述受测 `9000075…` 的语言源码一致，差异仅三份正式文档；[core main CI 37039387318](https://github.com/ikzerok/worldline/actions/runs/37039387318) 也已成功。本页 Windows 结果精确对应上述 `09e826…` 的实际测试。配对脚本离线测试 8/8、发行门禁脚本离线测试 67/67 已重验；后者使用 mock，没有创建 Release。

本轮 21 分钟 Linux native 功能会话使用的二进制 SHA-256 为 `8a28da45936280c4b54730e7390aaea109b4bc44fddaa10a98dc05df4293f1a2`。随后只修改 Windows 测试的路径期望及回执的重复 tooltip，重跑上述完整 Linux 门禁；新 native SHA-256 为 `ba63c7a99455c09ea659587599ccdbb1645dc16335028b39d0f455e4ad257a63`。新二进制的局部悬停与 CPU 采样结果单列如下，不宣称它已重新完成 21 分钟操作。

最后一处跨平台 binding 微修后的默认 native release SHA-256 为 `030cbc353d3d5fafb25be6e34b0812106c4c4e246d100667df7d22fec3b1bf29`；微修没有改变 Unix 分支的权限或调用顺序。这个二进制已实际启动，原工程 `a_workflow` 渲染正常、全部文件显示已保存；发布档案 `harbor · qa15` 成功载入 10 项选择，取消后于 17:17 UTC 通过系统 Alt+F4 正常退出，日志为空。下文功能时长和原生 CSV 仍精确绑定各自受测 hash，不把它们重新归为这个二进制的实测值。

Web WASM SHA-256 为 `21c718b83ebeddafe3c000cd9602fea8624f21157b3eeb4df3522c1e943e6935`；最后一处 cfg 微修后再次 Trunk 构建成功，WASM 与此前单 tooltip 修补包逐字节相同，静态核对及 Node 烟测也已重跑通过。Node 输出含 320000 字节 RGBA、390 个可见像素和 19 条进度消息，证据仍限于无 DOM 的 WASM 调用。

关键自动回归覆盖：

- 地图操作使用 core typed 请求和事务；曲线/控制点、组与变换、选择导出、legacy 共存、撤销/重做、失败保留稿件
- 后台任务检查任务类型、基线、会话代次、原请求与结果；快照活动文件和墓碑在复制前计预算，畸形、超额和陈旧结果不能提交
- 实际渲染线程许可、最多两个并发任务、RGBA/内部合成工作集预算、多图层容量提示、取消和缓存清理；单层性能通过不替代这些资源回归
- reader profile 后台规划、完整 core 重验、history 与缓存更新；失效选择和未知能力保留，明确授权不因刷新被悄悄裁剪
- 实际 `App::update` / `ViewportEvent::Close` 事件路径，任务运行中和 Done 已排队时关闭、发布窗口 X、清理等待、通道中断、提交点与重复请求
- reader 取消/X 后保留同一工作区的选择、profile ID、标题和查询；重开恢复输入，旧审核和旧任务结果失效
- ZIP 安全路径、字节预算、分块取消、工作区外暂存和无覆盖公布；错误/取消不改旧目标，不伪报交付成功

自动 egui 输入事件能够验证状态和控件行为，但不能代替物理 IME、屏幕阅读器、真实文件选择器或操作系统窗口验收。

## 原生功能验证与局部修复

Linux native `8a28da45…` 的本轮实际作者操作从 15:57:15 持续至 16:18:18 UTC，共 21 分 03 秒；期间连续进行操作，没有用空置等待补足时长。已经实际观察：

- 读者发布取消后重开，保留 profile ID `qa15`、标题 `harbor` 和 10 项选择，同时清除旧审核状态；本次没有输入查询文本
- 后台保存 profile 后先显示未保存状态；显式保存后生成 `.world/reader-profiles/qa15.json`，磁盘文件中 10 条 routes 可读取
- 前一候选曾完成地点新建与绑定的一次原子撤销/重做；最终候选重开后地点 `pier` 仍在，磁盘 `world.wl` 与地图 JSON 的 `target_ref` 一致。此处只把重开与持久化核验归于最终候选
- 单层 5000 节点地图完成真实缩放；5000 层压力图明确显示资源不足、未完整显示，没有把静默漏层记作成功
- 压力图临时隐藏全部图层后，队列和问题数均为 0；仅显示一层后恢复。`b_dense` 和 `c_layers_pressure` 两张地图文件的 SHA-256 与固定 fixture 一致，浏览操作没有改写工程
- 最终生成/完成竞态中取消，再次打开后保留 10 个对象、地图选择、3 章、2 个附件和静态故事说明授权；重新生成得到 16 个公开项、36 个文件、407879 原始字节和 38434 字节 ZIP
- 尝试已有 ZIP 目标先显示“目标 ZIP 已存在，未覆盖”；旧文件仍为 37637 字节、35 个条目。更换新目标后导出成功，新 ZIP CRC 检查通过，SHA-256 为 `565924e71ba264c8e06130c1617d688e42d0fc88aeac4284d3fe037e125e9958`
- 返回工作台后，长导出回执单行省略显示，没有与撤销、重做或语言区域重叠

该会话发现非阻断显示问题：鼠标悬停截断回执出现两张重复 tooltip（自动截断提示与完整文本提示各一张）。修正并通过自动回归后，16:39 UTC 在新 `ba63c7a9…` 二进制实际悬停，确认只有一张完整 tooltip，单行截断及右侧控件边界仍正确。

21 分钟功能会话未设置 `WORLDEDIT_PROFILE_CSV`，没有真实交互 CPU 帧采样。新 `ba63c7a9…` 二进制另开独立短采样会话，结果见下文；功能、布局和 CPU 三种证据不混为一次全平台验收。

## Release 性能测量

下表是本轮性能复测；此后仅有上述测试路径期望和 footer tooltip 修改，core 与性能路径代码未变，没有把旧数据重记为新二进制的测量。测试机器为 Linux `6.18.44` / x86_64，AMD EPYC 9V74 80-Core Processor，进程可见及 affinity 9 个逻辑 CPU；共享主机，CPU 配额及独占性未确认。Rust `1.98.0 (88d9e12ae 2026-08-18)`、LLVM `22.1.8`，jobs=2、incremental=0，dev/test debug=0。业务计时来自 optimized release 构建，未计入编译时间。

| 测量 | 固定负载与采样 | 阶段结果 |
| --- | --- | --- |
| 地图初始化和首帧 | 单层 5000 矩形，976×768 逻辑点地图工作区 | 初始化至首帧完成 436.693 ms，其中首帧 update 49.664 ms |
| 暖态地图平移 | 纹理连续 3 帧 Ready 后，3 轮 × 60 帧，真实 egui 指针事件 | update P95 8.680 / 8.438 / 10.241 ms；update+tessellate P95 8.793 / 8.630 / 10.492 ms |
| 暖态对象树滚动 | 同一负载，3 轮 × 60 帧，真实 egui 滚轮事件 | update P95 2.640 / 2.989 / 2.673 ms；update+tessellate P95 2.793 / 3.144 / 2.848 ms |
| reader 候选窗口 | 2000 候选全部选中，1188×848 逻辑点，10 预热帧后 160 帧 | update P95 0.856 ms，max 1.271 ms |
| profile 完整前台首次提交（最新复测） | 5 份新工程，每份 2000 对象、1600 字段授权 | 209 / 203 / 197 / 192 / 226 ms |
| profile 完整前台更新（最新复测） | 同 5 份工程，每份已有 2000 routes | 235 / 231 / 234 / 236 / 239 ms |
| 关系网络与资料切换 | 1000 对象、3000 关系；网络预热 8 帧后 160 帧，另 160 次资料绘制 | 网络 P95 15.682 ms / max 18.679 ms；资料 P95 6.296 ms / max 13.267 ms |

地图暖态六轮的 update 和 update+tessellate P95 均低于 33 ms，最大单次 update+tessellate 为 13.756 ms。首帧单列，未当作暖态通过。采样间约 60 Hz 的等待给后台执行时间，等待不算 CPU；没有测 GPU、present、vsync 或原生窗口完整帧延迟，因此这些数值不是 native FPS。默认 headless context 也不证明任何物理屏幕 DPI。

profile 计时包含消费 Done、scope/fresh 检查、core 完整 apply 重验、history、版本/缓存、菜单及当前输入更新；规划另计 158–195 ms，未混入前台 apply。每份新工程共 2000 对象和别名，含 1600 地点、200 事件、100 关系、99 时期、1 变量与私有 CANARY。新 fixture 没有清空进程/OS 缓存，不能称作机器冷启动。最新十次完整前台提交均低于 250 ms，最窄余量 11 ms，没有跨硬件性能保证。

优化前 core release 复测曾出现一次 profile apply 269 ms；该世界站测试仍成功，因为 profile 耗时只记录、没有作失败断言。随后完成等价索引优化，增加 v1/v2/v3 与原算法的逐字节对照，并重测五轮 core apply（197–216 ms）及上表十次完整前台提交。保留原超限事实，不用成功退出码掩盖中间失败；其余帧测量及 Linux 完整测试和 Clippy 也已重新通过。

## 独立原生 CPU 帧采样

使用上述 `ba63c7a9…` release、固定 fixture 的 `b_dense` 单层 5000 节点，原生窗口为 1188×848、pixels_per_point=1.0。采样期间保持 42% 视图和 Pan 工具：先做 12 次热身，随后三组各 60 次左右往返拖动；三组中没有切图、绘制、缩放或进入发布窗口。正常 Alt+F4 退出后写出 CSV，stderr 日志为空。

在读取 CSV 测量值之前固定筛选：`200000 <= elapsed_ms <= 320000` 且 `map_active && input_active`；超过 5000 ms 的间隙划分为三组。筛选不依据 CPU 耗时排除慢帧；12 次热身和发布窗口操作均不计入这段平移样本。CSV 共 3935 帧，包含 1829 个输入帧，其中 1632 帧进入本次平移统计；分位数使用 nearest-rank。

| 5000 节点原生平移 | 输入帧数 | P50 / ms | P95 / ms | 最大值 / ms |
| --- | --- | --- | --- | --- |
| 第 1 组，60 次手势 | 551 | 5.724740 | 12.395166 | 17.973332 |
| 第 2 组，60 次手势 | 535 | 5.800874 | 12.188132 | 19.633198 |
| 第 3 组，60 次手势 | 546 | 5.747433 | 12.625494 | 26.797287 |
| 三组合并 | 1632 | 5.752410 | 12.320241 | 26.797287 |

三组各自 P95 均低于 33 ms。指标来自 eframe `Frame::info().cpu_usage`，属于原生 CPU 帧开销；不包含 vsync、GPU、present，不能换算成真实 FPS，也不覆盖所有缩放、资源压力或机器。原始 CSV SHA-256 为 `5ff46c4d81083998d9c1998d36074a43dcf6e6a645b61db8f093537a3b25184d`；固定采样谓词和原始总帧数一并保留，不能只用合并分位数替代每组结果。

## 可选原型的实验性绘制采样

另以 `eds11_prototype` feature 显式执行默认忽略的 `fixed_tasks_draw_cpu_side_profile`。它使用正式原型样例，1024×640 逻辑点、pixels_per_point=1.0，各新 egui context 记录首帧及随后 40 个暖帧；J3 演练与 J4 提案比较在计时前执行，J2 仅注入合成文本事件。

| 原型任务 | 新 context 首帧 / ms | 40 暖帧 P95 / ms |
| --- | --- | --- |
| J1 世界资料 | 10.638288 | 0.079971 |
| J2 写作旁查 | 10.064342 | 0.096476 |
| J3 演练 | 5.031109 | 0.184608 |
| J4 审阅 | 4.519288 | 0.081353 |

测量是 release 下 `Context::run` 的 draw/layout CPU 侧墙钟耗时，不含 tessellation、实际操作系统输入、GPU/present、原生合成器、浏览器缩放或预先执行的 core 操作。首帧不是清空 OS 缓存后的启动；进程全量 RSS 在四任务后为 50112 / 51104 / 53112 / 53524 KiB，不是各任务新增内存。这是实验性技术采样，不是默认产品性能 SLA，也不与前述真实原生平移数值混算。

## 可复现命令与固定作者 fixture

使用相应提交的同级 `worldline/` 与 `worldedit/`，完整配对流程见[配对检查](paired-ci.md)。以下命令在 worldedit 目录执行，构建需要本机平台依赖与仓库指定工具链。

```sh
rustc --version --verbose
python3 scripts/check-source-lines.py
cargo fmt --all -- --check
cargo test --locked
cargo clippy --all-targets --locked -- -D warnings
cargo clippy --target wasm32-unknown-unknown --locked -- -D warnings
cargo build --release --locked
trunk build --release --locked
cargo test --features eds11_prototype --locked
cargo clippy --all-targets --features eds11_prototype --locked -- -D warnings
cargo build --features eds11_prototype --locked
cargo clippy --target wasm32-unknown-unknown --features eds11_prototype --locked -- -D warnings
cargo build --target wasm32-unknown-unknown --features eds11_prototype --locked

python3 scripts/make-world-authoring-fixture.py --output ../native-fixture
python3 scripts/make-world-authoring-fixture.py --output ../native-fixture --check
WORLDEDIT_DENSE_FIXTURE=../native-fixture/authoring/world.wl cargo test --locked author_fixture_presentation_and_media_are_registered -- --ignored --nocapture
WORLDEDIT_DENSE_FIXTURE=../native-fixture/authoring/world.wl cargo test --release --locked dense_scene_headless_release_profile -- --ignored --nocapture
cargo test --release --locked release_profile_2000_full_ui_apply_five_fresh_fixtures -- --ignored --nocapture
cargo test --release --locked two_thousand_candidate_selection_meets_release_frame_budget -- --nocapture
cargo test --release --locked network_release_profile_meets_m2_frame_and_reading_gates -- --nocapture
cargo test --release --features eds11_prototype --locked fixed_tasks_draw_cpu_side_profile -- --ignored --nocapture
```

正式生成器是 [make-world-authoring-fixture.py](../scripts/make-world-authoring-fixture.py)，纯 Python 标准库。输出目录必须为空；`--check` 只读比较，不覆盖验收期间保存的作者稿。固定负载名 `world-authoring-v015-fixed-v1`，生成器 SHA-256 为 `2fa6e8f373f2c63488311abab8e74936663737dedf2fcbcc896e678b5291bbb7`，当前 `fixture-manifest.json` SHA-256 为 `ee3cfd8cf5bfb9986e06143ddc397611ffa3858214a161ecd44d4cfc1c2aa43a`。清单含逐文件字节摘要，机器信息另存，不混入负载内容身份。

生成内容包括三张注册地图：400×200 作者流程图（真实 PNG、6 个 legacy 标记、7 个 scene 节点、校准及中英文字）、单层 5000 矩形图、5000 层各一个全视口半透明矩形的容量压力图；另含 1000 图元 SVG、曲线/组/文字 SVG、根 slice 裁剪 SVG、1 秒 PCM WAV、人物/地点/关系/事件和 3 个书稿章节。默认语言 1.9 与真正空目录新建分别在独立工作区，避免混扫。

最终 `--check` 只读核对 16 个确定性文件，0 mismatch。两仓 Rust 源码行数门禁分别检查 444 / 300 个文件，均不超过每文件 600 物理行；这些数量不是行为测试用例数。

展示门禁已经检查三张地图无诊断、图片/音频可用、校准存在、书稿来源已解析。5000 层压力图的预期是明确容量提示，不能把超额度全部显示当成通过条件；恢复、取消和工程字节保全另有回归。只跑 `wl check` 不足以验证这些展示数据。

原生 CPU 采样方式见[性能记录](performance.md)。每轮使用新的仓外 `WORLDEDIT_PROFILE_CSV` 文件，记录二进制 SHA-256、fixture 清单、操作系统、窗口/DPI、开始结束时间与实际操作；分析交互样本的 P50/P95/max，不用工具调用往返时间代替 CPU 帧。

## 受限路径的纯代码补充模拟

2026-10-02 18:05 UTC，在不访问受阻浏览器入口的条件下补充了以下有限模拟。没有新增产品功能或修改生产源码；这些检查不计入上面的 Rust 套件数量，也不替代真实浏览器和系统文件选择器验收。

- **导出站点的真实搜索脚本与静态闭包**：输入为上述 SHA-256 `565924e7…` 的完整 native ZIP。未改写的 `reader.js` / `search-data.js` 在 Node VM 的最小 DOM 模型中执行，45 组输入与类型切换场景、全部 34 条公开记录及 13 类筛选共 191 项断言通过；覆盖正文、别名、中英、大小写、空白、空输入/无结果、连续输入清旧结果和地图片段链接。另 651 项静态检查确认 29 个 HTML 均从首页可达、480 条本地引用（含 22 条片段）有效、manifest 精确覆盖 35 份资源并核对长度/FNV 摘要、没有外部引用。两次运行结果一致。模型只实现这些脚本所需的 DOM 接口，不证明真实排版、焦点、屏幕阅读器、媒体播放或离线加载。
- **真实 Worker 脚本与 WASM 的线程模拟**：未改写的最终 `worker.mjs`、生成 JS 和上述 `21c718…` WASM 在 Node `worker_threads` 中执行，以内存资源适配代替浏览器模块加载。共 15 项分层检查：11 项执行真实脚本/WASM，2 项 Node terminate，另 2 项分别为下述身份投影和既有 Rust 回归记录核对；均通过。真实预览/渲染及进度成功，RGBA 为 320000 字节、390 个可见像素，transferable 发送端 buffer 分离。不安全 SVG、坏 JSON、跨源模块/资源、无效协议版本/代次、路径越界/重复文件及错误工程基线被拒绝。在初始化暂停点和真实 `svg_parse` 进度点 terminate 均退出且无 Done。它不证明 Browser Worker 的实际调度、加载或下载。
- **主端身份拒绝的投影模型**：按当前 Rust host 的源码条件模拟 54 个 job ID / generation / baseline 变体拒绝。这里只执行投影模型，没有执行 wasm 专用的 Rust `receive`；不能把这一项写成真实主端防护通过。已有相关 Rust 回归仍按前面的完整套件记录，不重复增加数量。
- **系统 picker 返回值边界**：测试直接 include 未改写的 `svg_import_job.rs`，只替换 `rfd` 选择器返回的 `Some(path)` / `None`。实际后台线程、限量文件读取、UTF-8 与大小验证、core SVG 预检执行完成：有效路径、None、不存在路径、目录读取错误、无效 UTF-8、2 MiB + 1 拒绝、恰好 2 MiB 安全 SVG 通过、不安全 SVG 保留源码并拒绝、畸形 SVG 保留源码并报错，共 9 个分支通过。没有显示系统对话框，不证明 portal 或真实选择成功。

这些补充范围没有发现新的产品缺陷；测试模型或适配自身不等于浏览器实现。真实 `file://` / HTTP、Browser Worker 和 SVG 系统选择器的未验边界继续保留。

## 未验证及适用边界

- Windows 配对成功适用于上述 exact 源码与运行，不代表 Windows/macOS 原生 GUI 或浏览器验收通过；后续源码变化须另行验证
- 非最终候选的 30 分钟以上原生操作没有计入本轮 21 分钟会话；新修补 hash 的悬停和原生 CPU 样本单独记录。Windows/macOS GUI、物理 IME、读屏及 Linux SVG 文件选择器成功路径未获通过证据。本轮点击 picker 未出现系统对话框并返回空选择，SVG 源码输入成功不替代文件选择成功
- 实际 `file://` 打开被工具安全策略拒绝；本轮正常 HTTP 重试返回 `ERR_BLOCKED_BY_CLIENT`。真实静态站布局、离线搜索/链接/地图和 browser Worker 均未验，没有绕过限制
- Node WASM、静态资源审计、自动测试中的页面打开计数器，不证明浏览器下载或系统浏览器真正打开成功
- 编辑器 Windows 结论只接受最终 exact head 和配对 core 的 CI 结果；core 单仓成功、旧 editor run 或 Linux 通过均不能替代
