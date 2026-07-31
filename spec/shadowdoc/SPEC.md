# ShadowDoc Core Profile 0.1

> Status: Working Draft, 2026-07-31.
> This document is testable, but it is not a ratified standard.

ShadowDoc Core is a model-neutral, deterministic semantic projection of an immutable source
artifact. It lets a human viewer and an AI agent refer to the same bounded units without treating
the projection as the authoritative file.

The key words **MUST**, **MUST NOT**, **SHOULD**, **SHOULD NOT**, and **MAY** are to be interpreted as
described in RFC 2119 and RFC 8174.

## 1. Scope

Core 0.1 defines only:

- a source identity;
- one semantic projection descriptor;
- addressable text units;
- deterministic hashes and conformance rules.

It does not yet define visual coordinates, annotations, work orders, version graphs, OCR claims,
AI-generated summaries, or a writable universal document AST. Those are separate profiles so that
an implementation can adopt the small useful core without adopting the whole system.

## 2. Data model

A conforming document is a JSON object matching
[`schema/core.schema.json`](./schema/core.schema.json). It MAY appear by itself or inside an
ActionParity success envelope. Envelope fields are not part of the semantic identity.

### 2.1 Source identity

`source.sha256` MUST be the lowercase SHA-256 of the exact source bytes. `source.bytes` MUST be the
byte length of the same sequence.

`shadow.sourceSha256` MUST equal `source.sha256`. A consumer MUST reject a projection when they
differ. A source modified after inspection is a different source artifact even when its path is
unchanged.

The path is descriptive provenance, not identity. Consumers MUST NOT use the path alone to decide
that a projection still belongs to a file.

### 2.2 Projection descriptor

Core 0.1 has:

- `schema = "redline.shadow-document"`;
- `schemaVersion = 1`;
- `profile = "semantic"`;
- one declared `granularity`;
- a non-empty `producer`;
- an honest `lossy` declaration.

`lossy: false` would claim that the projection preserves every relevant source fact. The Redline
0.1 reference producer does not make that claim and emits `true` for every current format.

The `format` and `granularity` pair MUST follow this table:

| format | granularity |
|---|---|
| `docx` | `paragraph` |
| `xlsx` | `sheet` |
| `pptx-outline` | `slide` |
| `pdf` | `page` |
| `text`, `html` | `document` |
| `archive`, `archive-external` | `entry` |

### 2.3 Units

Each unit MUST contain:

- a unique `id` in `<kind>:<positive integer>` form;
- a `kind` equal to the `id` prefix;
- a human-readable `label`;
- Unicode `text`;
- `textSha256`, the lowercase SHA-256 of the UTF-8 encoding of `text`.

Unit order is significant. Given identical source bytes, producer version and parameters, a
producer MUST return the same ordered `(id, kind, textSha256)` sequence. This is the Core 0.1
definition of deterministic anchoring.

An empty `units` array is valid for an empty document or archive. A `document` granularity does not
require exactly one unit: text producers MAY emit ordered `document:1..N` blocks. If they do, the
segmentation algorithm and size limit MUST be deterministic for the same producer version and
parameters.

## 3. Consumption rules

A strict consumer MUST validate before using anchors. It MUST reject at least:

1. an unknown schema or version;
2. an invalid or mismatched source hash;
3. a duplicate unit ID;
4. a unit whose kind does not match its ID;
5. a unit whose text hash is invalid;
6. a format/granularity mismatch.

A consumer MAY preserve unknown properties and SHOULD do so when forwarding a ShadowDoc object.
An extension MUST NOT change the meaning of a Core field.

Selecting a subset of units is allowed and is the intended AI-context optimization. The work order
or annotation carrying that subset MUST still retain the source hash and the selected unit IDs and
text hashes.

## 4. Producer rules

A producer:

- MUST read the source without modifying it;
- MUST compute identity from bytes, not timestamps;
- MUST identify itself and version;
- MUST emit deterministic unit order and IDs for the same input and parameters;
- MUST mark uncertain or missing content explicitly rather than inventing source facts;
- MUST NOT label OCR or AI inference as source-derived Core text without provenance.

Core 0.1 does not require two different producers to create identical segmentation. Cross-producer
portability is identified as an open issue for 0.2; consumers SHOULD retain producer identity when
persisting annotations.

## 5. Security and privacy

A validator does not prove that the referenced source bytes exist, are safe to open, or contain no
malicious active content. Producers SHOULD parse untrusted files in a constrained process and MUST
apply archive path traversal protections before extraction.

ShadowDoc text can disclose the source contents. Implementations MUST apply the same access control,
retention and transmission policy to the projection as to the source. Source paths can contain
personal information and SHOULD be redacted when publishing fixtures or telemetry.

## 6. Conformance

The command:

```text
node scripts/shadowdoc-validate.mjs FILE --json
```

is the reference validator. The fixtures under `conformance/` are the public minimum suite.

A producer may claim **ShadowDoc Core 0.1 producer conformance** only if:

- every emitted document passes the reference validator;
- identical input is stable across at least ten repeated inspections;
- its implementation report names the producer version, platform, corpus and known loss modes.

A consumer may claim **ShadowDoc Core 0.1 consumer conformance** only if it accepts every valid
fixture and rejects every invalid fixture with a machine-readable violation.

## 7. Versioning

Schema version 1 is additive: producers MAY add properties, while consumers MUST ignore unknown
properties they do not understand. Removing a field, changing its meaning, changing a hash
algorithm, or relaxing a safety rejection requires a new schema version.

Normative behavior is determined by this file, the JSON Schema and conformance fixtures. If they
conflict during incubation, the conflict is a specification bug and MUST be resolved before any
stable release.
