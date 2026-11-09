---
name: create-skin
description: Make a Lodestone Launcher skin (a pack of skin.json, images and fonts) that paints the window's surfaces, sets its text colours, fonts and instance banners, and reads well. Use when asked to create, design, change, check or package a launcher skin or theme.
---

# Creating a Lodestone Launcher skin

This guide is for a person or an AI agent. A skin changes how Lodestone Launcher looks:

- every surface of the window, painted with a colour, a gradient or an image;
- the theme's text and border colours;
- fonts for each part of the window;
- the banners that new instances get as their Play picture.

A skin is only data. Nothing in a pack runs. `launcher/docs/skins.md` is the short reference; this guide covers the whole process.

**You are responsible for readability.** The launcher doesn't change your colours. It measures each surface and suggests fixes. Check every skin before you share it (step 5).

## 1. Lay out the pack

A pack is a folder, or a `.zip` of one (flat, or everything in one top folder). It holds only:

```
cozy/
  skin.json          the description (required)
  planks.png         images it names: .png, .jpg/.jpeg, .webp
  meadow.jpg
  cozy-serif.ttf     fonts it names: .ttf, .otf
```

Anything else makes the import fail: scripts, HTML, subfolders, or files that `skin.json` doesn't name.

**Images:**
- Each image decodes as PNG, JPEG or WebP, at most 8192 px a side.
- On import each image is scaled down and re-encoded:
  - the sidebar's to 1024 × 2048;
  - the Play picture and banners to 2560 × 1600;
  - other surfaces' to 2048 × 2048.
- A pattern you tile can be small (16 × 16 to 128 × 128).

**Fonts:**
- A single TrueType or OpenType font. No `.ttc` collections and no web fonts.
- At most 2 MB each, and at most 4 per pack.
- It must have a family name, a full name and a PostScript name.
- Only ship fonts whose licence allows redistribution, and check it before you ship.

**The whole pack** is at most 20 MB.

**Built-in images**, which a pack can name instead of shipping a file: `builtin:dirt` (a 16 px block to tile), `builtin:sky`, `builtin:sunset`, `builtin:night`, `builtin:glass`, `builtin:aurora`, `builtin:grid` and `builtin:grid-dawn`.

## 2. Write skin.json

```json
{
  "schema": 1,
  "name": "Cozy Cabin",
  "author": "you",
  "accent": "#E0A458",
  "surfaces": { "sidebar": { "fill": "#2A2018" } },
  "tokens": {},
  "fonts": {},
  "banners": [],
  "hero": {},
  "play": {}
}
```

`schema` (always `1`) and `name` (1–48 characters) are required. Every other key is optional. Unknown keys are refused, so a typo fails loudly. Colours are `"#RRGGBB"`.

### Surfaces

The `surfaces` map is keyed by surface id:

| id | what it is |
|---|---|
| `window` | behind everything; shows through a see-through `main` |
| `top_bar` | the bar along the top with the logo |
| `sidebar` | the instance list on the left |
| `main` | the page beside the sidebar (see-through by default: the window shows) |
| `hero` | the Play tab's picture, for instances without their own |
| `play_bar` | the bar along the bottom with the Play button |
| `card` | panels on a page, the Play bar's pickers |
| `setting_card` | the groups of rows in Settings |
| `row` | rows in lists: content, worlds, servers |
| `dialog` | dialogs and the Settings window |
| `settings_nav` | the Settings window's list of pages |
| `popover` | menus and tooltips |
| `input` | text fields |
| `button_primary`, `button_secondary`, `button_ghost`, `button_danger` | the four kinds of button |
| `toggle` | a switch that is on |
| `tab` | the instance tab that is chosen (Play, Content, …) |
| `pill` | the chosen segment of a segmented picker |
| `play_button` | the Play button |
| `progress` | progress bars |
| `toast` | notices at the bottom, such as Undo |

Each surface takes:

```json
{
  "fill": "#2A2018",
  "border": "#4A3A2A",
  "border_width": 1,
  "radius": 8,
  "text": "#F4EEE6",
  "muted": "#D9CDBE"
}
```

