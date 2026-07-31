# ShadowDoc 开源规范与生态计划

> 状态：孵化草案，2026-07-31。
>
> 本文回答：ShadowDoc 和影核是什么关系、它相对现有项目新在哪里、怎样从一个产品内部模型
> 变成可被其他项目独立实现的开放规范，以及如何围绕开源规范形成长期生态与商业。
>
> 数据模型见[影文档协议](./影文档协议.md)，产品与收费边界见
> [开源与商业化](./开源与商业化.md)。

## 1. 结论

可以参考 ActionParity（影核）的路径，把 ShadowDoc 做成开放规范；而且值得做。

但发布方式不是马上宣布“行业标准”，而是：

> **先用叠象证明这个模型能解决真实文件任务，再以规范、Schema、样例、验证器和第二个实现，
> 逐步证明它不是某个产品的私有 JSON。**

建议命名：

| 层级 | 正式名称 | 用法 |
|---|---|---|
| 动作规范 | **ActionParity（影核协议）** | 业务动作只实现一次，所有界面调用同一个核心 |
| 文档规范 | **ShadowDoc Specification（影文档开放规范）** | 原件、多影层、标注、工单和版本关系 |
| 配套关系 | **ActionParity companion specification** | 两者互补，但可以独立实现 |
| 参考产品 | **叠象 / Stelora** | 第一个完整用户产品和参考实现 |
| 当前代码名 | **Redline** | crate、CLI、Action ID 和 v1 schema 暂不迁移 |

中文传播可以说“**影核体系的影文档规范**”或“**影核·影文档**”，
但不建议把规范正式命名为“叠象文档协议”：

- 规范绑死厂商品牌，会降低其他项目采用意愿；
- 产品换品牌不应该迫使 Schema、包名和实现一起迁移；
- 叠象应该因“最好用的 ShadowDoc 实现”获益，而不是靠名字控制规范。

## 2. 与 ActionParity 的边界

两套规范共同遵循“核心事实不藏在界面里”，但回答不同问题：

```text
ShadowDoc
  文件是什么？有哪些影子层？人圈的是哪里？AI 基于哪一版？

ActionParity
  能执行什么动作？动作在哪里实现？GUI/CLI/MCP 是否调用同一个核心？

MCP / CLI / API
  用什么传输和调用？

叠象
  怎样把这些组合成用户真正会安装的产品？
```

互操作原则：

1. ShadowDoc 实现**不必**实现 ActionParity；
2. ActionParity 应用**不必**处理 ShadowDoc；
3. 两者组合时，`document.inspect`、`workorder.create`、`version.branch` 等 Action
   只实现一次，输入输出引用 ShadowDoc 对象；
4. MCP 只是其中一个调用面，不成为第二份文档模型或业务实现；
5. 将来可以定义非强制的 `ShadowDoc Action Profile`，规定推荐 Action ID 和 Schema 映射。

独立可采用非常重要。一个解析器项目可以只输出 ShadowDoc；一个标注工具可以只消费
Selectors；一个 Agent 可以只消费 WorkOrder。只有叠象需要把完整循环全部做通。

## 3. 真正的闪光点

“把各种文档转成一种结构”本身不是新思想。真正有辨识度的是以下组合：

### 3.1 原件不是中间格式的牺牲品

传统文件继续是权威原件，Shadow Layer 都与原件 sha256 绑定、可丢弃、可重建。
新标准不要求用户迁移掉 DOCX/PDF，也不把有损 AST 冒充原文件。

### 3.2 一个版本拥有多个信任隔离的影子层

视觉、语义、格式转换、OCR、AI 摘要和人工标注不是同一种事实：

- `source-derived`：确定性派生；
- `machine-inferred`：OCR/AI 推断；
- `human-authored`：人工意图；
- `system-recorded`：工单和审计。

现有工具常把 OCR、模型描述和解析结果混在同一输出里。ShadowDoc 要求消费者知道
“这句话来自原件、OCR 猜测、AI 摘要，还是人写的要求”。

