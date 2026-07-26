// A1 (uhura#29): the shared runtime must carry no app knowledge, and the port
// boundary must actually check the Uhura key types the generated tables declare.
// usage: node genericity.mjs <tablesModulePath> <runtimeModulePath> <runtimeSourcePath>
import { strict as assert } from "node:assert";
import { readFileSync } from "node:fs";

const [tablesPath, runtimePath, runtimeSource] = process.argv.slice(2);
const tables = await import(tablesPath);
const { createProvider } = await import(runtimePath);

// 1) The runtime source may not name any app-specific identity. The generated
//    tables own every app fact; the runtime owns only app-independent logic.
const source = readFileSync(runtimeSource, "utf8");
const appIdentifiers = source.match(/\b[A-Z][A-Z0-9_]*_ID_TYPE\b/g) ?? [];
assert.deepEqual(
  appIdentifiers,
  [],
  `runtime hardcodes app id types: ${[...new Set(appIdentifiers)].join(", ")}`,
);

// 2) The generated tables must publish the module identity they were built for.
assert.equal(
  typeof tables.MODULE,
  "string",
  "generated tables must export the Uhura module identity",
);

const key = (type, text) => ({
  $: "key",
  type,
  value: { $: "Text", value: text },
});
const bool = (value) => ({ $: "bool", value });

const fetchLog = [];
const fakeFetch = async (url, init) => {
  if (url.endsWith("/graphql/v1")) {
    return { ok: true, json: async () => ({ data: {} }) };
  }
  fetchLog.push(JSON.parse(init.body));
  return { ok: true, json: async () => ({ id: "row" }) };
};
const provider = createProvider({ base: "", tables, fetchImpl: fakeFetch });

// 3) A correctly typed key is unwrapped to its text.
await provider.dispatch(
  "SetLike",
  new Map([
    ["post", key(`${tables.MODULE}::PostId`, "P1")],
    ["liked", bool(true)],
  ]),
  "U1",
);
assert.deepEqual(fetchLog, [{ post: "P1" }], "typed key unwraps to its text");

// 4) A key carrying the wrong Uhura type must be refused at the port boundary.
//    Without this the declared id type is decorative and any app's keys pass.
await assert.rejects(
  provider.dispatch(
    "SetLike",
    new Map([
      ["post", key(`${tables.MODULE}::UserId`, "P1")],
      ["liked", bool(true)],
    ]),
    "U1",
  ),
  /expected Uhura key/,
  "a wrongly typed key must be refused",
);

// 5) A non-key value must be refused for the same reason.
await assert.rejects(
  provider.dispatch(
    "SetLike",
    new Map([
      ["post", { $: "Text", value: "P1" }],
      ["liked", bool(true)],
    ]),
    "U1",
  ),
  /expected Uhura key/,
  "an untagged value must be refused",
);

console.log("genericity ok: app-independent runtime, checked key types");
