This folder contains example macros that can be used as a starting point for creating your own macros.

Note: make sure your macro is in the correct format. The macro should be a folder with the `index.ts` or `index.js` file at the root of the folder as the entry point.

PRs are welcome! If you have a macro you'd like to share, feel free to create a PR to add it to this folder.

Read more about creating macros [here](https://github.com/Lodestone-Team/lodestone/wiki/Intro-to-Macro-and-Task)

## Runtime notes (Deno 2)

Macros run on an embedded Deno 2 runtime (`deno_runtime` 0.268). Macros written for older Lodestone releases, which embedded a Deno 1.x runtime, may need these changes:

* Deno 1.x APIs that were removed in Deno 2 are not available. These include `Deno.Buffer`, `Deno.readAll`, `Deno.writeAll`, `Deno.copy`, `Deno.iter`, `Deno.File` (use `Deno.FsFile`), the resource-id functions (`Deno.read`, `Deno.write`, `Deno.seek`, `Deno.close`, `Deno.fstat`), `Deno.resources` and `Deno.metrics`. Use `globalThis` instead of `window`.
* JSON imports need the `with` import attribute, for example `import data from "./data.json" with { type: "json" };`. TypeScript macros that still use `assert { type: "json" }` are rewritten automatically. Plain `.js` macros are not, and fail to parse.
* Imports must be relative paths or `http(s)://` URLs. `npm:`, `jsr:` and `node:` specifiers are not supported.
