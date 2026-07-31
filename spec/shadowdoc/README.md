# ShadowDoc Specification

> Incubation draft. This directory does not yet define a stable standard.

ShadowDoc is an open, model-neutral way to describe derived views of an immutable source artifact,
anchor human intent to those views, hand a bounded work order to an AI agent, and record every output
as a new verifiable file version.

The intended loop is:

```text
immutable source
  → source-bound Shadow Layers
  → human annotation
  → model-neutral WorkOrder
  → new standard file
  → verified VersionNode
  → continue or branch
```

ShadowDoc is a companion specification to
[ActionParity](https://github.com/dongsheng123132/action-parity), not a required extension of it:

- ShadowDoc specifies document/artifact nouns, selectors, trust and provenance;
- ActionParity specifies canonical application actions and their interface bindings;
- an implementation may adopt either specification independently;
- a combined implementation uses ActionParity actions to produce and consume ShadowDoc objects.

## Current material

- [Normative Core 0.1 Working Draft](./SPEC.md)
- [Core 0.1 JSON Schema](./schema/core.schema.json)
- [Conformance fixtures](./conformance/manifest.json)
- [Reference validator](../../scripts/shadowdoc-validate.mjs)
- [First implementation report](../../docs/ShadowDoc实验报告-v0.1.md)
- [Second-round text segmentation and parser PK report](../../docs/ShadowDoc实验报告-v0.1-第二轮.md)
- [Controlled parser PK corpus and runner](./benchmark/README.md)
- [ShadowDoc data model](../../docs/影文档协议.md)
- [Open specification and ecosystem plan](../../docs/ShadowDoc开源规范与生态计划.md)
- [Action and code plan](../../docs/功能与代码规划.md)
- [Action contracts](../../docs/动作契约.md)

## Incubation status

- Core Profile: implemented as a testable Working Draft;
- Annotation, Agent and Project Profiles: planned;
- security corpus beyond hash mismatch and stale-source refusal: planned;
- implementation-report template and public RFC process: planned.

The first reference implementation is Redline. A v1.0 release requires at least two independent
implementations using the same public conformance suite.

Run the current suite:

```text
node --test scripts/shadowdoc-validate.test.mjs
node --test scripts/shadowdoc-pk.test.mjs
cargo run --release -p redline-core --example shadowdoc_benchmark
```

## Licensing status

Reference software, schemas, validators and conformance code remain under Apache-2.0.
The license and patent terms for a future normative specification are still under review;
the incubation material must not yet be presented as a ratified open standard.
