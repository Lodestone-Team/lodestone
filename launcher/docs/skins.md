# Launcher skins

A skin changes how Lodestone Launcher looks. It can paint every surface of the
window (with a colour, a gradient or an image), set the theme's text and border
colours, pick fonts for each part of the window, and give new instances banners.
Choose one in **Settings → Launcher → Skin**, or with
`lodestone-launcher-cli skins use <id>`.

The built-in skins are `default`, `grassblock`, `glass` and `neon`. Their images are
drawn by the launcher. A pack can use them too: `builtin:dirt`, `builtin:sky`,
`builtin:sunset`, `builtin:night`, `builtin:glass`, `builtin:aurora`, `builtin:grid`
and `builtin:grid-dawn`.

An instance can override the skin's **Play button colour** and **accent** in its own
Settings → General. Its Play background (a skin banner, an image or a slideshow) also
wins over the skin's Play image.

## Packs

A skin is a pack: a folder, or a `.zip` of one, holding `skin.json`, the images it
names and the font files it names, and nothing else. Imported packs are kept in
`<launcher data>/skins/<id>/`. The format holds only data.

```json
{
  "schema": 1,
  "name": "Cozy",
  "author": "you",
  "accent": "#8BD86A",
  "surfaces": {
    "sidebar": {
      "fill": { "image": { "image": "planks.png", "fit": "tile", "tint": "#140E08", "tint_opacity": 55 } },
      "text": "#F4EEE6",
      "muted": "#D9CDBE"
    },
    "top_bar": { "fill": { "gradient": { "angle": 180, "stops": ["#3A2A1C", "#2A1F15"] } } },
    "card": { "fill": "#2A2118", "border": "#4A3A2A", "radius": 4 },
    "button_primary": { "fill": "#3F8F2A", "radius": 2 },
    "play_button": { "fill": { "gradient": { "stops": ["#5DB83B", "#3C8A22"] } } }
  },
  "tokens": { "muted": "#A29C94", "faint": "#BDB5AB" },
  "fonts": { "heading": "Lodestone Pixel", "body": "cozy-sans.ttf" },
  "banners": [
    { "id": "meadow", "name": "Meadow", "image": "meadow.jpg" },
    { "id": "night", "image": "builtin:night" }
  ],
  "hero": { "image": "meadow.jpg", "scrim": 40 },
  "play": { "shape": "pixel", "label": "GO" }
}
```

### Top-level keys

