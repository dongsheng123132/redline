import assert from "node:assert/strict";
import test from "node:test";

import {
  countDoclingItems,
  countTextBlocks,
  scoreRequiredText,
  summarizeAttempts,
} from "./shadowdoc-pk.mjs";

test("required text recall is explicit and exact", () => {
  const score = scoreRequiredText("Alpha Beta", ["Alpha", "alpha", "Beta"]);
  assert.equal(score.recallPercent, 66.67);
  assert.deepEqual(score.found.map((item) => item.found), [true, false, true]);
});

test("markdown-like output counts non-empty blocks", () => {
  assert.equal(countTextBlocks("# A\n\nbody\n\n\n# B"), 3);
  assert.equal(countTextBlocks(""), 0);
});

test("Docling items are deduplicated by self_ref", () => {
  const document = {
    body: [
      { self_ref: "#/texts/0", label: "title", text: "A" },
      { self_ref: "#/texts/0", label: "title", text: "A" },
      { self_ref: "#/texts/1", label: "paragraph", text: "B" },
    ],
  };
  assert.equal(countDoclingItems(document), 2);
});

test("attempt summary reports median and output determinism", () => {
  const attempts = [30, 10, 20].map((durationMs) => ({
    adapter: "example",
    status: "ok",
    durationMs,
    outputTextSha256: "same",
  }));
  const summary = summarizeAttempts(attempts);
  assert.equal(summary.durationMedianMs, 20);
  assert.equal(summary.durationMinMs, 10);
  assert.equal(summary.durationMaxMs, 30);
  assert.equal(summary.outputDeterministic, true);
  assert.equal(summary.successes, 3);
});
