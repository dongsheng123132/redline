# AGENTS.md —— 接手 Redline 前必读

> 这个文件是给 AI agent（Codex CLI / Claude Code / 其他）看的**唯一入口**。
> `CLAUDE.md` 只是一行指针，别在那边写内容，会漂。

## 这是什么

**Redline —— 面向 AI 时代的万能文档预览标记层。**

对外工作品牌暂定为 **叠象 Stelora · 万能文件工作台**；在商标、商号和域名正式核验前，
仓库、crate、CLI 和 Action ID 继续使用 Redline。品牌与品类决策见 `docs/产品战略.md`。

打开任意文档/图片/压缩包不需要装原软件（Office、WPS、AutoCAD、Photoshop、WinRAR），
人在上面圈选标注，选一个 AI agent 据此去改，产出**新文件**放在右边逐处对照。

产品目标是**成为每台电脑都装的那个工具**（AI 时代的 WinRAR），
所以「打开」这一层必须免费、干净、快、格式全。商业逻辑见 `docs/商业模型.md`。

## 三条铁律（改任何代码前先确认没违反）

### 1. 业务动作只实现一次

`crates/redline-core` 是唯一实现。GUI / CLI / MCP 都是它的**调用方**：

```
                    ┌─ GUI  →  Tauri command `redline_call`
dispatch(id, params)┼─ CLI  →  redline <子命令> / redline call
                    └─ MCP  →  （待实现）
```

**新增任何业务能力，先在核心加 Action，再在界面加调用方。**
如果你发现自己在界面层写「判断扩展名」「算 diff」「决定能不能覆盖文件」——停下，
那是核心的活。`redline actions` 打印的就是绑定清单，漏登记的动作任何界面都调不到。

反例（真实发生过）：TS 注册表把 `.doc` 当 docx 渲染，Rust 明确拒绝。
用户打开老文档看到乱码，会以为是 Redline 坏了。漂移不会自己报错。

界面动作（切标签、缩放、拖窗口、动画）**不进核心**，也永远不该进。

### 2. Redline 从不写回源文件

- 唯一写文件的动作是 `document.apply-track-changes`，它**只写新文件**
- agent 派发时，工单里写死了「读 SOURCE、写 OUTPUT、绝不改 SOURCE」，OUTPUT 由核心指定
- 解包只写目标目录，原压缩包不动

这不是保守，是三栏布局能成立的前提：右栏不是「改过的原文」，是**另一个文件**，
所以左右两栏其实是同一个预览组件开在两个路径上。破了这条，整个交互就塌了。

### 3. 安全拒绝是功能，不是错误

核心主动拒绝的场景（`ErrorClass::Refused`，退出码 2）：

| 场景 | 错误码 |
|---|---|
| 输出路径 = 输入路径 | `output_would_overwrite_input` |
| 输出已存在且没给 `--force` | `output_exists` |
| 原件在出 patch 之后被改过 | `stale_patch` |
| 替换点命中 0 次或 ≥2 次 | `ambiguous_replacement` |
| 压缩包里有 `../` 之类越界路径 | `unsafe_archive_path` |
| 解包目标已存在同名文件 | `would_overwrite` |
| 老二进制格式（.doc/.xls/.ppt） | `legacy_binary_format` |

**不要为了"用户体验好"把这些改成自动处理。** 每一条都有具体理由，见 `docs/决策记录.md`。
尤其：`unsafe_archive_path` 是直接拒绝整包，不做「尽力清洗」——清洗过的路径会让
用户以为解出来的东西在他以为的地方。

## 目录导航

