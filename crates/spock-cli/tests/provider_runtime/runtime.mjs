// Shared runtime: owns only the app-independent logic — the serialized queue,
// timeouts, snapshot refetch, settlement derivation, refusal-whitelist decisions.
// Everything app-specific (query, dispatch, routing, refusals) comes from the generated tables module.

export function createProvider({
  base = "",
  tables,
  fetchImpl = globalThis.fetch,
  timeoutMs = 15000,
}) {
  // App-independent port helpers. The Uhura key types they check are app data and
  // arrive from the generated tables module, never from this file.
  const helpers = {
    requiredField: (fields, name) => {
      const value = fields.get(name);
      if (!value) throw new TypeError(`Uhura value has no field \`${name}\``);
      return value;
    },
    keyText: (value, type) => {
      if (value?.$ !== "key" || value.type !== type) {
        throw new TypeError(`expected Uhura key ${type}`);
      }
      const body = value.value;
      if (body?.$ !== "Text" || typeof body.value !== "string") {
        throw new TypeError(`${type} must wrap Text`);
      }
      return body.value;
    },
    textValue: (value) => {
      if (value?.$ !== "Text" || typeof value.value !== "string") {
        throw new TypeError("expected Uhura Text");
      }
      return value.value;
    },
    boolValue: (value) => {
      if (value?.$ !== "bool" || typeof value.value !== "boolean") {
        throw new TypeError("expected Uhura Bool");
      }
      return value.value;
    },
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
