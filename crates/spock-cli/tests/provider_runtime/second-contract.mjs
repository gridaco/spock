// A2 (uhura#29): the generated snapshot query for a *second, independently
// authored* Spock contract must be accepted by that contract's own live authority.
// filter-lab models no domain and was written to stress the query layer, so it
// carries shapes Instagram never exercises: a composite key, a self-reference,
// a closed set, a float, a timestamp, and a nullable column.
// usage: node second-contract.mjs <baseUrl> <queryPath>
import { strict as assert } from "node:assert";
import { readFileSync } from "node:fs";

const [base, queryPath] = process.argv.slice(2);
const query = readFileSync(queryPath, "utf8");

const res = await fetch(`${base}/graphql/v1`, {
  method: "POST",
  headers: { "content-type": "application/json" },
  body: JSON.stringify({ query }),
});
assert.equal(res.status, 200, `graphql http ${res.status}`);
const body = await res.json();
assert.ok(!body.errors, `the authority refused the generated query: ${JSON.stringify(body.errors)}`);

// The seed is the contract's own: five widgets (three sharing rank 1) and three edges.
const counts = Object.fromEntries(
  Object.entries(body.data).map(([k, v]) => [k, v.length]),
);
assert.deepEqual(counts, { widgets: 5, edges: 3 }, `seed counts: ${JSON.stringify(counts)}`);

// Shapes Instagram's contract does not have, read back through the generated selection.
const widgets = body.data.widgets;
const edges = body.data.edges;

// Composite key: `edge` has no single key, so both key columns are selected as refs.
for (const edge of edges) {
  assert.ok(edge.src?.id, "composite-key edge selects src by key");
  assert.ok(edge.dst?.id, "composite-key edge selects dst by key");
}

// Self-reference: `parent` is a nullable ref to the same table.
const parented = widgets.filter((w) => w.parent !== null);
assert.equal(parented.length, 2, "two seeded widgets carry a parent");
for (const w of parented) {
  assert.ok(
    widgets.some((other) => other.id === w.parent.id),
    "a self-referencing parent resolves inside the same collection",
  );
}

// Nullable column: exactly one seeded widget omits `note`.
assert.equal(
  widgets.filter((w) => w.note === null).length,
  1,
  "the nullable column reads back as null, not as an omitted field",
);

// Closed set, float, timestamp: present and typed as the contract declares.
assert.deepEqual(
  [...new Set(widgets.map((w) => w.kind))].sort(),
  ["alpha", "beta", "gamma"],
  "the closed set reads back as its declared members",
);
assert.ok(
  widgets.every((w) => typeof w.score === "number"),
  "float column reads back as a number",
);
assert.ok(
  widgets.every((w) => typeof w.made_at === "string" && w.made_at.endsWith("Z")),
  "timestamp column reads back canonically",
);

console.log("second contract ok: live authority accepted the generated query");
