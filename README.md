# Redline

**面向 AI 时代的万能文档预览标记层。**

打开任意文档/图片不需要装原软件（Office、WPS、AutoCAD、Photoshop……），人在上面圈选、画箭头、写批注，AI agent（Claude Code、Codex、Hermes、OpenClaw……任何东西）读懂这些标注去改真正的源文件。**Redline 自己从不写回源文件**——它只负责"看见 + 标记 + 转发"，改文件是 agent 自己的事。

对标 [Cowart](https://github.com/zhongerxin/cowart)（给 Codex 用的图片画布标注工具），但通用于任何文档格式、宿主无关（不绑定某一个 agent 或某一个宿主 app）。

## 现状

`packages/redline-core` 是唯一的包，正在孵化期。第一个真实宿主是 [OpenCodex](https://github.com/dongsheng123132/opencodex)（通过 `link:` 依赖引用本仓库，不是 vendor 进它自己仓库）。

- ✅ 通用文档模型 + 宿主适配器接口（`RedlineHost`，核心不碰任何平台 API）
- ✅ 11 种格式的懒加载 viewer：图片/HTML/文本/PDF/Word/Excel/PPTX大纲/ZIP/PSD/3D(stl·obj·gltf·glb)/DXF
- ✅ 轻量 SVG 标注层（框/箭头/画笔/批注），标注坐标是分数坐标（0~1），跟窗口缩放无关
- ⏳ `packages/redline-mcp`：把同一套内核包成 MCP Server，让外部 agent（不经过某个具体宿主 app）也能直接调用——下一步

## 架构

```
redline-core                  宿主无关的通用内核（零 Tauri/Electron 依赖）
├── document-model.ts         统一抽象：{docId, units:[{page/layer/sheet…}], text, bbox}
├── host-adapter.ts           RedlineHost 接口——读字节/存标注/发给agent/打开外部程序，
│                             宿主各自实现一遍，内核代码换宿主不用改
├── viewers/                  按扩展名懒加载的 viewer registry
└── annotation/                SVG 标注层 + 持久化 hook
```

每个宿主只需要实现 `RedlineHost` 接口，然后挂 `<RedlinePanel host={...} path={...} fileName={...} />`。opencodex 的实现在它自己仓库的 `src/opencodex/redline-host-tauri.ts` 里，可以当参考。

## 协议

Apache-2.0——带专利授权条款，方便被其他 agent 项目或商业产品放心嵌入。

## 为什么不做成一个独立 app

做过这个决定的讨论过程：先想做独立桌面 app + MCP Server，后来收敛成"不需要壳子，先证明自己能被真实宿主用起来"。独立 app 是以后的事，不是现在的事。
