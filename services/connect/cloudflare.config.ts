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
      // v2, the friend list: a pairing mailbox per code, a slot per slot id, a meeting
      // session per core, and a lookup per lookup id (the session that registered it).
      PAIRS: { type: "durable-object", worker: "lodestone-connect", exportName: "Pair" },
      SLOTS: { type: "durable-object", worker: "lodestone-connect", exportName: "Slot" },
      MEET_SESSIONS: { type: "durable-object", worker: "lodestone-connect", exportName: "MeetSession" },
      LOOKUPS: { type: "durable-object", worker: "lodestone-connect", exportName: "Lookup" },
      // Per client address: mailboxes, sessions and new slots made; knocks; presence
      // checks and slot reads (unauthenticated); and the other v2 requests.
      FRIEND_CREATE_LIMIT: { type: "rate-limit", namespace: "1004", simple: { limit: 10, period: 60 } },
      KNOCK_LIMIT: { type: "rate-limit", namespace: "1005", simple: { limit: 30, period: 60 } },
      LOOKUP_LIMIT: { type: "rate-limit", namespace: "1006", simple: { limit: 120, period: 60 } },
      // Slot reads: a friend list reads one slot per friend when it refreshes.
      SLOT_READ_LIMIT: { type: "rate-limit", namespace: "1008", simple: { limit: 600, period: 60 } },
      // Lookup registrations a session adds or drops, in units of 16 (so 640 a minute):
      // a core changes all its ids about once a day, and adds the next period's hourly.
      REGISTER_LIMIT: { type: "rate-limit", namespace: "1009", simple: { limit: 40, period: 60 } },
      FRIEND_LIMIT: { type: "rate-limit", namespace: "1007", simple: { limit: 60, period: 60 } },
    },
    exports: {
      // SQLite-backed, which the Workers Free plan allows.
      Invite: { type: "durable-object", storage: "sqlite" },
      Pair: { type: "durable-object", storage: "sqlite" },
      Slot: { type: "durable-object", storage: "sqlite" },
      MeetSession: { type: "durable-object", storage: "sqlite" },
      Lookup: { type: "durable-object", storage: "sqlite" },
    },
    // The Worker's own error logs only. Invocation logs (one per request) are off: they
    // would keep invite codes, lookup ids and players' addresses.
    observability: { enabled: true, logs: { enabled: true, invocationLogs: false }, traces: { enabled: false } },
  },
};
