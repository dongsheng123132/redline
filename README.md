# Redline

**简体中文** · [English](./README.en.md)

**面向 AI 时代的万能文档预览标记层。**

打开任意文档/图片/压缩包不需要装原软件（Office、WPS、AutoCAD、Photoshop、WinRAR……），人在上面圈选、画箭头、写批注，选一个 AI agent（Claude Code、Codex……）据此去改，产出**新文件**放在右边逐处对照。不满意就继续标注、再派一次。

**Redline 自己从不写回源文件**，也不许 agent 写。这条纪律不是保守，是三栏布局能成立的前提：右栏不是「改过的原文」，是另一个文件——所以左右两栏其实是同一个预览组件开在两个路径上。

对标 [Cowart](https://github.com/zhongerxin/cowart)（给 Codex 用的图片画布标注工具），但通用于任何格式、宿主无关。

## 产品方向：AI 时代的 WinRAR

Redline 最终不是只给某一种文档做批注，而是成为 Windows 上的**万能打开、预览和修改入口**：

- 双击文件、拖入文件夹或从右键菜单打开，不要求电脑预装原软件；
- 统一预览文档、图片、表格、演示、压缩包、设计稿、模型、代码和文件夹；
- 人直接在预览上圈选、画箭头、写“这里怎么改”；
- 中间选择本机或远程 AI agent，由它处理真实文件；
- 右边立即预览新产物和结构化差异，决定接受、继续修改或换一个 agent。

WinRAR 解决的是“电脑能打开压缩包”；Redline 要解决的是 AI 时代更大的问题：**任意文件都能打开、看懂、指出哪里要改，并把修改交给 AI 完成。**

### 无限修改模式

三栏不是固定死的整个产品，而是无限画布上的**一次修改回合**：

```text
版本 A（预览、标注） → AI 动作（agent + 要求） → 版本 B（结果预览、差异）
                                                   │
                                                   ├→ AI 动作 → 版本 C
                                                   └→ 换 agent → 版本 D

版本 A ──回到这里重新标注──→ 另一条 AI 动作 → 版本 E
```

一次回合完成后，版本 B 会成为下一回合的左侧输入，画布继续向右生长；用户可以沿用原 agent，也可以换另一个 agent。中间的 AI 动作卡可折叠、拖动或向左收起，把空间让给前后两个版本的对照预览。

用户也可以把画布向左拖回任一历史版本，从那个版本重新标注并发起修改。此时创建的是**新分支**，不是回滚覆盖：旧版本、旧产物、旧批注和当时使用的 agent 都保留。横向表示连续修改，纵向表示从同一父版本产生的不同方案；画布支持平移、缩放和无限扩展。

每个版本节点都是不可变快照，每条连线都记录一次可审计的 AI 动作：

- 父版本及其内容哈希；
- 标注、整体要求和被选中的区域；
- 使用的 agent、模型和权限说明；
- 输出文件、运行状态和与父版本的差异；
- 创建时间、分支名称及用户最终选择。

因此“继续改”永远是基于某个明确版本生成一个新版本，源文件不会被静默覆盖。接受结果时再由用户选择导出、另存为或替换；历史画布本身仍可复查、分叉和复用。

第一阶段仍用当前三栏工作台把**单次回合**做扎实；下一阶段再把多个回合串成版本图和无限画布。动作核心继续只实现一次，GUI、CLI、MCP 和未来的文件关联都调用同一套版本/派发动作。

## 现状

```
redline/
├── crates/redline-core/     ★ 动作核心 —— 业务动作只实现一次（Rust，无界面）
├── crates/redline-cli/        CLI 面（薄壳，零业务逻辑）
├── apps/redline-desktop/      GUI 面：独立 Tauri 壳 + 三栏工作台 → 发布的单 exe
├── packages/redline-core/     渲染层：11 种格式的 viewer + SVG 标注层（TS）
├── scripts/                   格式表漂移检查
└── bin/redline.py             ⚠️ 已废弃，见下文
```

- ✅ **动作核心**：格式注册表、inspect/diff/apply、压缩包 list/extract、agent 派发
- ✅ **CLI**：`redline inspect / diff / apply / verify / formats / actions / archive / agents / send / call`
- ✅ **GUI**：三栏工作台，Windows 单 exe + NSIS 安装包
- ✅ **11 种格式懒加载 viewer**：图片/HTML/文本/PDF/Word/Excel/PPTX大纲/ZIP/PSD/3D/DXF
- ✅ **SVG 标注层**：框/箭头/画笔/批注，分数坐标（0~1），跟窗口缩放无关
- ⏳ `redline-mcp`：把同一个 dispatch 包成 MCP Server，让外部 agent 直接调
- ⏳ **rar/7z 解包**：格式表已认这些扩展名并给出正确提示，解包后端（进程外 7z）还没接
- ⏳ **文件关联 / 右键菜单 / 托盘**：单 exe 已支持 `redline <文件>` 直接打开，注册表那一步还没做

## 一条规矩：业务动作只实现一次

影核协议（ActionParity）第 13 条。`crates/redline-core` 是唯一实现，三个界面都是它的**调用方**：

```
             ┌─ GUI    (Tauri command `redline_call`)
dispatch() ──┼─ CLI    (redline call / 各子命令)
             └─ MCP    (计划中)
```

`redline actions` 打印的就是那张绑定清单。任何一个界面想「自己再算一下」都会在这张表上露馅。

这不是洁癖：这个项目**已经漂过一次**——TS 的注册表把 `.doc` 当 docx 渲染（打开老文档看到乱码，然后以为是 Redline 坏了），Python CLI 那份明确拒绝。漂移不会自己报错，只会在某个用户打开一个 `.doc` 时暴露。现在 Rust 是唯一真相源，`node scripts/check-format-parity.mjs` 守着两边一致。

## 用

### 单 exe

`redline.exe` 一个二进制同时是 GUI 和 CLI，按第一个参数分流：

| 怎么调 | 走哪边 |
|---|---|
| `redline` / 双击 | GUI，空白工作台 |
| `redline C:\包.zip` | GUI，直接打开这个文件（文件关联走这条） |
| `redline inspect 方案.docx` | CLI |

> 交互式在 cmd/PowerShell 里敲命令、想直接在屏幕上看输出，用同时构建出来的 `redline-cli.exe`（console 子系统）。管道和重定向两个都正常。

### CLI

```bash
redline inspect 方案.docx                 # 解析成结构化快照（只读）
redline diff 原件.docx 改后.docx          # 按 unit 对齐的结构化差异
redline archive list 包.zip               # 列条目，不解包
redline archive extract 包.zip ./解出     # 解包，zip-slip 一律拒绝
redline agents                            # 有哪些 AI agent，装没装
redline send 方案.docx --agent codex --output 改后.docx \
  --note "paragraph:2=标题改得更正式" --instruction "整体语气正式"
redline apply 方案.docx patch.json 待审.docx --author Codex
```

给机器用的约定：

- **stdout 只出结果**，日志走 stderr；**非 TTY 自动切 JSON**，不带 ANSI
- **`ok` 是稳定成功位**，成功失败同一个信封形状
- **退出码**：`0` 成功 / `1` 输入错误 / `2` 安全拒绝 / `3` 内部错误 / `64` 用法错误

`2` 单独占一个码是有意的：安全拒绝不是 bug，脚本不该重试，该让人来看。

### 安全约束（在核心里强制，不靠调用方自觉）

- `apply` 只处理 `.docx`——只有 Word 有原生 Track Changes 能承载修订痕迹
- 输出必须是新文件，路径不能等于输入；已存在的输出没有 `--force` 不覆盖
- `expected_input_sha256` 是状态版本号，原件在出 patch 之后被改过就拒绝执行，**不允许「最后写入者获胜」当默认**
- 替换点必须唯一命中，命中 0 次或 2 次以上一律拒绝
- 解包分两趟：先把全部条目路径验一遍，一条不合格就整包拒绝，**零字节残留**
- 老二进制格式（`.doc`/`.xls`/`.ppt`）明确拒绝而不是硬解——不伪造「无损可编辑」

### 派给 AI 时的权限

派发用的完整命令行和授权说明会原样出现在返回结果里（`command` / `permissionNote`），不藏在代码里替用户做决定：

| agent | 命令 | 授了什么 |
|---|---|---|
| Claude Code | `claude -p --permission-mode acceptEdits` | 工作目录内改文件，不能跑任意命令 |
| Codex CLI | `codex exec --sandbox workspace-write --skip-git-repo-check -` | 工作目录内改文件；跳过 git 仓库检查（文档目录通常不是 git 仓），沙箱没放松 |

提示词走 **stdin** 不走命令行参数：Windows 上这俩都是 npm 装的 `.cmd` shim，参数要过 cmd.exe 再解析一遍，换行和 `& | "` 会被吃掉；而且命令行总长约 32KB，长文档的工单撑得爆。

「成没成」看产物存不存在，不看退出码——agent 可以退出码 0 但什么都没写。

## 开发

```bash
cargo test --workspace                    # 动作核心的单测
node scripts/check-format-parity.mjs      # 两份格式表有没有漂
pnpm install
pnpm --filter redline-desktop tauri dev   # 起 GUI
pnpm --filter redline-desktop tauri build # 出 exe + NSIS 安装包
```

## 协议

Apache-2.0——带专利授权条款，方便被其他 agent 项目或商业产品放心嵌入。

依赖只用宽松协议。rar/7z 走**进程外**调用 7z 可执行文件，不把 unRAR license 的代码拖进二进制。

## bin/redline.py 已废弃

那是 Rust 核心之前的第一版 CLI（Python）。它和 TS 渲染层各自实现了一遍文档解析，正是本项目要消灭的那种漂移。功能已全部迁进 `crates/redline-core`，且不再需要目标机器装 Python。

文件暂时保留供对照，**新代码不要再引用它**。

## 为什么最早说「不做独立 app」，现在做了

早期收敛过一次：先证明内核能被真实宿主（OpenCodex）用起来，独立 app 以后再说。

推翻它的理由很具体：**寄生在别人的宿主里，永远拿不到「双击一个 zip 就打开」这个入口**，而那正是 WinRAR 的全部护城河。要取代它就得有自己的 exe、文件关联和右键菜单。内核仍然宿主无关，OpenCodex 那条路没断。