### 3.3 标注不是截图，是可执行工单的锚点

同一处修改同时使用 unit、文字哈希/引用和 bbox 等 Selector。人的圈点可以直接变成
模型中立 WorkOrder，而不是再由人把“第二页右上角”翻译成聊天提示词。

### 3.4 AI 不修改抽象影子后冒充原文件

Agent 消费原件和被授权的影子层，输出一个新标准文件；核心验证后才创建新版本节点。
这避开了“通用 AST 能否无损写回所有格式”这个几乎不可兑现的承诺。

### 3.5 无限画布有可验证的数据地基

横向迭代、纵向分叉、换 Agent，不是聊天 UI 动画，而是不可变 `VersionNode` 和
`ActionEdge`。每个新版本拥有自己的 ShadowBundle，能重新预览、diff 和继续加工。

### 3.6 人和多个 Agent 共享同一位置语言

人在视觉影子上操作，AI 在语义影子上读取，两者通过相同 Selector 和源哈希对齐。
换 Claude、Codex、本地模型或行业 Agent，不需要重新上传、重新 OCR、重新解释位置。

这些思想单独看都有先例；**把它们组织成一个本地优先、模型中立、原件不可变、
可执行且可审计的文件加工循环**，是 ShadowDoc 最值得开源验证的主张。

在完成更系统的论文、专利和产品检索前，不对外宣称“全球首创”；对外应说：

> ShadowDoc 提出了一种新的开放组合模型，并提供可运行参考实现与可重复基准。

## 4. GitHub 和现有标准里有哪些相邻智慧

