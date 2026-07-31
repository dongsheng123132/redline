# ShadowDoc parser PK corpus

This is a reproducible comparison harness, not a claim that all parser outputs have the same purpose.

Generate the controlled files outside Git:

```text
cargo run --release -p redline-core --example shadowdoc_benchmark -- --corpus-dir target/shadowdoc-pk-corpus
cargo build --release -p redline-cli
node scripts/shadowdoc-pk.mjs spec/shadowdoc/benchmark/controlled.manifest.json
```

The runner invokes every adapter as a child process with no shell:

- Redline uses the canonical `document.inspect` action through `redline-cli`;
- MarkItDown uses Python `convert_local`, with plugins and network-backed AI features disabled;
- Docling uses `DocumentConverter` and records Markdown plus `DoclingDocument.export_to_dict()`.

The current machine may not have optional Python packages. Missing adapters are reported as `skipped`;
they are never counted as failed extraction or silently replaced with a different implementation.

Install competitors only in an isolated virtual environment, then point the runner to that Python:

```text
python -m venv target/shadowdoc-markitdown-venv
target\shadowdoc-markitdown-venv\Scripts\python.exe -m pip install "markitdown[docx,pptx,xlsx,pdf]"
python -m venv target/shadowdoc-docling-venv
target\shadowdoc-docling-venv\Scripts\python.exe -m pip install docling
node scripts/shadowdoc-pk.mjs spec/shadowdoc/benchmark/controlled.manifest.json --markitdown-python target\shadowdoc-markitdown-venv\Scripts\python.exe --docling-python target\shadowdoc-docling-venv\Scripts\python.exe
```

The checked-in manifest runs three fresh child processes and reports median/min/max duration plus
output determinism. Process startup is deliberately included for all adapters. A publishable product
benchmark still needs pinned versions, warm in-process runs, a real corpus and peak-memory collection.

`requiredText` is deliberately simple and auditable. It catches missing head/tail content but does not
measure reading order, table topology, visual fidelity, OCR accuracy or annotation relocation. The
planned 50-file corpus must add human-reviewed expectations for those dimensions.
