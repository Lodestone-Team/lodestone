# Copy Desk

The Copy Desk is a clickable mock of the Lodestone Launcher and the GPUI Host where every
piece of English text can be edited in place. You can move around the app, set states
(signed out, game running, update ready, empty lists, errors and so on) and change the
words where they appear. Edits are saved to `edits.json` in this folder. Then the edits
are written into the source (`src/i18n/en/*.rs`) and the French and Chinese translations
are redone.

It needs only Python 3 and a browser. No build and no Rust are needed to edit.

## Set up and edit

```sh
git pull
python3 launcher/tools/copy-desk/copy_desk.py serve
```

This opens http://127.0.0.1:8765/ (use `--port` to change the port, `--no-open` to stop
the browser from opening). The page reads the strings from the source each time it
loads, so it always shows the current text.

- **Mock app:** pick Launcher or Host at the top left. With **Edit text** on, click any
  text and type. Turn it off to click through the app: buttons, tabs and dialogs work.
  The **States** panel on the left sets what the app shows. The **Pages** list jumps to
  any page or dialog.
- **All strings:** every string, grouped by file and section, with its note for
  translators. Use it for text the mock doesn't show, and to search.
- **Edited only** shows only what you changed. **Export edits** copies them as JSON.
- Edited text is highlighted. Text in `{braces}` (like `{name}` or `{count}`) is filled
  in by the app. Keep it, in any order. A plural has two forms (one, other). A
  per-platform string has one value for each system.
- Clearing a string completely means "remove this text". That needs a code change, so
  it is applied by hand.

Edits save as you type. Stop the server with Ctrl+C. Your edits stay in `edits.json`.

Open it at `127.0.0.1` or `localhost` only. The server answers only its own address and
saves only edits from its own page, so other web pages you have open can't change
`edits.json` (which `apply` writes into the source).

## Hand the edits over

Pick one:

1. **Open a pull request with `edits.json`.** A developer then applies it. This is the
   simplest way if you don't write code.
2. **Apply them yourself** (see below), in a pull request with the source changes.
3. **With Claude Code:** ask it to "apply the Copy Desk edits". The
   `apply-copy-edits` skill (`.claude/skills/apply-copy-edits/SKILL.md`) writes the
   English, redoes the translations, runs the tests and opens a pull request.

## Apply the edits

```sh
python3 launcher/tools/copy-desk/copy_desk.py status   # what changed, and what needs a hand edit
python3 launcher/tools/copy-desk/copy_desk.py apply    # write the English into en/*.rs
```

`apply` writes plain strings and plurals, and removes them from `edits.json`. These edits
stay in `edits.json` for a hand edit:

- an emptied string: delete the message from `en`, `fr`, `zh_hans` and `zh_hant`, and
  delete or change the code that shows it;
- a per-platform string (`if cfg!(...)`), or a string defined in
  `launcher-core/src/wording.rs`;
- a string whose source changed after the edit was made, or whose `{placeholders}`
  changed.

Then redo the `fr`, `zh_hans` and `zh_hant` translations of each changed string (the
same key in `src/i18n/<locale>/<file>.rs`). Run this in `launcher/`:

```sh
cargo fmt
cargo test -p lodestone-launcher -p lodestone-host -p lodestone-ui
```

When `edits.json` is empty, `apply` deletes it.

## Change the mock

The mock is plain browser JavaScript in `page/`:

- `page/mock/runtime.js`: the router, the **States** panel, the editing, and the `t()`
  function that shows a string by id (`app/file/Enum/Key`).
- `page/mock/launcher.js` and `page/mock/host.js`: the pages, dialogs and states of each
  app.
- `page/index.html`: the desk around the mock, the **All strings** list, and how edits
  are saved.

The mock is not generated from the Rust UI. When a page changes a lot, update its mock
by hand. If the mock names a string that no longer exists, it shows the key in angle
brackets, for example `⟨SkinEditorLead⟩`. A new string that the mock doesn't show can
still be edited in **All strings**.

`python3 -m unittest test_copy_desk` (in this folder) tests the server.

`extract_strings.py` parses the `messages!` enums. `python3 extract_strings.py out.json`
writes all strings to a file.

The same page also runs as a claude.ai Artifact. There, it saves edits to the artifact's
database instead of `edits.json`.
