# 可选 EDS-11 内存工程原型

此入口用于独立布局与 core 接线实验，不替换默认编辑器。默认编译不启用 `eds11_prototype` feature；即使启用，原生仍须明确传入 `--eds11-prototype`。Web 试验入口只匹配精确 query 参数 `eds11=1`，没有该参数仍使用正常编辑器。

运行：`cargo run --locked --features eds11_prototype -- --eds11-prototype`。

## 能力与边界

- 两个虚构内存 Project，不打开、迁移、save 或导出用户作品；草稿、展示位置、运行结果和比较结果按作品隔离
- J1 使用真实 `presentation_commands::apply/undo`，验证 revision、文档 hash、标记身份及锁层。只读/锁定样例在候选克隆构造拒绝条件；失败克隆不会替换当前内存工程，取消只结束候选
- J3 临时预览不创建 Story，明确运行后才执行真实 runtime，显示实际输出、ended 与 visits
- J4 调用真实 `preview_proposal`；尚未接通的解决和采纳不伪装完成，不提供默认采纳动作
- J2 仍是显隐、焦点与按作品恢复输入的布局实验，醒目标明假数据，不伪装写入 core

提交计数只表示成功的展示命令与撤销。作品目录不创建；界面的作品磁盘零写说明不代表对 OS/图形驱动缓存做过全盘 syscall 审计。

## 入口与持久化隔离

集成沿用当前 `compatibility.json`，不带回旧原型分支的兼容文件或旧生产代码。原型使用独立 app ID `worldedit-eds11-prototype`，显式禁止窗口与 egui memory 持久化，App::save 不写任何键，不保存布局、收藏或草稿。

检查使用 `python3 scripts/check-pair.py` 或 Windows 的 `scripts/check-pair.ps1`，包含默认与 feature 开启的测试、严格 native/WASM Clippy 和构建。默认编辑器能力与实验入口分别验证。

可手动运行技术采样：`cargo test --locked --features eds11_prototype --bin worldedit eds11_prototype::profile::fixed_tasks_draw_cpu_side_profile -- --ignored --exact --nocapture`。这是 CPU 侧 egui 绘制/布局与进程内存采样；J3/J4 核心动作在计时前准备，不等同物理输入到屏幕延迟、GPU 呈现或发行性能承诺。

本轮采用云端 Linux + AI 模拟验收。Windows 实际会话、物理 IME/DPI、Web 实际交互和真人研究未执行，不伪称等价通过。验收材料按维护者要求仅在本地保留。设计采用范围见 [ADR](../../design/ADR-20260930-optional-prototype.md)。