| 项目/标准 | 它的智慧 | ShadowDoc 应借鉴什么 | 它没有覆盖的主循环 |
|---|---|---|---|
| [Docling](https://github.com/docling-project/docling) / [DoclingDocument](https://docling-project.github.io/docling/concepts/docling_document/) | 统一文档表示、层级、bbox、provenance、丰富解析器 | 作为 `semantic`/`visual` ParserAdapter；不要重写成熟解析器 | 不负责人工修改工单、标准文件新版本和 Agent 分叉图 |
| [Unstructured](https://github.com/Unstructured-IO/unstructured) | 按格式 partition 成元素，支持 OCR/布局策略和流水线 | 格式适配、策略选择、失败降级必须显式 | 主要面向 ETL/RAG，不是原件保护与结果文件迭代协议 |
| [MarkItDown](https://github.com/microsoft/markitdown) | 为 LLM 保留必要结构，输出 token 友好的 Markdown；插件化 | `ai-context` Layer 应小、便宜、可按需生成 | Markdown 是一种上下文投影，不承载视觉锚点和版本图 |
| [Pandoc](https://github.com/jgm/pandoc) | Reader → AST → Writer 的模块化；明确承认表达力下降会有损 | 适配器可插拔，并诚实记录 `lossy` | 不追踪原件哈希、人工意图、Agent 动作和结果节点 |
| [W3C Web Annotation](https://www.w3.org/TR/annotation-model/) | Body/Target/Selector，可用多个 Selector 指向资源片段 | Selector 设计尽量兼容，不自造全部标注术语 | 不定义 Office/压缩包语义层、AI 工单和新文件生成 |
| [IIIF Presentation API](https://iiif.io/api/presentation/3.0/) | Manifest/Canvas/Annotation Page，把空间或时间内容画到稳定 Canvas | 视觉 Layer 的 Canvas、页、Range 和标注页设计 | 重点是数字藏品呈现，不处理任意本地文件加工 |
| [W3C PROV-O](https://www.w3.org/TR/prov-o/) / [RO-Crate](https://github.com/ResearchObject/ro-crate) | Entity/Activity/Agent 和可携带项目元数据 | ActionEdge 和 `.shadowdoc` 项目可提供 PROV/RO-Crate 映射 | 通用 provenance 不定义文档位置、预览和修改安全 |
| [cite / OpenContracts](https://github.com/Open-Source-Legal/OpenContracts) | 人工标注为 ground truth，AI 与人共享文档图；有 MCP、版本化 corpus 和精确坐标映射 | 证明“人类标注 + Agent + 文档图”是真需求；学习实现报告和社区 corpus | 聚焦引用/知识图，不把任意文件加工成一串可交付新版本 |
| [Automerge](https://github.com/automerge/automerge) | local-first、不可变快照、历史、分支和自动合并 | 项目元数据未来多人协同时可采用 CRDT | 不尝试无损合并任意 DOCX/PDF/CAD 二进制产物 |
| [tldraw](https://github.com/tldraw/tldraw) / [xyflow](https://github.com/xyflow/xyflow) | 成熟无限画布与节点图交互 | 画布只做 Version Graph 的投影，不自己成为事实源；tldraw 当前生产使用有专用许可，xyflow 为 MIT，选型前复核 | UI 引擎不定义文件、工单和安全语义 |
| [MCP](https://modelcontextprotocol.io/specification/2025-06-18/index) | Resources、Tools、Prompts 与用户同意边界 | 用 MCP 暴露 ShadowDoc 资源和 ActionParity 动作 | MCP 不规定文档内部模型，也不保证 GUI/CLI 共用实现 |

这里最接近产品思想的是 cite/OpenContracts，但它也反过来证明了差异：
它的核心成果是**可积累的引用图**；ShadowDoc 的核心成果应是**可积累、可验证、
可继续分叉的文件加工图**。

## 5. v0.1 规范应该管什么

### 必须规范

- `SourceArtifact`、`ArtifactVersion` 的身份和内容哈希；
- `ShadowBundle`、`ShadowLayer`、Layer revision、依赖与信任类型；
- 最小 unit 结构、稳定 ID 和来源信息；
- `Selector`、`Annotation` 以及陈旧/冲突时的拒绝规则；
- `WorkOrder` 的输入节点、允许 Layer、人的要求、权限与输出约束；
- `VersionNode`、`ActionEdge` 和“新产物不覆盖输入”的不变量；
- schema/version/extension 规则；
- 安全错误码和消费者必须拒绝的场景；
- 规范符合性声明、实现报告和测试 fixture。

### 不应规范

- 某个 PDF/OCR/Office 解析算法；
- 某个模型供应商或提示词；
- 无限画布长什么样；
- GUI、CLI、MCP 三者必须全部存在；
- 所有格式必须压成一个无损 AST；
- `.shadowdoc` 必须采用 ZIP、SQLite 或某个二进制封装；
- 云计费、用户账号或叠象品牌规则。

### 分层 Profile

不要让第一个实现就吞下全部复杂度：

1. `Core Profile`：原件身份、Bundle/Layer、unit 和来源；
2. `Annotation Profile`：Selectors、人工标注和 stale 拒绝；
3. `Agent Profile`：WorkOrder、权限、输出约束和验证；
4. `Project Profile`：Version Graph、ActionEdge 与可选 `.shadowdoc` 容器。

实现可以声明支持哪些 Profile，但 Core 是其余 Profile 的前置条件。

## 6. 先在当前 monorepo 孵化，不急着拆新仓

现阶段规范和参考实现互相校正，放在同一个公开仓库最不容易漂移：

```text
spec/shadowdoc/
  README.md
  SPEC.md                       规范正文（后续）
  schema/
    core.schema.json
    annotation.schema.json
    workorder.schema.json
    project.schema.json
  examples/
    minimal/
    annotated-docx/
    scanned-pdf/
    branched-project/
  conformance/
    valid/
    invalid/
    security/
  rfcs/
  implementation-reports/
```

满足以下任一条件后，再考虑抽成 `shadowdoc-spec` 独立仓：

- 出现第二个非叠象实现，需要独立发布节奏；
- 有外部维护者只参与规范，不参与桌面产品；
- 规范的知识产权/许可证边界必须与产品代码分开；
- 中立基金会或工作组愿意接管规范治理。

在此之前拆仓只会制造 Schema 与参考实现漂移。可以通过 GitHub Pages 提供独立规范站点，
不等于必须先有独立仓库。

## 7. 许可证和知识产权

规范、软件、说明文档和样例数据不是同一种作品，不建议全部机械套一个许可证。
正式发布前请专业核验，建议目标结构：

| 内容 | 建议许可证 |
|---|---|
| Rust/TS 参考实现、Schema 验证器、CLI、测试代码 | Apache-2.0 |
| 规范正文 | Community Specification License 1.0，评审后启用 |
| 教程和非规范说明 | CC BY 4.0 |
| 自动生成/无隐私测试数据 | CC0 或明确的可再分发许可 |
| 名称、Logo、兼容徽章 | 独立商标政策 |

Linux Foundation 的
[Community Specification](https://www.linuxfoundation.org/projects/standards)
就是为 Git 工作流、规范、参考实现和测试套件同仓发展设计的；项目成熟后再考虑加入基金会，
现在不用先交出控制权或承担组织成本。

规范贡献除了 DCO，还需要明确的规范知识产权和专利承诺。Apache-2.0 适合软件，
但不能想当然地等于完整的开放标准 IPR 流程。启用 Community Specification License、
成立工作组或接受公司级规范贡献前应做法律核验。

## 8. 发布阶段和准入门槛

### v0.1 Working Draft

- 规范正文包含 MUST/SHOULD/MAY 的明确规则；
- 四个 Profile 至少完成 Core；
- 发布 JSON Schema、最小样例和非法样例；
- Redline 是第一个实现；
- CLI 验证器可以输出稳定 JSON 和退出码；
- 明确标注“孵化草案，不代表行业标准”。

### v0.2 Implementers Draft

- DOCX、PDF、XLSX、图片/扫描件至少四类真实 fixture；
- Docling 或 MarkItDown 至少一个进程外 Adapter；
- GUI/CLI/MCP 中至少两种调用面通过同一核心；
- 第一份外部实现报告或兼容性实验；
- Schema 扩展和未知字段规则稳定。

### v0.9 Release Candidate

- 至少两个独立代码库实现 Core Profile；
- conformance suite 覆盖 stale、路径越界、覆盖输入、歧义 Selector 等安全场景；
- 破坏性改动流程、迁移文档、错误码注册表和安全考虑齐全；
- 公开 RFC 期至少 60 天。

### v1.0

- 两个独立实现通过同一套公开测试；
- 至少一个实现不是由叠象团队维护；
- 真实文件任务证明标注到新产物的成功率或成本显著优于传统流程；
- 没有未处理的高危安全问题；
- 规范 IPR、商标和治理文件完成核验；
- v1 字段兼容承诺和废弃周期明确。

没有第二个实现，就只有“公开的数据结构”，还不是事实标准。

## 9. GitHub 运作方式

### 孵化期治理

- 维护者主导，但所有规范修改走公开 PR；
- 行为变化必须附 RFC、样例和 conformance fixture；
- RFC 至少开放评论 14 天，安全紧急修复除外；
- 合并说明必须记录采用/拒绝意见和理由；
- 规范版本与参考实现版本分开；
- 不通过私聊决定破坏性字段变化。

### 进入社区治理的触发条件

当出现至少三个非创始贡献者、两个独立实现或两个组织采用后：

- 成立 3–5 人维护委员会；
- 任一公司不能单独占多数席位；
- 规范符合性测试永久免费；
- 商业支持、托管 CI、培训和实施可以收费；
- 不能通过付费才能声明基本兼容；
- 兼容徽章可受商标规则约束，防止恶意冒充。

### GitHub 必备材料

- 90 秒能看懂的问题/方案演示；
- `SPEC.md`、JSON Schema、最小合法/非法样例；
- `shadowdoc validate` 或等价验证器；
- `CONTRIBUTING.md`、RFC 模板、实现报告模板；
- `SECURITY.md`、威胁模型和隐私说明；
- changelog、迁移指南、版本支持表；
- `good first issue`：格式 Adapter、Selector fixture、语言 SDK；
- Discussions：规范问题、实现帮助、RFC 和展示案例分区。

## 10. 怎样获得采用，而不只是 GitHub Star

ShadowDoc 的第一批用户不是普通办公用户，而是这些项目的开发者：

1. 文档解析/OCR 工具；
2. 文档标注和审阅工具；
3. Agent/MCP 框架；
4. 本地优先知识管理工具；
5. 合同、科研、财务等行业文档系统。

最有效的传播不是发一篇宏大宣言，而是提供可直接复用的桥：

- `DoclingDocument → ShadowDoc semantic Layer` Adapter；
- `MarkItDown → ai-context Layer` Adapter；
- W3C Web Annotation ↔ ShadowDoc Selector 映射；
- PROV-O/RO-Crate ↔ ActionEdge/Project 导出；
- MCP Resource/Tool 示例服务器；
- 10 份公开、无隐私的黄金文件和预期输出；
- 一段真实演示：圈选 PDF → 换两个 Agent → 生成两个新 DOCX/PDF 分支 → 对照。

每个外部项目只要能采用一个 Profile，就算生态增长；不要要求别人安装完整叠象。

## 11. 开源后的商业运作

ShadowDoc 越开放，叠象越有机会成为默认实现，但不能把基本兼容做成收费门槛。

| 永久开放 | 可以收费 |
|---|---|
| 规范、Schema、验证器、conformance suite | 托管兼容性 CI 和迁移服务 |
| 本地参考实现、CLI、MCP | 官方签名桌面发行版的持续运维价值 |
| 本地多影层、标注、版本图 | 云 OCR、云模型、跨设备同步和长任务沙箱 |
| 自带 key/本地模型 | 免配置官方 Agent、成本路由和失败恢复 |
| 实现报告和基本兼容声明 | 企业策略、审计汇聚、SSO、SLA 和私有部署 |
| 社区 Adapter API | 专业格式 Adapter、认证支持与行业实施 |

事实标准带来的护城河不是收“协议税”，而是：

- 叠象最先、最完整地实现规范；
- 官方签名发行版最省心；
- 拥有最多真实格式 fixture 和兼容经验；
- Agent、格式厂商和企业系统优先对接 ShadowDoc；
- 云服务、同步、企业责任和支持自然聚集到主要维护者。

## 12. 接下来 90 天

### 0–30 天：把内部模型变成可挑战的草案

- 建立 `spec/shadowdoc/` 孵化目录；
- 从现有实现提取 Core Profile 的规范性规则；
- 发布最小 JSON Schema 和 5 组合法/非法 fixture；
- 给 `document.inspect` 生成第一份实现报告；
- 开启 GitHub Discussions/RFC 模板。

### 31–60 天：证明不是纸面协议

- 实现验证器；
- 完成 Annotation Profile 的多 Selector 和 stale 拒绝；
- 做 Docling/MarkItDown Adapter 中至少一个；
- 发布 DOCX、PDF、XLSX、扫描件黄金样例；
- 用同一工单跑 Claude/Codex 两种 Agent。

### 61–90 天：寻找第二个实现者

- 发布 `v0.1 Working Draft` GitHub Release；
- 做独立规范页面和 5 分钟实现教程；
- 邀请 5–10 个相邻项目维护者评审，不群发营销；
- 接受第一个外部 Adapter 或实现报告；
- 根据真实互操作问题修改规范，而不是只看 Star。

核心指标：

- 独立实现数；
- 通过 conformance fixture 的项目数；
- 外部 Adapter 数；
- 真实 WorkOrder → 新文件成功率；
- 相对全文发送的 token 节省；
- stale/歧义/覆盖输入被正确拒绝的次数；
- 源文件被意外改写事故：必须始终为 0。
