# ShadowDoc Core 0.1 实验报告

> 日期：2026-07-31
> 结论状态：第一轮可行性证据，不是“最佳标准”证明。

后续：第一轮记录的 Markdown 0% 反例已经在
[`ShadowDoc Core 0.1 第二轮实验`](./ShadowDoc实验报告-v0.1-第二轮.md)中修复并重测，
本文件保留原始结果作为前后对照。

## 一句话结论

ShadowDoc 的核心假设已经通过第一轮机械实验：对于有稳定语义分段的 DOCX、XLSX、
PPTX、PDF，只发送目标 unit 和相邻上下文，JSON 上下文字节比发送全部 units 少
**87.38%–98.49%**；同一输入重复解析 10 次，源身份和 unit 锚点稳定率均为 **100%**；
原件变化后，旧影文档会被 `stale_shadow` 拒绝。

但它还不能宣称“目前最适合 AI 时代的人机协作标准”。真实 Markdown 在当前实现中只有
一个全文 unit，上下文缩减是 **0%**；尚未做真实复杂文件的解析正确率、Agent 修改成功率、
跨生产器兼容性和第二独立实现测试。现阶段准确定位是：

> **一个值得继续验证的开放协作层 Working Draft，而不是一个已经胜出的新标准。**

## 1. 要验证什么

| 假设 | 可证伪指标 | 第一轮结果 |
|---|---|---|
| H1：人和 AI 可稳定引用同一位置 | 同一字节输入重复 10 次，ordered `(id, kind, textSha256)` 一致 | 5 个场景均 100% |
| H2：局部工单可明显减少 AI 上下文 | 目标 unit + 前后各 1 个，与全部 units 的 UTF-8 JSON 字节比较 | 多 unit 格式减少 87.38%–98.49% |
| H3：旧影子不会误套到新原件 | 修改源字节后带旧哈希派发 | 正确拒绝，错误码 `stale_shadow` |
| H4：规范不是只靠人读 | 正例被接受、反例有机器可读错误 | 6/6 conformance tests 通过 |
| H5：所有格式都自然获得收益 | 对真实仓库 Markdown 做相同测量 | 未通过：单 unit，减少 0% |

这里的“上下文”是 `units` JSON 的 UTF-8 字节数，是可重复的**代理指标**，不是模型账单
token。它不包含系统提示词、工单模板和模型 tokenizer 差异，因此不能把 98.49% 直接说成
“省 98.49% 费用”。

## 2. 方法

### 环境

- Windows 11 家庭版；
- Intel Core i9-12900H，63.8 GiB 内存；
- Rust 1.88.0；
- `release` profile；
- 基线 commit `ba8f53d`，叠加本报告对应的未提交实验实现。

### 语料

受控生成四类合法文件：

- 200 段 DOCX；
- 24 个 sheet、每 sheet 100 行的 XLSX；
- 40 页 PPTX；
- 40 页带文本层 PDF。

另加入仓库中的真实文档 `docs/影文档协议.md`。受控语料用于让分段数量和内容完全可重复，
真实 Markdown 用来防止只在理想样本上得出结论。

每个文件先预热一次，再用同一 `redline_core::inspect` 路径重复 10 次。选中窗口固定为中间
unit 加前后各一个。安全实验先 inspect 一个文本文件，随后追加字节，再用旧
`expectedSourceSha256` 调用 `agent.dispatch` 的 dry-run。

复现命令：

```text
cargo run --release -p redline-core --example shadowdoc_benchmark
node --test scripts/shadowdoc-validate.test.mjs
```

机器可读原始结果见
[`spec/shadowdoc/reports/redline-core-0.1-windows-x64.json`](../spec/shadowdoc/reports/redline-core-0.1-windows-x64.json)。

## 3. 结果

| 场景 | units | 全量 bytes | 局部 bytes | 减少 | 锚点稳定 | inspect 中位数 |
|---|---:|---:|---:|---:|---:|---:|
| 受控 DOCX | 200 | 47,477 | 718 | 98.49% | 100% | 1.089 ms |
| 受控 XLSX | 24 | 115,083 | 14,527 | 87.38% | 100% | 4.872 ms |
| 受控 PPTX | 40 | 8,294 | 625 | 92.46% | 100% | 1.080 ms |
| 受控 PDF | 40 | 7,574 | 571 | 92.46% | 100% | 1.629 ms |
| 真实 Markdown | 1 | 20,657 | 20,657 | **0%** | 100% | 0.225 ms |

这组数据支持的是“稳定 unit + 局部工单有潜在效率收益”，不是“解析质量已经超过
Office、Docling 或专业 PDF 引擎”。受控 OOXML 高度可压缩，`sourceBytes` 也不代表真实办公
文件大小；解析耗时只用于本机回归，不能外推到扫描 PDF、图片、复杂表格和低配电脑。

## 4. 与现有方案 PK：谁做什么最好

