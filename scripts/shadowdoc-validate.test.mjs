import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

import { validateShadowDoc } from "./shadowdoc-validate.mjs";

const root = new URL("../spec/shadowdoc/conformance/", import.meta.url);
const manifest = JSON.parse(await readFile(new URL("manifest.json", root), "utf8"));

for (const fixture of manifest.fixtures) {
  test(`${fixture.valid ? "accepts" : "rejects"} ${fixture.file}`, async () => {
    const value = JSON.parse(await readFile(new URL(fixture.file, root), "utf8"));
    const output = validateShadowDoc(value, fixture.file);
    assert.equal(output.ok, fixture.valid);
    if (fixture.code) {
      assert.ok(output.violations.some((violation) => violation.code === fixture.code), JSON.stringify(output));
    }
  });
}

test("accepts additive extension fields", async () => {
  const value = JSON.parse(await readFile(new URL("valid/minimal.json", root), "utf8"));
  value.vendorExtension = { kept: true };
  value.units[0].vendorExtension = "opaque";
  assert.equal(validateShadowDoc(value).ok, true);
});