| Key | Meaning |
|---|---|
| `schema` | Always `1` for now. Other versions are refused. |
| `name`, `author` | Up to 48 and 64 characters. |
| `accent` | Links, focus rings, selected tabs, primary buttons. |
| `progress` | Progress bars (a `progress` surface wins over it). |
| `surfaces` | How each surface is painted, by surface id (below). |
| `tokens` | The theme's text and border colours: `text`, `secondary`, `muted`, `faint`, `border`. They apply wherever a surface has no text colour of its own. |
| `fonts` | Fonts for `heading`, `body`, `sidebar`, `button`, `play`, `tab` and `mono`: a bundled family (`Satoshi`, `Chillax`, `Clash Grotesk`, `JetBrains Mono`, `Lodestone Pixel`) or a `.ttf`/`.otf` file in the pack. |
| `banners` | Up to 16 Play backgrounds the skin gives instances: `id` (lowercase letters, digits and dashes, kept stable across updates), an optional `name`, and `image`. |
| `hero.image` | The Play tab's image for instances without their own (the same as `surfaces.hero.fill.image`). |
| `hero.scrim` | 0–100: a shade under the Play tab's title, so it reads on a bright image. |
| `play.shape` | `rounded` (default), `pill` or `pixel`. |
| `play.label` | Instead of "PLAY", at most 16 characters. |
| `play.glow` | A soft glow in the button's colour. |
| `banner.in_sidebar` | Each instance's Play background, faintly, behind its sidebar row. |
| `sidebar.*`, `play.color`, `play.gradient` | The older way to paint the sidebar and the Play button; still read. `surfaces` wins where both are given. |
| `sidebar.readability`, `hero.readability` | `off` (default) or `auto`. See [Readable text](#readable-text). |

Colours are `#RRGGBB`. Every key but `schema` and `name` is optional.

### Surfaces

`window`, `top_bar`, `sidebar`, `main` (the page), `hero` (the Play tab's picture),
`play_bar`, `card`, `setting_card`, `row` (content, worlds and server rows),
`dialog`, `settings_nav`, `popover` (menus and tooltips), `input`, `button_primary`,
`button_secondary`, `button_ghost`, `button_danger`, `toggle` (a switch that is on),
`tab` (the chosen tab), `pill` (the chosen segment), `play_button`, `progress` and
`toast`.

Each surface takes:

| Key | Meaning |
|---|---|
| `fill` | `"#RRGGBB"`; `{"solid": {"color": "#RRGGBB", "opacity": 0–100}}`; `{"gradient": {"kind": "linear" or "radial", "angle": degrees (CSS: 180, the default, runs top to bottom), "stops": ["#RRGGBB", …] or [{"color", "at": 0–100}, …]}}` with 2–8 stops; or `{"image": {"image": file or builtin, "fit": "cover" or "tile", "tint", "tint_opacity": 0–100, "blur": 0–20}}`. Images are for the large surfaces (not inputs, buttons other than Play, switches, tabs, pills or progress bars). |
| `border`, `border_width` | A border colour, 0–4 pixels wide (1 by default). |
| `radius` | Corner radius, 0–32 pixels. |
| `text` | The surface's own text colour, on `sidebar`, `toast` and the controls (buttons, `tab`, `pill`, `play_button`). Elsewhere text follows `tokens`. A control's label without one is white or near-black, whichever reads better on its fill. |
| `muted` | The sidebar's quieter text: headers, subtitles, counts, the footer and icons. |

A see-through `main` (a solid fill with `opacity` under 100) shows the `window`'s
fill through it.

### Fonts and banners

Pack fonts are checked when the pack is imported: a single TrueType or OpenType font
(no collections or web fonts), at most 2 MB, at most 4 per pack, with family, full
and PostScript names. They are handed to the system's text engine like the fonts the
launcher ships. A part whose font can't be used keeps the launcher's.

When an instance is created, it takes one of the active skin's banners at random.
Afterwards its **Play background** picker offers the skin's banners under **From the
skin**, besides its own image or slideshow. The instance keeps the banner by skin and
banner id (`skin:<skin>/<banner>`), so a skin update that keeps the id keeps the
banner. If the skin or banner is gone, the default Play picture shows.

## Readable text

Readability is the skin's responsibility. The launcher doesn't change a skin's
colours or add shades behind its text unless the pack asks for it.

What it does instead is measure and suggest. When a skin loads, each image is
measured after its blur: its average colour, and its light and dark ends (the 90th
and 10th percentile of brightness). Each surface the skin changes is then checked:
its text, muted text and hints against the worse end of what the surface shows
(its fill over what is under it, and an image under its tint). Body text and
subtitles need 4.5:1, labels on controls and hints 3:1, the Play tab's title 4.5:1
under its `scrim`.

The skin editor lists what falls short, such as "Sidebar, muted text: 3.1:1, needs
4.5:1", each with **Apply suggested fix**, and **Fix all**. A fix is the smallest change
that gets there:

- the surface's own text colour, where it has one (the sidebar, notices, controls);
- else the surface's colour, or a stronger tint on its image, so the theme's text
  reads on it;
- else the theme's shared colour (`tokens`);
- a `hero.scrim` for the Play picture.

`lodestone-launcher-cli skins check <id>` prints the same list. The built-in skins
pass every check.

`readability: "auto"` on `sidebar` or `hero` is still read, as an opt-in: the launcher
then works out the sidebar's text colours (and a light or dark backing, when colours
alone can't do it) or the Play picture's shade from the image itself, and the editor
leaves those surfaces to it.

## What importing checks

- `skin.json` must parse with only the keys above; anything else is refused.
- The pack holds `skin.json`, the images and the fonts it names, nothing more: no
  scripts, HTML or other files. A `.zip` may have them in one top folder.
- Image and font names are plain file names (`.png`, `.jpg`, `.jpeg`, `.webp`;
  `.ttf`, `.otf`): no folders, no `..`, no absolute paths.
- Each image must decode as PNG, JPEG or WebP (at most 8192 px a side). It is re-encoded
  and scaled down: the sidebar's to 1024 × 2048, the Play image's and banners to
  2560 × 1600, other surfaces' to 2048 × 2048.
- Each font must pass the checks above.
- The pack may hold at most 20 MB.

## Safe mode

Start the launcher with `--no-skin`, or set `LODESTONE_LAUNCHER_NO_SKIN=1`, to show the
default look whatever skin is chosen. A skin that can't be read also falls back to the
default look; Settings → Skin says why.

## The bundled pixel font

`Lodestone Pixel` (`assets/fonts/lodestone-pixel/`) is the launcher's own 5 × 7
pixel font, drawn for this project and built by `scripts/make-pixel-font.py`. It is
under the SIL Open Font License 1.1 (`OFL.txt` beside it). Grassblock uses it for
headings and the Play button.