- `fill` takes one of four forms:
  - a colour: `"#2A2018"`;
  - a see-through colour: `{"solid": {"color": "#0E111A", "opacity": 60}}` (opacity 0–100);
  - a gradient: `{"gradient": {"kind": "linear", "angle": 135, "stops": ["#3A0CA3", "#7209B7", "#F72585"]}}`.
    - `kind` is `linear` (the default) or `radial`.
    - `angle` works as in CSS: 0 runs bottom to top, 90 left to right, and 180 (the default) top to bottom.
    - `stops` holds 2–8 colours, spaced evenly, or `{"color": "#…", "at": 0–100}` to place them, in order.
    - A two-colour linear gradient is drawn natively. Other gradients are drawn once into a small image and stretched over the surface.
  - an image: `{"image": {"image": "planks.png", "fit": "tile", "tint": "#140E08", "tint_opacity": 55, "blur": 0}}`.
    - `fit` is `cover` (the default) or `tile`.
    - The tint is laid over the image.
    - `blur` is 0–20 px.
    - Images are only for the large surfaces: not `input`, buttons other than Play, `toggle`, `tab`, `pill` or `progress`.
- `border` and `border_width`: border width is 0–4 px, 1 if only a colour is given.
- `radius`: corners, 0–32 px.
- `text`: the surface's own text colour. It is only used on `sidebar`, `toast` and the controls (the buttons, `tab`, `pill`, `play_button`); setting it anywhere else is refused. On the other surfaces, text uses `tokens`. A control without `text` gets white or near-black, whichever reads better on its fill.
- `muted`: only on `sidebar`. It covers the sidebar's quieter text: headers, subtitles, counts, the footer and its icons.
- `material`: glass. What is behind the panel shows through it: `"translucent"` (as it is), `"blur"` or `"frost"` (blurred, with a pale veil and grain), or `{"kind": "frost", "blur": 24, "opacity": 35, "frost": 10, "grain": 20}`.
  - `blur` is 0–40 px (16 for `blur`, 20 for `frost`; `translucent` doesn't blur). `opacity` (0–100) is how much of the panel's own fill shows over it; without it, a see-through solid's own opacity, else 60 / 55 / 40. `frost` and `grain` are 0–100.
  - Only on the panels: `top_bar`, `sidebar`, `main`, `play_bar`, `card`, `setting_card`, `row`, `dialog`, `settings_nav`, `popover`, `input`, `toast`. Refused on `window`, `hero` and the controls.
  - It shows what the skin paints behind it: the `window` for the bars, `main`, dialogs, menus and notices; `main` (or the `window` through a see-through `main`) for cards, rows and inputs; the dialog, and the dimmed window behind it, for `setting_card` and `settings_nav`. Give the `window` an image for glass to have something to blur.
  - Readability checks count the material: what is behind, softened by the blur, under the veil and the fill at its opacity.

### See-through windows: transparency and window_opacity

A skin can make the whole window see-through, so the desktop shows behind it. This is separate from a surface's `material`, which is how what is *inside* the window shows through a panel.

```json
{
  "transparency": "glass",
  "window_opacity": 70
}
```

- `transparency` is one of:
  - `opaque` (the default);
  - `clear`: the desktop shows through, sharp;
  - `frosted`: the desktop shows through, blurred by the system;
  - `glass`: frosted, and the floating layer (sidebar, top bar, Play bar, dialogs, popovers, notices) becomes Liquid Glass: a lit rim, a shadow beneath, the window's picture bent at the edges, a flash of light as dialogs and popovers appear.
- `window_opacity` (30–100, default 75) is how opaque the window is.
- Each surface then fades by its role:
  - the window takes `window_opacity`;
  - panels on it (`sidebar`, `top_bar`, `play_bar`, `settings_nav`, `main`) add a little more shade;
  - `card`, `setting_card`, `row`, `input` and `hero` add a little more;
  - `dialog`, `popover` and `toast` stay mostly solid, because they sit over the launcher's own content, which isn't blurred.
  - Buttons, toggles and the Play button keep their fill.
  - A surface with its own `material` keeps it, and keeps as much of its fill as the material says.
- When `window` has an image fill, the panels without a `material` of their own show it blurred under them, so they look frosted over your image.
- The user's Settings › General › Transparency wins over the skin's `transparency` and `window_opacity`. Their "Skin" choice follows yours.
- What shows through changes with the desktop, so the readability checks assume the worst: white behind light text, black behind dark text. When text only falls short because the window is see-through, the suggestion is a higher `window_opacity`.

### Tokens, accent and progress

```json
"tokens": { "text": "#F3E8FF", "secondary": "#D8C4F0", "muted": "#BFA8DC", "faint": "#A58BC7", "border": "#3A1F5C" },
"accent": "#F72585",
"progress": "#4CC9F0"
```

- **`tokens`** are the theme's shared colours. Each kind of text has its own:
  - `text`: names, titles and values;
  - `secondary`: labels;
  - `muted`: descriptions and subtitles;
  - `faint`: hints and placeholders;
  - `border`: lines.

  If you darken or lighten the big surfaces, set the tokens to match. One set covers every surface without its own text colour.
- **`accent`** is used for links, focus rings, the chosen tab and primary buttons. The launcher makes it readable on dark panels and under white text.
- **`progress`** colours progress bars. A `progress` surface wins over it.

### Fonts

```json
"fonts": {
  "heading": "Lodestone Pixel",
  "body": "Satoshi",
  "sidebar": "cozy-serif.ttf",
  "button": "Clash Grotesk",
  "play": "Lodestone Pixel",
  "tab": "Clash Grotesk",
  "mono": "JetBrains Mono"
}
```

Each value is either a bundled family or a font file in the pack. The bundled families are:
- `Satoshi`: the body text;
- `Chillax`: headings;
- `Clash Grotesk`: titles and the Play button;
- `JetBrains Mono`;
- `Lodestone Pixel`: a 5 × 7 pixel font, OFL.

What each part covers:
- `heading` covers page titles and section headings;
- `play` covers the Play button and the Play tab's title;
- `mono` covers logs and code.

A part you leave out keeps the launcher's font. A font file that can't be used falls back to the launcher's font without an error.

### Banners

```json
"banners": [
  { "id": "meadow", "name": "Meadow", "image": "meadow.jpg" },
  { "id": "night", "name": "Night", "image": "builtin:night" }
]
```

Up to 16 banners. The `id` is 1–32 lowercase letters, digits or dashes.

**Keep ids stable across updates.** An instance stores its banner as `skin:<skin id>/<banner id>`, so an update that keeps the id keeps each instance's banner.

How instances use them:
- A new instance takes one at random from the active skin.
- An instance's Play background picker lists them under **From the skin**.
- If the skin or the banner is gone, the default Play picture shows.

### The Play tab and the Play button

```json
"hero": { "image": "meadow.jpg", "scrim": 45 },
"play": { "shape": "pixel", "label": "GO", "glow": false },
"banner": { "in_sidebar": true }
```

- `hero.image` is the same as `surfaces.hero.fill.image`.
- `hero.scrim` (0–100) shades the bottom left of the Play picture, where the white title sits. Use it on bright images.
- `play.shape` is `rounded`, `pill` or `pixel`.
- `play.label` replaces "PLAY" (at most 16 characters).
- `play.glow` adds a soft glow.
- The Play button's colours come from `surfaces.play_button`.
- `banner.in_sidebar` shows each instance's banner faintly behind its sidebar row.

### Older keys and readability: auto

Packs from before `surfaces` used the `sidebar` keys (`image`, `fit`, `tint`, `tint_opacity`, `blur`, `text.primary`, `text.muted`) and `play.color` / `play.gradient`. They still work, but `surfaces` wins where both are given; new packs should use `surfaces`.

`"readability": "auto"` on `sidebar` or `hero` is an opt-in. The launcher then works out that surface's text colours and a backing (or the hero's shade) from the image itself. The default is `"off"`.

