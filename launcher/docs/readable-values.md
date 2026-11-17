# Readable values

How Lodestone Launcher and Lodestone Host show names and values. Each kind of value
has one role, and each role has one look in both apps and both themes, so a reader
learns it once. The pieces are in `launcher/crates/lodestone-ui`:
`components::readable` (the components), `i18n::spans` (`tr_spans!`/`trn_spans!`),
`values` (truncation, the product phrase, the role styles), and `theme`
(`outline()`, `Tone`, `pill`, `fonts::tabular()`, `text::VERSION`).

## The rules

1. **The object is never inside the sentence.** What a dialog, toast or error is
   about gets its own unit: an `ObjectCard` under a dialog's title, an `ObjectChip`
   in a toast or notice (`Toast::object`), the row's name in a list. Titles name the
   action and the kind only ("Delete instance?"). The body says "its folder", not
   the name again.
2. **One role, one style.** Object: Satoshi 500 in the main ink, no quotes. Mod,
   plugin and modpack versions: `VersionTag`. Numbers: tabular figures, the unit one
   ink lighter. Exact text (file names, paths, addresses, ids, env vars, codes):
   JetBrains Mono. Times: muted ink. Keys: 12px, 450, faint ink. State:
   `StatusBadge`.
3. **The game and its loader are one plain phrase**: "Minecraft 26.3", "Fabric
   0.20.1", "Paper 1.21.4" (`ProductPhrase`, `values::phrase`, joined with a
   no-break space). In rows it stands alone, with no key. In key-over-value strips
   and fact tables its key is **Game** or **Loader** (`Product::fact_key`). Vanilla
   has no loader fact; a fact table that must say it shows "Vanilla"
   (`FactMsg::Vanilla`) with no version. Only mods, plugins and modpacks get a
   `VersionTag` ("Fabric API" then `0.116.17+1.21.1`).
4. **Every value that isn't self-explanatory has a key**: a translated word, or in
   tight rows a 12px icon whose tooltip and accessible name read "key: value"
   (`Fact::icon`). A size with its unit and a version right after its name need no
   key (`Fact::bare` / `Fact::bare_named`, whose key is for accessibility only). Keys
   are their own messages, with no colon.
5. **No separator characters.** Never join facts with " · " or " • ". Facts are
   elements: `FactLine::row` (12px apart, never wraps, drops the lowest-priority
   facts and lists all of them in its tooltip) or `FactLine::strip` (24px apart,
   thin dividers, wraps). Lists whose rows share the same facts use `ListColumns`
   with `ListHeader`, not keys on every row.
6. **Sentences with values are typed spans**: `tr_spans!` and `trn_spans!` tag each
   value with its role, so translators keep the word order and the style moves
   with the placeholder. Long or actionable values (paths, URLs, codes, keys) leave
   the sentence for a `ValueField`.
7. **Mono means exact.** Never for prose or display names. File names, paths and
   long versions are cut in the middle, keeping the extension, the last two folders
   or the end; the full value is in the tooltip and the accessible name. Paths
   under the home folder start with `~`.
8. **Numbers** come from `i18n::format` and are drawn by `Num` (tabular figures; the
   unit once, after the total: "1.2 / 3.4 MB"). Values that change live get
   `.min_w(..)` and `.align_right()`.
9. **Colour means status only.** `theme::pill` with a `Tone`; yellow for a shortfall
   or mismatch, red for what blocks. Chips and tags are grays and `theme::outline()`.
   No new colours, no new opacities (Daylight maps only the ones it knows).
10. **Density stays the same.** An icon replaces a key, a header replaces repeated
    keys, a `CountBadge` replaces "(7)", a `KindIcon` replaces "(folder)". A list of
    names shows 5, then a counted "Show N more" (`NameList`). Never an uncounted "…".
11. **Layouts survive translation.** Strips and dialogs wrap rather than cut keys;
    `FactList` sizes its key column to the widest key (96px to 40%) and wraps keys to
    2 lines. Never uppercase in code. Arrows are icons, mirrored in RTL.

## Which to use

