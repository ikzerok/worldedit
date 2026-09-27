# 本地化工作台（CAP-09D）

工作台围绕 worldline-core 的 `LocalizationSelection` 与 JSON 交换协议提供显式白名单导出和译文导回。面板只显示 core 返回的源文件、行号、kind、字符串片段与诊断；不会解析 `.wl`、猜测 locale、扩大 ID 范围或添加引擎适配。协议规则以 [worldline/spec/localization.md](../../worldline/spec/localization.md) 为准。

## 操作

1. 输入区分大小写的源语言、目标语言和每行一个的字符串 ID；locale/ID 格式、重复项、能力开关、源引用与 protected token 均交由 core 验证。
2. 选择“预览导出”后，检查字符串协议版本、每项源引用和诊断；来源与诊断位置可点击返回源码。只有 core 允许的计划可继续导出；计划过期、缺少 ID 或工程编译失败时，不生成文件。
3. 选择 JSON 交换文件，或在编辑区粘贴/编辑 UTF-8 JSON，再选择“预览导入”。面板显示交换包的源引用、译文状态及 core 诊断；缺译、旧源（`STALE_SOURCE`）和占位符/链接 token 问题（`INVALID_TOKEN`）不会被静默导回。
4. 只有 `can_apply` 通过的计划可打开确认框。确认后 core 重新校验并执行 Project 导入事务；取消、预览错误或 apply 错误保留交换文本且不清空 Project 缓冲。未保存的 Project 修改会被 core 阻止，需先保存或撤销。

非空导入 JSON 和尚未成功导入的 locale、ID 配置均作为未提交输入保留；即使已导出交换文件，切换工程仍会提示，以免丢失工作台中的表单值。替换文件读取失败会撤销旧的导入许可，但不删除编辑区原文，需重新预览才能确认。

## 平台范围与未验证项

- **原生：**导出通过 core `Project::export_localization` 创建新的工作区外 `.json` 文件，不覆盖现存目标；导入使用文件选择器和 core `Project::apply_localization_import`。源基线、计划摘要、Project 基线及 token 校验由 core 重验，成功后才替换 Project 缓冲。
- **Web：**导出将 core 预览中的交换 DTO 编码为 UTF-8 JSON Blob 并交给浏览器下载；浏览器控制最终下载位置与同名文件处理，编辑器不能承诺原生 `create_new` 的目标路径/不覆盖保证。可选择一个 `.json` 文件并由 core 生成导入预览，但当前配对 core 的 `apply_localization_import` 不编译到 wasm，因此 Web 的“应用译文”保持禁用；预览不会写入工程，不能将此路径视为完整导回支持。
- 本说明不等同于桌面文件对话框或浏览器下载/文件选择的人工验收。未经对应平台实测，不标记平台通过；Web 下载由浏览器接收也不证明用户已保存文件。

工作台不提供 runtime locale、自动回退、机器翻译、多人翻译服务或引擎加载。