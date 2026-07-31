# redline-core

[Redline](https://github.com/dongsheng123132/redline) 的**动作核心** —— 无界面，不知道自己被谁调用。

打开任意文档 / 图片 / 压缩包不需要装原软件（Office、WPS、AutoCAD、Photoshop、WinRAR），
把它解析成统一的结构化快照喂给 AI，把人的标注派给 AI agent 去改。

**这个 crate 从不写回源文件。** 唯一写文件的动作只写新文件。

## 用

只有一个入口，返回值永远是一个信封（成功失败同一个形状）：

```rust
use serde_json::json;

let out = redline_core::dispatch("document.inspect", &json!({ "path": "方案.docx" }));
if out["ok"] == true {
    for unit in out["units"].as_array().unwrap() {
        println!("{} {}", unit["id"], unit["text"]);
    }
}
```

也可以直接调类型化的 API：

```rust
let snapshot = redline_core::inspect::inspect("方案.docx")?;
let report = redline_core::diff::diff_files("原件.docx", "改后.docx")?;
let entries = redline_core::archive::list("包.zip")?;
# Ok::<(), redline_core::Error>(())
```

## 动作

| Action ID | 干什么 |
|---|---|
| `document.inspect` | 解析成结构化快照（docx / xlsx / pptx / pdf / text / zip） |
| `document.verify` | 能不能被解析 |
| `document.diff` | 按 unit 对齐的结构化差异 |
| `document.formats` | 格式注册表全文 |
| `document.apply-track-changes` | 把 patch 写成带 Word 修订痕迹的**新文件** |
| `archive.list` / `archive.extract` | 压缩包列条目 / 解包（zip-slip 一律拒绝） |
| `agent.catalog` / `agent.dispatch` | 有哪些 AI CLI / 把标注派给它去改 |

完整参数、返回结构和 30 个错误码见仓库里的
[`docs/动作契约.md`](https://github.com/dongsheng123132/redline/blob/main/docs/动作契约.md)。

## 错误分类

`RedlineError.class` 有三种，对应 CLI 的退出码：

- `input`（1）—— 输入不对，改参数重试
- `refused`（2）—— **核心主动拒绝**：会覆盖原件、原件已变更、替换点不唯一、
  压缩包路径越界。这不是 bug，别自动重试，让人来看
- `internal`（3）—— 解析崩了或 IO 失败

## 无界面保证

依赖树里不含任何 GUI / 窗口 / 浏览器内核的 crate，可以在无头环境、容器、
MCP server 里直接跑。这条由 CI 门禁守着，不是口头承诺。

## 协议

Apache-2.0 —— 带专利授权条款，方便被其他 agent 项目或商业产品放心嵌入。
协议不授予 Redline 的商标使用权。
