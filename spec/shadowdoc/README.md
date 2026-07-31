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

- [ShadowDoc data model](../../docs/影文档协议.md)
- [Open specification and ecosystem plan](../../docs/ShadowDoc开源规范与生态计划.md)
- [Action and code plan](../../docs/功能与代码规划.md)
- [Action contracts](../../docs/动作契约.md)

## Planned incubation artifacts

- normative `SPEC.md`;
- JSON Schemas for Core, Annotation, Agent and Project profiles;
- valid, invalid and security conformance fixtures;
- a machine-readable validator;
- an implementation-report template;
- public RFCs for behavior-changing proposals.

The first reference implementation is Redline. A v1.0 release requires at least two independent
implementations using the same public conformance suite.

## Licensing status

Reference software, schemas, validators and conformance code remain under Apache-2.0.
The license and patent terms for a future normative specification are still under review;
the incubation material must not yet be presented as a ratified open standard.
