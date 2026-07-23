// Shared runtime: owns only the app-independent logic — the serialized queue,
// timeouts, snapshot refetch, settlement derivation, refusal-whitelist decisions.
// Everything app-specific (query, dispatch, routing, refusals) comes from the generated tables module.

export function createProvider({
  base = "",
  tables,
  fetchImpl = globalThis.fetch,
  timeoutMs = 15000,
}) {
  const helpers = {
    keyText: (v) => String(v),
    requiredField: (fields, name) => {
      if (!(name in fields)) throw new Error(`missing field ${name}`);
      return fields[name];
    },
    boolValue: (v) => Boolean(v),
    textValue: (v) => String(v),
    POST_ID_TYPE: "post",
    USER_ID_TYPE: "user",
    STORY_ID_TYPE: "story",
  };

  const timed = async (url, init) => {
    const ctrl = new AbortController();
    const t = setTimeout(() => ctrl.abort(), timeoutMs);
    try {
      return await fetchImpl(url, { ...init, signal: ctrl.signal });
    } finally {
      clearTimeout(t);
    }
  };

  async function snapshot() {
    const res = await timed(`${base}/graphql/v1`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ query: tables.SNAPSHOT_QUERY }),
    });
    if (!res.ok) throw new Error(`snapshot http ${res.status}`);
    const body = await res.json();
    if (body.errors) throw new Error(`snapshot: ${JSON.stringify(body.errors)}`);
    return body.data;
  }

  // Mutations are serialized: the next RPC does not go out before the previous settlement completes.
  let chain = Promise.resolve();
  function dispatch(mutation, fields, actor) {
    const run = chain.then(() => settle(mutation, fields, actor));
    chain = run.catch(() => {});
    return run;
  }

  async function settle(mutation, fields, actor) {
    const routing = tables.MUTATION_ROUTING[mutation];
    if (!routing) throw new TypeError(`unknown mutation \`${mutation}\``);
    const { operation } = tables.toBackendOperation(mutation, null, fields, helpers);
    if (routing.mode === "local") return { settlement: "accepted", local: true };
    if (routing.mode === "host") return { settlement: "host-delegated" };

    const call =
      routing.calls.length === 1
        ? routing.calls[0]
        : routing.calls.find(
            (c) => Boolean(operation[routing.flag]) === c.when,
          );
    const args = Object.fromEntries(call.args.map((a) => [a, operation[a]]));
    const headers = { "content-type": "application/json" };
    if (actor) headers["x-spock-actor"] = actor;
    const res = await timed(`${base}/rest/v1/rpc/${call.fn}`, {
      method: "POST",
      headers,
      body: JSON.stringify(args),
    });
    const body = await res.json();
    if (res.ok) {
      const data = await snapshot();
      return { settlement: "accepted", data };
    }
    const code = String(body.error?.code ?? "").replace(/_/g, "-");
    const declared = (tables.COMMAND_REFUSALS[call.route] ?? []).includes(code);
    return { settlement: "refused", reason: declared ? code : "refused", declared };
  }

  return { snapshot, dispatch };
}
