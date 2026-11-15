---
name: apply-copy-edits
description: Apply the English copy edits saved by the Copy Desk (launcher/tools/copy-desk/edits.json, or a Copy Desk artifact's database) to the Launcher's and Host's i18n source, redo the French, Simplified Chinese and Traditional Chinese translations, test, and open a PR. Use when a developer asks to apply Copy Desk edits or copy changes.
---

# Apply Copy Desk edits

The Copy Desk (`launcher/tools/copy-desk/`, see its README) saves edits as
`{ "app/file/Enum/Key": { "texts": [...], "original": [...] } }`. `texts` has one entry
for a plain string, two for a plural (one, other), and one per platform for an
`if cfg!(...)` string. `original` is the English when the edit was made.

1. **Find the edits.** Use `launcher/tools/copy-desk/edits.json`, or if the developer
   points to a Copy Desk artifact, read its `edits` collection (ArtifactData `list`; doc
   ids spell `/` as `~`). Run `python3 launcher/tools/copy-desk/copy_desk.py status
   --json` (for an artifact, first write its edits into `edits.json` in that shape). It
   gives each edit's file, key, current text, new text, and why it needs a hand edit.
2. **Work on a branch** from an up-to-date `origin/main` (a worktree if the main
   checkout has changes).
3. **Write the English.** `copy_desk.py apply` writes plain strings and plurals. Do the
   rest by hand:
   - **Emptied text** means remove it. Delete the message from `en`, `fr`, `zh_hans`
     and `zh_hant`, and change the code that shows it. Use `hint_opt(None)` or a `None`
     lead in place of an empty string. Never leave an empty string behind.
   - **Per-platform strings** keep their `if cfg!` shape. **`wording::` strings** are
     edited in `launcher/crates/launcher-core/src/wording.rs`.
   - **The source changed since the edit:** show the developer the edit and the current
     text, and ask.
   - Keep every `{placeholder}`. Fix plain typos in the new text (say which ones in the
     PR). Match the punctuation of nearby strings, for example a full stop at the end
     of a hint. Otherwise keep the developer's words.
   - If the same English is used by the other app (the Launcher and Host often share a
     hint, such as `BetaBuildsHint`), ask whether to change both, unless the developer
     already said.
4. **Redo the translations** of each changed key in
   `src/i18n/{fr,zh_hans,zh_hant}/<file>.rs`. Translate the new English. Don't patch
   the old translation. Follow each locale's existing terms in that file (for example
   "build", "instance", "réglages"; 构建 against 組建). French uses `’` and a space
   before `:`.
5. **Update the mock** (`launcher/tools/copy-desk/page/mock/*.js`) if a key was removed
   or renamed.
6. **Check:** in `launcher/`, run `cargo fmt` and
   `cargo test -p lodestone-launcher -p lodestone-host -p lodestone-ui`. Optionally,
   render the changed screen with the offscreen scenes (AGENTS.md).
7. **Clear the edits:** remove the applied entries from `edits.json`. Delete the file
   when it is empty. For an artifact, ask before deleting its rows.
8. **Open a PR** with a table of each key: the old English, the new English, and any
   change you made to the developer's wording.