```
crates/redline-core/src/
  lib.rs        ★ dispatch() 绑定清单 —— 找任何动作从这里进
  action.rs       Action ID 常量 + 输出信封
  format.rs     ★ 格式注册表（单一真相源）—— 新增格式只改这里
  inspect.rs      docx/xlsx/pptx/pdf/text/archive → 统一快照
  archive.rs      压缩包 list/extract，zip-slip 防护
  diff.rs         按 unit id 对齐的结构化差异
  apply.rs        docx Track Changes（唯一写文件的动作）
  agent.rs        派发给 claude/codex，子进程管理
  ooxml.rs        zip + XML 底座
  paths.rs        路径对外呈现的唯一规则
  error.rs        错误分类 → 退出码

crates/redline-cli/src/lib.rs     CLI 面（薄壳，零业务逻辑）
apps/redline-desktop/
  src-tauri/src/main.rs           单 exe 入口：按参数分流 GUI / CLI
  src-tauri/src/lib.rs            Tauri command：只有 redline_call 一扇门通业务
  src/core.ts                     前端通向核心的唯一一扇门
  src/App.tsx                     三栏工作台
  src/host.ts                     RedlineHost 的 Tauri 实现（宿主能力，非业务）
packages/redline-core/            渲染层（TS）：viewer + SVG 标注层
scripts/check-format-parity.mjs   两份格式表的漂移检查
docs/                             产品战略 / 开源商业化 / 功能代码规划 / 传播计划
                                  影文档协议 / ShadowDoc 开源生态 / 架构 / 动作契约
                                  开发计划 / 商业模型 / 决策记录
spec/shadowdoc/                   ShadowDoc 开放规范孵化入口
bin/redline.py                    ⚠️ 已废弃，别引用，别改
```

## 常用命令

```bash
cargo test --workspace                    # 动作核心单测（改核心必跑）
node scripts/check-format-parity.mjs      # 两份格式表有没有漂（改格式必跑）
cargo build                               # 调试构建
cargo build --release                     # 出 redline.exe / redline-cli.exe

pnpm install
pnpm --filter redline-desktop tauri dev   # 起 GUI（热更新）
pnpm --filter redline-desktop tauri build # 出 exe + NSIS 安装包
cd apps/redline-desktop && npx tsc --noEmit   # 前端类型检查
```

**提交前这三条必须全绿**：`cargo test --workspace`、`node scripts/check-format-parity.mjs`、`npx tsc --noEmit`。

## 已经踩过的坑（别再踩一遍）

1. **Windows 上 `Command::new("codex")` 起不来。** npm 装的 CLI 是 `codex.cmd`
   批处理 shim，Rust 只会去找 `codex.exe`。必须先在 PATH 里按 PATHEXT 解析成完整路径
   再 spawn（`agent.rs::resolve_program`）。症状是「清单说已安装、派发说没装」。

2. **提示词必须走 stdin，不能走 argv。** 过 `.cmd` shim 时参数要被 cmd.exe 再解析一遍，
   换行和 `& | " ^` 会被吃掉或改写；Windows 命令行总长约 32KB，长文档工单撑得爆。

3. **接管控制台时只能补空句柄，不能覆盖已有的。** 无条件把 stdout 指向 `CONOUT$` 会把
   重定向冲掉——`redline formats > out.json` 得到零字节文件、退出码却是 0。

4. **Rust 的 `regex` 不支持 lookahead。** apply 里找 `w:r` 边界是手工扫描的，
   而且 `<w:rPr>` 也以 `<w:r` 开头，边界判断错了会替换错位（有测试守着）。

5. **中文标识符里夹 ASCII 单词要用下划线连**，`fn 只有 docx 支持 apply()` 编译不过，
   得写成 `fn 只有_docx_支持_apply()`。

6. **`packages/redline-core` 有一份只读副本 vendored 在
   `~/Desktop/claude/u-claw/uking-release-0978/src/vendor/redline-core`。**
   改了上游要记得同步那边，否则 OpenCodex 会跟着漂。**永远不要直接改那份副本。**

7. **cmd/PowerShell 里交互式敲 `redline agents` 看不到屏幕输出**，因为单 exe 是 GUI 子系统、
   cmd 不等它。这不是 bug，是固有代价；这种场景用 `redline-cli.exe`。管道和重定向都正常。

## 提交纪律

- **每跑通一个小步就 commit**，别攒大包
- commit message 写**为什么**，不只写做了什么；踩到的坑写进去
- 动核心/接口前先 `git log` + `git status` 看清别的终端干了什么
- 并行改大功能开 `git worktree` 硬隔离
- 绝不在别人可能有未提交改动时跑 `reset --hard` / `checkout` / `clean`

## 接着做什么

看 `docs/开发计划.md`。任务按优先级排好了，每条都带验收标准和边界。
**当前第一优先：文件关联 + 右键菜单**——那是从「一个能跑的 exe」到「每台电脑都有」
之间唯一的硬障碍，整个商业模型都压在它上面。
