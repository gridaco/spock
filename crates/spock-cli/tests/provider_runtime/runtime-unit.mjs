// Shared-runtime unit checks (fake server): branch routing, three settlement paths, serialized queue.
// usage: node runtime-unit.mjs <tablesModulePath> <runtimeModulePath>
import { strict as assert } from "node:assert";

const [tablesPath, runtimePath] = process.argv.slice(2);
const tables = await import(tablesPath);
const { createProvider } = await import(runtimePath);

const log = [];
let rpcDelay = 0;
let rpcScript = () => ({ ok: true, body: { id: "row" } });
const fakeFetch = async (url, init) => {
  if (url.endsWith("/graphql/v1")) {
    log.push("gql");
    return { ok: true, json: async () => ({ data: { posts: [] } }) };
  }
  const fn = url.split("/rpc/")[1];
  log.push(`rpc:${fn}:${init.headers["x-spock-actor"] ?? "-"}`);
  if (rpcDelay) await new Promise((r) => setTimeout(r, rpcDelay));
  const { ok, body } = rpcScript(fn, JSON.parse(init.body));
  return { ok, json: async () => body };
};

const provider = createProvider({ base: "", tables, fetchImpl: fakeFetch });

// Uhura wire values: mutation fields arrive as a keyed map of tagged values.
const key = (type, text) => ({
  $: "key",
  type,
  value: { $: "Text", value: text },
});
const bool = (value) => ({ $: "bool", value });
const postId = (text) => key(`${tables.MODULE}::PostId`, text);

// 1) Branching: liked=true → like_post / liked=false → unlike_post, argument = post id
let r = await provider.dispatch(
  "SetLike",
  new Map([["post", postId("P1")], ["liked", bool(true)]]),
  "U1",
);
assert.equal(r.settlement, "accepted");
r = await provider.dispatch(
  "SetLike",
  new Map([["post", postId("P1")], ["liked", bool(false)]]),
  "U1",
);
assert.equal(r.settlement, "accepted");
assert.deepEqual(
  log.filter((l) => l.startsWith("rpc")),
  ["rpc:like_post:U1", "rpc:unlike_post:U1"],
);

// 2) Settlement: the accept path must re-snapshot after the RPC
assert.deepEqual(log, ["rpc:like_post:U1", "gql", "rpc:unlike_post:U1", "gql"]);

// 3) Refusal inside the whitelist: the declared reason passes through as-is
rpcScript = () => ({ ok: false, body: { error: { code: "not_authorized" } } });
r = await provider.dispatch(
  "SetLike",
  new Map([["post", postId("P1")], ["liked", bool(true)]]),
  null,
);
assert.deepEqual(r, { settlement: "refused", reason: "not-authorized", declared: true });

// 4) Refusal outside the whitelist: collapsed to a generic refusal (no internal leakage)
rpcScript = () => ({ ok: false, body: { error: { code: "disk_on_fire" } } });
r = await provider.dispatch(
  "SetLike",
  new Map([["post", postId("P1")], ["liked", bool(true)]]),
  "U1",
);
assert.deepEqual(r, { settlement: "refused", reason: "refused", declared: false });

// 5) Local mode: accepted without an RPC
const before = log.length;
r = await provider.dispatch("LoadMore", new Map(), "U1");
assert.deepEqual(r, { settlement: "accepted", local: true });
assert.equal(log.length, before, "local mutation performs no fetch");

// 6) Serialization: two dispatches never overlap (second RPC only after the first settles)
log.length = 0;
rpcScript = () => ({ ok: true, body: {} });
rpcDelay = 30;
const p1 = provider.dispatch(
  "SetLike",
  new Map([["post", postId("A")], ["liked", bool(true)]]),
  "U1",
);
const p2 = provider.dispatch(
  "SetSave",
  new Map([["post", postId("B")], ["saved", bool(true)]]),
  "U1",
);
await Promise.all([p1, p2]);
assert.deepEqual(
  log,
  ["rpc:like_post:U1", "gql", "rpc:save_post:U1", "gql"],
  "queue serializes settlements",
);

console.log("runtime ok: branching, settlement x3, local, serialization");
