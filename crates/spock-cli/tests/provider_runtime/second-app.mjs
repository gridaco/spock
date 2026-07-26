// A1 (uhura#29): a second app, generated from its own contract and its own package
// identity, must drive the same shared runtime with no runtime change at all.
// usage: node second-app.mjs <tablesModulePath> <runtimeModulePath>
import { strict as assert } from "node:assert";

const [tablesPath, runtimePath] = process.argv.slice(2);
const tables = await import(tablesPath);
const { createProvider } = await import(runtimePath);

assert.equal(tables.MODULE, "app.blog@2", "tables carry this app's identity");

const sent = [];
const fakeFetch = async (url, init) => {
  if (url.endsWith("/graphql/v1")) {
    return { ok: true, json: async () => ({ data: {} }) };
  }
  sent.push({ fn: url.split("/rpc/")[1], args: JSON.parse(init.body) });
  return { ok: true, json: async () => ({ id: "row" }) };
};

const provider = createProvider({ base: "", tables, fetchImpl: fakeFetch });
const articleId = (text) => ({
  $: "key",
  type: `${tables.MODULE}::ArticleId`,
  value: { $: "Text", value: text },
});
const bool = (value) => ({ $: "bool", value });

// Branch both ways through this app's own fns.
await provider.dispatch(
  "SetPublished",
  new Map([["article", articleId("A1")], ["published", bool(true)]]),
  "editor",
);
await provider.dispatch(
  "SetPublished",
  new Map([["article", articleId("A1")], ["published", bool(false)]]),
  "editor",
);
assert.deepEqual(sent, [
  { fn: "publish_article", args: { article: "A1" } },
  { fn: "retract_article", args: { article: "A1" } },
]);

// Instagram's key type is meaningless here and must be refused.
await assert.rejects(
  provider.dispatch(
    "SetPublished",
    new Map([
      [
        "article",
        {
          $: "key",
          type: "app.instagram@1::PostId",
          value: { $: "Text", value: "A1" },
        },
      ],
      ["published", bool(true)],
    ]),
    "editor",
  ),
  /expected Uhura key app\.blog@2::ArticleId/,
  "another app's key type must not be accepted",
);

console.log("second app ok: own identity, own fns, shared runtime unchanged");
