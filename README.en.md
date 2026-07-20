# Redline

[简体中文](./README.md) · **English**

**A universal document preview & markup layer, built for the age of AI.**

Open any document or image without installing the original software (Office, WPS, AutoCAD, Photoshop…). A human circles, draws arrows, and writes notes on top; an AI agent (Claude Code, Codex, Hermes, OpenClaw… anything) reads those annotations and edits the real source file. **Redline never writes back to the source file itself** — it only "sees + marks + forwards." Actually editing the file is the agent's job.

Comparable to [Cowart](https://github.com/zhongerxin/cowart) (an image-canvas annotation tool for Codex), but generalized to any document format and **host-agnostic** (not tied to one specific agent or host app).

## Status

`packages/redline-core` is the only package, still in incubation. Its first real host is [OpenCodex](https://github.com/dongsheng123132/opencodex), which references this repo via a `link:` dependency rather than vendoring it in.

- ✅ Universal document model + host-adapter interface (`RedlineHost`; the core touches no platform API)
- ✅ Lazy-loaded viewers for 11 formats: image / HTML / text / PDF / Word / Excel / PPTX outline / ZIP / PSD / 3D (stl·obj·gltf·glb) / DXF
- ✅ Lightweight SVG annotation layer (box / arrow / pen / note); annotation coordinates are fractional (0–1), independent of window zoom
- ⏳ `packages/redline-mcp`: wrap the same core as an MCP Server so external agents can call it directly (without going through a specific host app) — next up

## Architecture

```
redline-core                  host-agnostic universal core (zero Tauri/Electron deps)
├── document-model.ts         unified abstraction: {docId, units:[{page/layer/sheet…}], text, bbox}
├── host-adapter.ts           RedlineHost interface — read bytes / save annotations / send to agent /
│                             open external program; each host implements it once, core code
│                             never changes across hosts
├── viewers/                  extension-based lazy-loaded viewer registry
└── annotation/              SVG annotation layer + persistence hook
```

Each host only implements the `RedlineHost` interface, then mounts `<RedlinePanel host={...} path={...} fileName={...} />`. OpenCodex's implementation lives in its own repo at `src/opencodex/redline-host-tauri.ts` and can serve as a reference.

## License

Apache-2.0 — includes a patent grant, so other agent projects or commercial products can embed it with confidence.

## Why not ship it as a standalone app

We went through this decision: initially we wanted a standalone desktop app + MCP Server, then converged on "no shell needed — first prove it can be used by a real host." A standalone app is a matter for later, not for now.