## 3. Readability: how to make text read

The checks the launcher runs, and the targets to meet:

- Body text and subtitles need **4.5:1** contrast against what is behind them.
- Labels on controls, hints and icons need **3:1**.
- The Play picture's white title needs **4.5:1** under its scrim.

Text is checked against the **worse end** of what it sits on:
- for an image, its 10th and 90th percentile of brightness, after blur and under its tint;
- for a gradient, its darkest and lightest colours;
- for a see-through fill, the fill over whatever is under it (the window under `main`, and so on).

Practical rules:

- **Images.** An image busy enough to reach both ends needs a tint. A 50–70 % dark tint lets white text read on almost anything. Blur also calms textures down.
- **Dark themes.** The launcher's own muted grey (`#767A82`) only reaches about 3.5:1 on dark panels. If you change the big surfaces, set `tokens.muted` to `#9EA1A8` or lighter.
- **Light themes.** Set dark `tokens` (text near `#111114`, muted around `#4A4D55`), and dark `text` and `muted` on the sidebar.
- **Controls.** Leave `text` unset and let the launcher pick white or near-black, unless you need a brand colour that passes 3:1.
- **Play picture.** For bright pictures, set `hero.scrim` (40–60 is typical).

**How the suggestions work.** For each check that falls short, the launcher suggests the smallest change that passes, trying these in order:

