# 0.2 内容/展示格式升级与回退

本说明覆盖 M1–M4 新增的 Project 展示文档、语言 1.10 作者资料和协作文档。升级是显式的；打开旧作品本身不会创建清单、提高语言版本或改写源码。

## 旧工程保持方式

没有 `.world/project.json` 的作品继续按旧模式递归读取全部 `.wl`，语言版本为 1.9。已有 1.9 清单也不会因打开新版编辑器自动变成 1.10。地图、网络布局、预设、批注和提案只有在作者明确创建/保存对应文档后才进入工程生命周期。

未知 `language_version` 或未知 `required_features` 会把相关展示/清单能力置为只读；原始字节仍可另存，不用空结构替换。旧工具回退前应保留完整工程副本；旧版本不得用于安全编辑它不理解的新必需能力。

## 显式启用 1.10

通用 `entity` 与独立 `relation_type` / `relation_def` 需要清单 `language_version: "1.10"`，并按实际使用声明 `content.entities.v1` / `content.relations.v1`。旧 character 关系仍保留旧身份与旧指纹规则；需要稳定关系 ID 时使用显式提升预览/提交，不静默转换。

展示文档按用途注册：`maps`、`graph_views`、`presets`、`comments`、`proposals`。相应必需能力为 `presentation.maps.v1`、`presentation.graph_views.v1`、`presentation.presets.v1`、`collaboration.comments.v1`、`collaboration.proposals.v1`；未知可选字段往返保留。

## 活动源码集

不配置 `source_config` 时继续递归分析全部 `.wl`。只有同时声明 `workspace.source_sets.v1` 并提供 `mode: explicit`、非空 `active` 和 `archived` 后才启用显式源码集。入口必须属于 active；越界路径、active/archived 重叠、include 到非活动文件都明确报错，不退回“偷偷全读”。

## 模板与作者范围

16 类创作模板只是 UI 提示和可选字段目录，不保存实例值。切换或删除模板不会删除正文、未知自定义属性或别名。时期/章节/版本范围只筛选作者明确写下的 scope；不推断继承、历史插值或世界状态。

## 迁移负例

自动回归覆盖：无清单/1.9 兼容、未知版本/必需能力只读、GraphView 损坏/重复键、陈旧 Revision、source_config 越界/归档 include、模板切换长文保真、跨视图重命名冲突、批注失锚和提案三方冲突。最终证据见 [WP-17 验收](wp17-acceptance.md)。

## 回退

回退编辑器时优先检出曾通过配对 CI 的 worldedit 提交，并按其 `compatibility.json` 检出对应 worldline SHA。不要只回退一个仓库或只修改 compatibility 指针。新版能力仍在作品中的情况下，旧工具只用于读取它已知的部分；需要继续编辑时使用匹配的新配对版本。
