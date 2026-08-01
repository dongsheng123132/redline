# redline-cli

[Redline](https://github.com/dongsheng123132/redline) 的 CLI 面 —— `redline-core` 的**薄壳**，零业务逻辑。

打开任意文档 / 压缩包不需要装原软件，解析成 AI 能读的结构化快照，把标注派给 AI agent 去改。

```bash
redline inspect 方案.docx                 # 结构化快照（只读）
redline diff 原件.docx 改后.docx          # 按 unit 对齐的差异
redline archive list 包.zip               # 列条目，不解包
redline archive extract 包.zip ./解出     # 解包，zip-slip 一律拒绝
redline agents                            # 有哪些 AI agent，装没装
redline send 方案.docx --agent codex --output 改后.docx --note "paragraph:2=改得更正式"
```

## 给机器用的约定

- **stdout 只出结果**，日志走 stderr
- **非 TTY 自动切 JSON**，管道里永远可解析、不带 ANSI
- **`ok` 是稳定成功位**，成功失败同一个信封形状
- **`execution_id` 可追踪到动作核心**，成功 payload 固定在 `result`
- **退出码**：`0` 成功 / `1` 输入错误 / `2` **安全拒绝** / `3` 内部错误 / `64` 用法错误

`2` 单独占一个码：安全拒绝不是 bug，脚本不该重试，该让人来看。

完整契约见 [`docs/动作契约.md`](https://github.com/dongsheng123132/redline/blob/main/docs/动作契约.md)。

Apache-2.0。协议不授予 Redline 的商标使用权。