1. the surface's own text colour (`surfaces.<id>.text` or `.muted`), where the surface has one;
2. otherwise, a darker or lighter fill (`surfaces.<id>.fill`), or a stronger image tint;
3. otherwise, the shared token (`tokens.text`, `tokens.muted` or `tokens.faint`);
4. for the Play picture, `hero.scrim`.

"Fix all" applies them round by round: first each surface's own fixes, then the shared tokens against what the surfaces have become. Read the result. A token that suits a light card may not suit a dark page, so change the fill instead when two surfaces pull apart.

## 4. Edit it in the launcher (optional)

Settings → Skin → **Pop out editor** opens the skin editor in its own window:

- **Left column:** the surfaces, with a swatch of each one's paint and a count of its failing checks; then text colours, fonts, banners, the Play button and readability.
- **Right pane:** the controls for whatever is chosen on the left.

Every change shows in the main window at once, and Undo (⌘Z) takes it back. A built-in skin is copied on its first change. **Export…** writes the pack as a `.zip`. The switches under "Show in the window" open the Settings window, a menu or a notice, so you can see the surfaces they use.

## 5. Check and test with the CLI

`lodestone-launcher-cli` talks to the running launcher (see its SKILL.md). Use a development launcher with its own data folder, not your everyday one.

```sh
# validate a pack and check its text, without importing it
lodestone-launcher-cli skins check --path ./cozy
lodestone-launcher-cli --json skins check --path ./cozy | jq '.checks[] | select(.passes | not)'

# import, use, and check the skin in use
lodestone-launcher-cli skins import ./cozy --use
lodestone-launcher-cli skins check

# apply the suggested fixes to an imported skin, then export it
lodestone-launcher-cli skins fix cozy-cabin
lodestone-launcher-cli skins export ./cozy-cabin.zip --id cozy-cabin

# go back to the default look
lodestone-launcher-cli skins use default
```

**Reading `skins check`:**
- `passes: true` means nothing falls short.
- Each check has `surface`, `role` (`text`, `muted`, `faint` or `label`), `color`, `ratio`, `target` and a `fix` such as `surfaces.sidebar.muted = #D9CDBE`.
- An invalid pack gives `valid: false` with the reason. Fix it and run the check again.

**Where the window is now:** `lodestone-launcher-cli trace snapshot` reports the page, tab, dialog, size and zoom. `lodestone-launcher-cli trace watch settings.change` shows each skin edit as it is saved.

**Pictures without a window** (a development build with the `screenshot` feature):

```sh
cargo build -p lodestone-launcher --features screenshot
B=target/debug/lodestone-launcher
timeout 60 $B --scene skin-grassblock --screenshot grassblock.png   # a built-in skin
timeout 60 $B --scene skin-showcase --screenshot showcase.png       # gradients and fonts
timeout 60 $B --scene skin-materials --screenshot materials.png     # frosted, blurred and translucent panels
timeout 60 $B --scene settings-skin --screenshot settings.png       # the Settings window
timeout 60 $B --scene skin-editor-readability --screenshot ed.png   # the editor's suggestions
```

