# ShadowDoc Core 0.1 第二轮实验报告

> 日期：2026-07-31
>
> 目标：修掉第一轮唯一明确失败项，并建立可重复的外部解析器 PK，而不是扩大宣传结论。

## 1. 结论

第二轮得到两个有用结果：

1. 真实 Markdown 从单一全文 unit 改为确定性文本块后，上下文缩减从 **0% 提升到
   98.37%**，10 次重复锚点稳定率仍为 **100%**。
2. 在四份简单受控 DOCX/XLSX/PPTX/PDF 上，Redline 与 MarkItDown 0.1.7 的头尾必含文本
   召回均为 **100%**，三次输出都完全一致；ShadowDoc 输出额外显式绑定源 SHA-256。

这支持“轻核心先秒开并建立协作锚点，复杂文件再调用深度适配器”的分层路线。它仍不证明
Redline 的解析质量超过 MarkItDown 或 Docling。

机器可读结果：
[`redline-core-0.1-round2-windows-x64.json`](../spec/shadowdoc/reports/redline-core-0.1-round2-windows-x64.json)。

## 2. 修复第一轮反例

第一轮的真实 Markdown 是整个文件一个 `document:1`，选择一个 unit 等于发送全文。

第二轮核心按两个确定性规则生成 `document:1..N`：

- 空行作为人的自然文本块边界；
- 没有空行的超长 JSON/日志按 4,000 Unicode 字符上限切分。

分块只在 Rust 动作核心实现。TextViewer 不复制算法，只显示核心给出的 active unit，因而
不会出现“AI 锚到块 61，但人的标注画在整篇全文上”的视觉/语义漂移。

| 指标 | 第一轮 | 第二轮 |
|---|---:|---:|
| units | 1 | 122 |
| 全量 units JSON | 20,657 bytes | 39,769 bytes |
| 目标 + 前后各 1 unit | 20,657 bytes | 649 bytes |
| 上下文缩减 | 0% | **98.37%** |
| 10 次锚点稳定率 | 100% | 100% |
| release inspect 中位数 | 0.225 ms | 0.757 ms |

代价也很明确：完整 ShadowDoc JSON 因每块都有 ID、label 和 hash，从 20,657 bytes 增至
39,769 bytes。收益来自**按需发送少量块**，如果每次仍把全部 ShadowDoc 发给模型，就失去
优化意义。

## 3. 公平 PK 工具

新增 runner 对同一个本地源文件分别启动：

- release `redline-cli document.inspect`；
- MarkItDown Python `convert_local`，禁用插件和云端/LLM增强；
- Docling `DocumentConverter`，存在时输出 Markdown 和 `export_to_dict()`。

每个适配器都使用无 shell 子进程、相同源字节、相同超时和相同必含文本；缺失依赖报告
`skipped`，解析错误报告 `failed`。每个 case 新起进程重复三次，记录中位数、范围和输出
hash 是否一致。

第一版受控 XLSX/PPTX 只包含 Redline 测试所需 XML，MarkItDown 正确拒绝缺少
`[Content_Types].xml` 的包。我们没有把它记成竞品失败，而是修正生成器，补齐 OOXML
Content Types、relationships、presentation/workbook 和 layout/master 部件后重测。

这件事反而证明 PK 工具是有用的：它先发现了**我们的测试语料不公平**。

## 4. Redline 与 MarkItDown 0.1.7

下表是新进程冷路径中位数，包含 Rust/CLI 或 Python 解释器和依赖启动时间。每格都重复三次。

| 格式 | Redline | MarkItDown | 冷进程倍率 | 两者必含文本召回 |
|---|---:|---:|---:|---:|
| DOCX | 22.958 ms | 1,259.570 ms | 54.86× | 100% / 100% |
| XLSX | 30.696 ms | 1,625.403 ms | 52.95× | 100% / 100% |
| PPTX | 23.989 ms | 1,169.418 ms | 48.75× | 100% / 100% |
| PDF | 24.770 ms | 1,224.939 ms | 49.45× | 100% / 100% |

两者四个 case 都是 3/3 成功，输出 hash 三次相同。这个结果只能说明：

- 对当前简单受控文件，Redline 的桌面冷启动路径明显更轻；
- MarkItDown 同样完整抽到了头尾文本，并非“慢但漏内容”；
- `sourceHashBound=true` 只出现在 ShadowDoc，因为源绑定是协议要求；MarkItDown 输出 Markdown
  没有内建这层，但完全可以由上层包装，不能把它说成 MarkItDown 的安全漏洞。

不能从这组数据推导：

- Redline 的 PDF 阅读顺序、表格、公式、图片或 OCR 更好；
- Rust 解析算法本身比 MarkItDown 快 50 倍；
- MarkItDown 不适合后台批处理——它可以在一个常驻 Python 进程内复用；
- 输出 bytes 更多或更少就代表质量更高。

[MarkItDown 官方说明](https://github.com/microsoft/markitdown)将其定位为面向 LLM 文本分析的
轻量 Markdown 转换器，支持 PDF、Word、PowerPoint、Excel 等格式；官方也建议对本地文件
使用范围更窄的 `convert_local`，本实验遵循了这一点。

## 5. Docling 为什么没有数据

本机使用独立 `target/shadowdoc-docling-venv` 执行一次 `pip install docling`，在唯一的
900 秒上限内未完成；终止后没有 Docling package metadata，因此没有运行转换。

结论必须写成：

> **Docling 本轮 `not_run`，不是失败，也不是质量较差。**

这次环境准备成本支持“Docling 作为可选深解析进程，而非默认轻内核依赖”的工程判断，但不能
代替质量实验。Docling 官方已经提供多格式、先进 PDF 结构理解、OCR、表格和 lossless JSON，
这些正是 Redline 当前没有、未来应该复用的能力：
[Docling 官方仓库](https://github.com/docling-project/docling)。

PK runner 已支持 `--docling-python` 指向独立环境；网络更快的机器无需改代码即可补齐第三列。

## 6. 当前产品判断

现在更清晰的三层不是竞争关系：

```text
毫秒级内建 parser
  → 先打开、建立 source hash 和稳定 units
  → MarkItDown：轻量扩格式/Markdown 影子
  → Docling：复杂 PDF、OCR、表格、视觉 provenance 深解析
```

ShadowDoc 的价值不是成为第四个解析器，而是让三种解析结果都进入同一个协作关系：

```text
源文件身份 → 可验证 units → 人工标注意图 → Agent 工单 → 新文件版本
```

## 7. 下一轮必须换真实文件

受控语料只验证管线和头尾文本，没有复杂版式。下一轮应收集至少 50 份可公开或脱敏文件：

- 每种 DOCX/PDF/PPTX/XLSX 至少 10 份；
- 复杂表格、合并单元格、公式、批注、图片、扫描页、双栏 PDF、页眉页脚；
- 人工标注应出现的文本、顺序、表格结构和页码/bbox；
- 测 extraction recall、reading order、table fidelity、峰值内存、常驻进程耗时；
- 最后才做“传统上传全文 vs ShadowDoc 局部工单”的真实 Agent 修改成功率。

只有真实任务成功率不下降且上下文/交互显著减少，才能继续向“AI 时代最佳人机文档协作规范”
推进。
