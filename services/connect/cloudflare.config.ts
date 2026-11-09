// connect.lodestone.cc on Cloudflare Workers, as `cf deploy` reads it. wrangler.jsonc
// says the same for `wrangler dev` (local development and the tests). See README.md.
export default {
  worker: {
    name: "lodestone-connect",
    entrypoint: "./src/index.js",
    compatibilityDate: "2026-10-01",
    // Only on lodestone.cc: the account's workers.dev address carries a personal name.
    workersDev: false,
    previewUrls: false,
    domains: ["connect.lodestone.cc"],
    env: {
      INVITES: { type: "durable-object", worker: "lodestone-connect", exportName: "Invite" },
      // Per client address: making invites, joining them, and a host's other requests
      // (connecting to wait for joins, listing them, ending an invite).
      CREATE_LIMIT: { type: "rate-limit", namespace: "1001", simple: { limit: 10, period: 60 } },
      JOIN_LIMIT: { type: "rate-limit", namespace: "1002", simple: { limit: 30, period: 60 } },
      HOST_LIMIT: { type: "rate-limit", namespace: "1003", simple: { limit: 60, period: 60 } },
    },
    exports: {
      // SQLite-backed, which the Workers Free plan allows.
      Invite: { type: "durable-object", storage: "sqlite" },
    },
    // The Worker's own error logs only. Invocation logs (one per request) are off: they
    // would keep invite codes and players' addresses.
    observability: { enabled: true, logs: { enabled: true, invocationLogs: false }, traces: { enabled: false } },
  },
};
