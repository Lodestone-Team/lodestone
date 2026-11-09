// The new-format config that `cf deploy` reads (wrangler.jsonc says the same, for
// `wrangler dev`). Each invite is one Durable Object; see src/index.js.
export default {
  worker: {
    name: "lodestone-rendezvous",
    entrypoint: "./src/index.js",
    compatibilityDate: "2026-10-01",
    // Only on lodestone.cc: the account's workers.dev address carries a personal name.
    workersDev: false,
    previewUrls: false,
    domains: ["connect.lodestone.cc"],
    env: {
      SESSIONS: { type: "durable-object", worker: "lodestone-rendezvous", exportName: "Session" },
    },
    exports: {
      // SQLite-backed, which the Workers Free plan allows.
      Session: { type: "durable-object", storage: "sqlite" },
    },
  },
};
