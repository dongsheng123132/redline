# The ShadowDoc Manifesto

> Status: Non-normative Working Draft, 2026-07-31.
> This manifesto explains why ShadowDoc exists. When it conflicts with the normative specification,
> the specification wins.

## A file should be ready for work, not merely ready to open

Files still arrive as Word documents, spreadsheets, presentations, PDFs, images, archives and many
other formats. People increasingly do not want to learn every authoring application before they can
understand or change those files. They want to preview the content, point at the exact thing that
needs attention, ask an AI agent to help, verify the result and continue from there.

Today those steps are fragmented. A viewer sees pixels, an agent receives copied text, a comment
refers to a location that may move, and a generated result can lose its relationship to the source.
ShadowDoc exists to preserve that relationship.

**ShadowDoc turns an immutable source file into one or more rebuildable, addressable and verifiable
shadow layers, so humans and AI agents can work on the same intent without silently rewriting the
source.**

It is not a new office file format. It is a companion layer for existing and future formats.

## The collaboration loop

```text
immutable source
  → rebuildable shadow layers
  → human or machine annotations
  → bounded, model-neutral work order
  → new output file
  → verified version node
  → continue, compare or branch
```

The source remains the authority. A shadow makes it easier to see, address and process. An
annotation records intent. An agent produces a candidate. A new file records the result. History can
extend to the right indefinitely or branch from any earlier version without pretending there is only
one final truth.

## Seven principles

### 1. The source is the authority

A source artifact is identified by its bytes, not merely by its filename or path. Implementations
must not silently overwrite it. If the bytes change, the identity changes.

### 2. Shadows are useful because they are disposable

A shadow layer may contain extracted structure, text, visual coordinates, thumbnails, OCR or
AI-assisted interpretation. It is derived state, not the original. It should declare how it was made
and be rebuildable when its producer improves.

### 3. Humans and agents should point to the same thing

A circle on a page, a selected cell, a paragraph and a semantic unit should resolve to explicit,
verifiable anchors. Human intent must survive the handoff to an agent without becoming an ambiguous
prompt fragment.

### 4. Every accepted change creates a new version

Agent output is a candidate artifact, never a silent mutation. Each version should preserve its
source relationship, provenance and verification state. A user can compare, reject, continue or
branch from any version.

### 5. Models and agents are replaceable

Annotations and work orders belong to the user, not to one AI vendor. The same bounded intent should
be portable across local models, hosted models and specialized agents. Changing the worker must not
erase the collaboration history.

### 6. Opening should be fast, local-first and inexpensive

Common files should become useful before an expensive conversion pipeline finishes. Implementations
may reveal deeper structure progressively and call heavyweight parsers, OCR or AI only when the task
needs them. Private source bytes should stay local unless the user explicitly chooses otherwise.

### 7. Claims require interoperable evidence

A promising idea is not yet a standard. Every normative claim should be backed by a schema,
conformance fixture or independently reproducible test. Performance and quality claims should name
their corpus, environment and limitations. A stable release requires independent implementations,
not only a reference product.

## Build with the ecosystem

ShadowDoc should reuse mature parsers, renderers and standard vocabularies wherever they fit. Tools
such as Docling, MarkItDown, PDF engines, office libraries and the W3C Web Annotation model can be
producers, consumers or design inputs. ShadowDoc's job is to preserve source identity, addressable
intent, provenance and version relationships across those components—not to replace all of them.

A useful producer may implement only the small Core Profile. Richer visual, annotation, agent and
project profiles should remain separable so adoption does not require an all-or-nothing platform.

## What ShadowDoc refuses to become

ShadowDoc is not:

- a universal writable AST that promises lossless editing of every format;
- a replacement for Word, PDF, image or archive standards;
- permission for an AI inference to masquerade as source truth;
- a mechanism for silently overwriting originals;
- a protocol owned by one model, agent or application;
- a marketing label called a standard before conformance and independent adoption exist.

These limits keep the core small enough to implement and strict enough to trust.

## The invitation

We invite viewer authors, parser maintainers, agent builders, document-tool developers and users to
challenge the model with real files. Contribute failing fixtures, competing implementations,
security cases, measurements and simpler designs. Compatibility matters more than control, and
evidence matters more than rhetoric.

The long-term goal is simple: **any file can become a safe, shared workspace for a person and their
chosen AI—without giving up the original.**

For the product built on this idea:

> Can't open a file? Start with DieXiang. Point to what should change, then let any AI create the next
> version.
