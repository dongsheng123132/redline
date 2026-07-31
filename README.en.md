# Redline

[简体中文](./README.md) · **English**

> Working public brand: **Stelora — Universal File Workspace** (`叠象` in Chinese).
> The brand is pending formal trademark, company-name, and domain clearance. The repository,
> CLI, crates, and Action IDs remain Redline until that work is complete.

**A model-neutral universal file workspace for the age of AI.**

> **Can't open it? Try Stelora first.**

Open documents, images, archives, design files, models, code, and folders without first installing
their original applications. Mark the exact area that needs work, choose an AI agent such as Claude
Code, Codex, or a local model, and compare the newly generated file on the right.

Redline never silently writes back to the source file. Every result is a new file, and the planned
version graph turns each AI run into an immutable, auditable branch.

> WinRAR's universal opening + Git's version branches + interchangeable AI agents.

## Product direction

The current three-column workspace represents one revision round:

```text
Version A → annotations + agent → Version B → another agent → Version C
     └──────────────────────────→ alternative Version D
```

The next stage connects these rounds on an infinite canvas. Users can continue to the right, move
back to any earlier version, or create a new branch with another agent. Parent versions, outputs,
annotations, agent identity, permissions, hashes, and diffs remain available.

Claude and Codex are engines, not platform dependencies. Redline owns the operating-system entry
point, format handling, work orders, immutable versions, comparison, and audit trail.

## Current status

- Rust action core: format registry, inspect, diff, docx Track Changes, verify, archive, agent dispatch
- CLI with stable JSON envelopes and classified exit codes
- Tauri desktop app: one Windows executable, NSIS installer, three-column workspace
- Lazy viewers for images, HTML, text, PDF, docx, xlsx, pptx outline, ZIP, PSD, 3D, and DXF
- SVG annotations using normalized coordinates
- End-to-end Codex dispatch verified without changing the source file
- zip-slip rejection verified with no partial extraction residue

Next priorities:

1. File associations and Windows context menus
2. RAR/7z extraction through an external 7z process
3. A trusted signed release chain
4. `redline-mcp` and portable WorkOrders
5. Immutable version graph and infinite canvas
6. Optional metered cloud AI and, later, enterprise controls

## Architecture

```text
GUI ─┐
CLI ─┼── redline_core::dispatch(action_id, params) ── unified envelope
MCP ─┘                         planned
```

Business behavior is implemented once in `crates/redline-core`. GUI, CLI, and MCP are callers.
The GUI may render and provide host capabilities; it must not reimplement format detection, diff,
overwrite policy, or archive safety.

Registered Actions currently include:

- `document.inspect`
- `document.diff`
- `document.apply-track-changes`
- `document.verify`
- `document.formats`
- `archive.list`
- `archive.extract`
- `agent.catalog`
- `agent.dispatch`

## Safety contract

- Output paths must differ from input paths.
- Existing outputs are not overwritten without explicit permission.
- A patch is rejected if the source hash has changed.
- Ambiguous replacements are rejected rather than guessed.
- Unsafe archive paths reject the entire extraction before anything is written.
- Legacy `.doc`, `.xls`, and `.ppt` binaries are rejected instead of misparsed.
- Agent success is determined by the expected output file, not only by process exit code.

## Documentation

| Topic | Document |
|---|---|
| Brand, category, users, and competitive strategy | [Product strategy (Chinese)](./docs/产品战略.md) |
| Revenue model | [Business model (Chinese)](./docs/商业模型.md) |
| Open-source and commercial boundaries | [Open source & commercialization (Chinese)](./docs/开源与商业化.md) |
| Feature-to-code roadmap | [Feature and code plan (Chinese)](./docs/功能与代码规划.md) |
| AI-ready visual and semantic mapping for traditional files | [ShadowDoc protocol (Chinese)](./docs/影文档协议.md) |
| ShadowDoc open-specification and ecosystem path | [ShadowDoc open-spec plan (Chinese)](./docs/ShadowDoc开源规范与生态计划.md) |
| Prioritized implementation plan | [Development plan (Chinese)](./docs/开发计划.md) |
| Distribution and communication | [Go-to-market plan (Chinese)](./docs/传播计划.md) |
| Architecture | [Architecture (Chinese)](./docs/架构.md) |
| Action contracts | [Action contracts (Chinese)](./docs/动作契约.md) |
| Decision log | [Decision log (Chinese)](./docs/决策记录.md) |

## Development

```bash
cargo test --workspace
node scripts/check-format-parity.mjs
pnpm install
pnpm --filter redline-desktop tauri dev
pnpm --filter redline-desktop tauri build
cd apps/redline-desktop && npx tsc --noEmit
```

## License

Apache-2.0. The action core, CLI, planned MCP server, reusable viewers, and desktop app remain open.
Official hosted AI, billing, abuse prevention, and enterprise policy/audit control planes are
separate services. The code license does not grant rights to impersonate the official brand,
signatures, domains, or update channels.
