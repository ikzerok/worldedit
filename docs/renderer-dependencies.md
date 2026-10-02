# 矢量显示依赖

原生与WASM均使用resvg 0.48.1，只启用text支持，关闭默认外部字体/图片特性。文字字体沿用编辑器随包内嵌的Noto字体及其OFL许可。MIT许可原文在 `assets/licenses/resvg-MIT.txt`，桌面与Web打包复制为 `RESVG-LICENSE.txt`；原crate同时提供Apache-2.0选项，本产品使用其MIT授权。

resvg只处理worldline-core安全serializer产生的SVG，不直接执行作者输入。外部资源解析器被禁用；profile预检、路径、文字、组、affine、根viewport裁剪与作者编辑语义仍以core为准。栅格结果是按实际viewport/zoom/DPI生成的派生缓存，不替代可编辑矢量数据。

渲染库内部隔离组/裁剪表面在分配前按保守峰值预检，不能把我方RGBA预算冒充整个进程内存上限。具体平台预算、最多实际任务数和拒绝/取消边界见 [矢量作者流程](vector-authoring-0.15.md) 与 [后台计算](web-worker.md)。依赖版本、native/wasm兼容与资源闭包都必须由最终候选构建验证。