The scenes use made-up data in a temporary folder, never your own. To picture your own pack, import it into a development launcher and look at the window.

**Safe mode:** if a skin makes the window unusable, start the launcher with `--no-skin` (or `LODESTONE_LAUNCHER_NO_SKIN=1`) and choose another skin.

## 6. An example, end to end

The steps:

1. Make the folder:

   ```sh
   mkdir cozy
   ```

2. Add the images and font, named exactly as `skin.json` names them:
   - `planks.png`: a 32 × 32 pattern you made;
   - `meadow.jpg`: a photo you own;
   - `cozy-serif.ttf`: a font whose licence allows redistribution.

3. Write `cozy/skin.json`:

```json
{
  "schema": 1,
  "name": "Cozy Cabin",
  "author": "you",
  "accent": "#E0A458",
  "surfaces": {
    "window": { "fill": "#1C1612" },
    "top_bar": { "fill": { "gradient": { "stops": ["#3A2A1C", "#2A1F15"] } } },
    "sidebar": {
      "fill": { "image": { "image": "planks.png", "fit": "tile", "tint": "#140E08", "tint_opacity": 60 } },
      "text": "#F4EEE6",
      "muted": "#D9CDBE"
    },
    "play_bar": { "fill": { "gradient": { "stops": ["#2A1F15", "#1E160F"] } } },
    "card": { "fill": "#2A2018", "border": "#4A3A2A", "radius": 6 },
    "setting_card": { "fill": "#2A2018", "border": "#4A3A2A", "radius": 6 },
    "dialog": { "fill": "#221A13", "border": "#4A3A2A", "radius": 10 },
    "popover": { "fill": "#2E2319", "border": "#4A3A2A", "radius": 8 },
    "button_primary": { "fill": "#B5651D", "radius": 4 },
    "toggle": { "fill": "#E0A458" },
    "pill": { "fill": "#B5651D", "text": "#FFFFFF" },
    "play_button": { "fill": { "gradient": { "angle": 90, "stops": ["#E0A458", "#B5651D"] } } },
    "progress": { "fill": "#E0A458" }
  },
  "tokens": { "text": "#F2EADF", "secondary": "#D8CBBB", "muted": "#B7A894", "faint": "#A39380", "border": "#4A3A2A" },
  "fonts": { "heading": "cozy-serif.ttf", "play": "cozy-serif.ttf" },
  "banners": [
    { "id": "meadow", "name": "Meadow", "image": "meadow.jpg" },
    { "id": "night", "name": "Night", "image": "builtin:night" }
  ],
  "hero": { "image": "meadow.jpg", "scrim": 45 },
  "play": { "shape": "rounded" }
}
```

4. Check it, import it and look:

```sh
lodestone-launcher-cli skins check --path ./cozy     # fix anything that falls short
lodestone-launcher-cli skins import ./cozy --use
lodestone-launcher-cli skins check                   # again, with the images measured in the launcher
```

Then look at the window: the sidebar, the Play tab, Settings, a menu and a notice. The editor's preview switches show the last three.

5. Share it:

```sh
lodestone-launcher-cli skins export ./cozy-cabin.zip --id cozy-cabin
```

Anyone can import the `.zip` in Settings → Skin → Import….

## Limits at a glance

| | |
|---|---|
| pack | 20 MB; `skin.json`, images and fonts only |
| name, author | 48 and 64 characters |
| image decode | 8192 px a side; PNG, JPEG, WebP |
| stored image | sidebar 1024 × 2048; hero and banners 2560 × 1600; other surfaces 2048 × 2048 |
| gradient | 2–8 stops; angle −360 to 360 |
| border, radius, blur | 0–4 px, 0–32 px, 0–20 px |
| material | blur 0–40 px; opacity, frost, grain 0–100 |
| fonts | .ttf/.otf, 2 MB each, 4 per pack |
| banners | 16; ids 1–32 of `a-z 0-9 -` |
| Play label | 16 characters |
| opacity | window 30–100; each surface 0–100 |