| You show | Use |
| --- | --- |
| A mod, plugin or modpack with its version | `ObjectChip`/name, then `VersionTag::new(v)` |
| The game or a loader | `ProductPhrase::new(Product::Fabric, "0.20.1")` (or `from_name`) |
| An old and a new version | `DiffValue::versions(from, to)`; text values `DiffValue::text` |
| What a dialog is about | `ObjectCard::new(name).kind(Kind::World).fact(..)` |
| What a toast or notice is about | `Toast::object(ObjectChip::new(name).kind(..))`, `.status(..)` |
| A name inline, clickable or not | `ObjectChip` (`.on_click` makes it a link) |
| A row's second line | `FactLine::row(id).fact(..)` with `.priority(n)`, `.shrink()` for a file name |
| A header's facts | `FactLine::strip(id)` |
| A details table | `FactList::new(id).row(key, value)` (`.framed(true)` as a card) |
| A size, count or fraction | `Num::bytes`, `Num::count`, `Num::fraction_bytes`, `Num::fraction` |
| A path, file name or exact value in a row | `PathText::path` / `::file` / `::exact` |
| An exact value inline | `MonoValue::new(v)` |
| A path, URL, code or secret to copy | `ValueField::new(id, v).label(key).kind(ValueKind::..)` |
| State | `StatusBadge::new(label, Tone::Success)` |
| A count on a tab, row or heading | `CountBadge::new(n)`, `ListRow::count(n)`, `Tabs::counts(..)` |
| Why a snapshot or backup exists | `ReasonTag::new(label).icon(..)` |
| A snapshot or backup with no name | `UntitledName::new(t(ValueMsg::UntitledSnapshot)).on_rename(..)` |
| Space needed against free | `StatRow::new().tile(StatTile::bytes(..)).meter(MeterBar::needed_free(..))` |
| A running job | `ProgressBlock::new(id, status, name).version(..).percent(..).stat(..)` |
| Two sides that should match | `CompareTable::new(id, [a, b]).row(key, a, b, mismatch)` |
| Names grouped by outcome | `NameList::new(id).group(NameGroup::new(h).item(NameItem::new(n).icon(..).reason(..)))` |
| Columns in a list | `ListColumns::new(vec![Column::size(..), Column::date(..)], fold_below)`, `ListHeader`, `.cells(..)`, `.fold_into(..)` |
| A sentence with values | `tr_spans!(Msg::X, name = Obj(n), version = Ver(v))` |

## Typed spans

```rust
use lodestone_ui::i18n::spans::{Mono, Num, Obj, Time, Ver};

// In any crate whose `crate::i18n` has `t` and `Plural` (both apps do):
let line = lodestone_ui::tr_spans!(ShellMsg::CoreHost, host = Mono(&host));
let more = lodestone_ui::trn_spans!(ContentPlural::FilesAdded, n, name = Obj(&pack));
div().child(line) // a selectable RichText; `line.plain()` for logs and accessibility
```

Wrappers: `Obj` (a name), `Ver` (a version), `Mono` (exact text), `Time` (a
formatted time), `Num(u64)` (grouped by locale), `NumText` (an already formatted
number), `Plain`; a `&str`/`String` is plain, and a `format::Measure` is a number
with a lighter unit. Values are sanitized (controls, newlines and bidi overrides
replaced, length capped) and never read for placeholders; an unknown placeholder
stays as written. Runs can't change the font size, so a mono value in a sentence
keeps the sentence's size.

## Decisions

- Runs are built from the parent's `window.text_style()`; tooltip views set their
  own text style.
- The macros look messages up through the calling crate's `crate::i18n`, on
  purpose (`clippy::crate_in_macro_def` is allowed there); each app's i18n tests
  compile a use of both.
- A `FactLine` row always has a tooltip listing every fact with its key.
- `ListColumns` folds from the width measured in the previous frame and refreshes
  only when the folded state changes.
- A `VersionTag` has a tooltip when the version is over 22 characters
  (`VERSION_TOOLTIP_OVER`, what fits its 160px).
- Names from servers and friends are untrusted: `values::truncate::sanitize` runs
  before they are shown.

## The gallery

Every component with sample values, in both themes and all four languages:

```sh
cd launcher
export LODESTONE_LAUNCHER_DATA_DIR=$(mktemp -d) LODESTONE_PATH=$(mktemp -d) LODESTONE_HOST_DATA_DIR=$(mktemp -d)
cargo run -p lodestone-launcher --features screenshot -- --scene components --screenshot c.png
#   components-2, components-3; components-narrow[-2..-5] at 520px; --lang fr|zh-Hans|zh-Hant
cargo run -p lodestone-host --features screenshot -- --screenshot c.png --components [--page 2] [--light] [--narrow] [--lang fr]
```

The Host's gallery opens alone, before anything starts the core.