这轮没有在本机安装 Docling、MarkItDown 和 Unstructured，因此下表是基于各项目官方契约的
**能力边界比较**，不是同语料性能排名。

| 方案 | 官方主要目标与已具备优势 | ShadowDoc 应该借用什么 | ShadowDoc 想补的层 |
|---|---|---|---|
| **Docling** | 多格式解析、先进 PDF 理解、OCR、版面/表格结构、统一且可导出 lossless JSON 的 `DoclingDocument` | 把它作为高质量 `ParserAdapter`，尤其 PDF/OCR/表格；不要重写它已经成熟的解析器 | 源哈希绑定的协作事务、人的意图、模型中立工单、新文件版本与分叉 |
| **Unstructured** | 把文件 partition 成 Title、NarrativeText、ListItem 等 elements，并带页码、坐标和表格 HTML 等 metadata；适合 ingestion/RAG/chunking | 复用 element 分类、坐标和 chunk 策略 | 把抽取元素变成人和 AI 共用、可验证且不覆盖原件的修改锚点 |
| **MarkItDown** | 轻量地把多种文件转成适合 LLM 的 Markdown；官方明确说目标是文本分析而非高保真人类转换 | 做轻量 fallback / 文本影子，快速扩大格式覆盖 | Markdown 输出之外的源身份、稳定 selector、标注、修改结果和版本图 |
| **W3C Web Annotation** | 已是 W3C Recommendation，定义跨平台 annotation、target、selector；成熟度远高于 ShadowDoc | Annotation Profile 直接兼容它的 selector 思想，不自创一套孤岛批注词汇 | 文档解析、AI 工单、源文件安全约束和输出版本不在它的范围内 |
| **ShadowDoc Core 0.1** | 当前已实测源哈希绑定、确定性 unit、局部上下文和 stale 拒绝；模型中立 | — | 解析广度、视觉 provenance、OCR、真实任务效果、生态与治理都还没证明 |

官方依据：

- [Docling 官方仓库](https://github.com/docling-project/docling)列出多格式、先进 PDF、
  OCR、统一表示、lossless JSON 和 MCP；其
  [DoclingDocument 文档](https://docling-project.github.io/docling/concepts/docling_document/)
  说明了 provenance。
- [Microsoft MarkItDown 官方仓库](https://github.com/microsoft/markitdown)明确把自身定位为
  面向 LLM/文本分析的轻量 Markdown 转换器，并提醒并非高保真人类转换。
- [Unstructured 官方 partition 文档](https://docs.unstructured.io/open-source/core-functionality/partitioning)
  和 [elements/metadata 文档](https://docs.unstructured.io/api-reference/legacy-api/partition/document-elements)
  说明了元素、页码、坐标与表格 metadata。
- [W3C Web Annotation Data Model](https://www.w3.org/TR/annotation-model/)是正式
  Recommendation，并定义了 Selector 等可复用模型。

最关键的产品判断不是“打败这些解析器”，而是：

> **用 Docling / Unstructured / MarkItDown 生产 Shadow Layer；由 ShadowDoc 统一约束来源、
> 锚点、人的意图、Agent 工单和新文件版本。**

如果 ShadowDoc 变成另一个通用文档 AST，它会直接落入成熟项目的优势区，而且没有胜算。

## 5. 这算标准了吗

还不算。现在具备了从“理念”进入“可互操作草案”的最低门槛：

- normative `SPEC.md`；
- Draft 2020-12 JSON Schema；
- 无第三方依赖的 validator；
- valid / invalid conformance fixtures；
- Redline 参考生产器；
- 可重复的机器基准和原始报告。

但至少还缺：

1. 第二个独立实现；
2. Annotation、WorkOrder、Project Profile 的 Schema 与安全用例；
3. 50 份以上真实文件，覆盖复杂表格、扫描 PDF、批注、图像、公式和损坏文件；
4. 同一任务的传统“上传全文聊天”对照实验；
5. 跨解析器/版本的 selector 保持或明确失效策略；
6. 公开 RFC、版本治理和兼容性承诺。

## 6. 下一轮判定门槛

下一轮不以“功能更多”为成功，而以这些数据门槛为成功：

- conformance 正反例通过率 100%；
- 同一生产器、同一版本、同一参数的锚点稳定率 100%；
- 50 份真实任务的上下文 bytes/token 中位减少至少 70%；
- 局部上下文下 Agent 任务成功率不低于发送全文；
- 陈旧原件误接受率 0，源文件覆盖次数 0；
- DOCX/PDF/PPTX/XLSX 各至少 10 份，与 Docling/MarkItDown 同机输出做元素召回、
  表格保持、位置 provenance、耗时和内存对照；
- 至少一个非 Redline 消费者只依靠公开 Schema 和 fixtures 完成互操作。

达到这些门槛，才能合理地说“ShadowDoc 是一种更适合人和 AI 修改文档的开放协作规范”；
在此之前，只能说它的核心思想有数据支持，而且方向值得继续做。
