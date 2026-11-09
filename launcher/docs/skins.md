# Launcher skins

A skin changes how Lodestone Launcher looks: the sidebar's background, the Play tab's
image, the Play button, the accent colour and the progress bar. Choose one in
**Settings → Launcher → Skin**, or with `lodestone-launcher-cli skins use <id>`.

The built-in skins are `default`, `grassblock`, `glass` and `neon`. Their images are
drawn by the launcher; a pack can use them too, as `builtin:dirt`, `builtin:sky`,
`builtin:glass` and `builtin:grid`.

An instance can override the skin's **Play button colour** and **accent** in its own
Settings → General. Its Play background (image or slideshow) also wins over the skin's
Play image.

## Packs

A skin is a pack: a folder, or a `.zip` of one, holding `skin.json` and the images it
names, and nothing else. Imported packs are kept in `<launcher data>/skins/<id>/`.
The format is meant to be shared with other Lodestone apps later; it holds only data.

```json
{
  "schema": 1,
  "name": "Cozy",
  "author": "you",
  "accent": "#8BD86A",
  "progress": "#6FC24A",
  "sidebar": {
    "image": "planks.png",
    "fit": "tile",
    "tint": "#140E08",
    "tint_opacity": 55,
    "blur": 0
  },
  "banner": { "in_sidebar": false },
  "hero": { "image": "meadow.jpg" },
  "play": {
    "color": "#5DB83B",
    "gradient": "#3C8A22",
    "shape": "pixel",
    "label": "GO",
    "glow": false
  }
}
```

| Key | Meaning |
|---|---|
| `schema` | Always `1` for now. Other versions are refused. |
| `name`, `author` | Up to 48 and 64 characters. |
| `accent` | Links, focus rings, selected tabs, primary buttons. Made readable when used: links keep 4.5:1 on the dark panels, button text keeps 3:1. |
| `progress` | Progress bars, such as the Play button's while the game prepares. |
| `sidebar.image` | An image in the pack, or a `builtin:` image. |
| `sidebar.fit` | `cover` (default) or `tile` (repeated at its size, for a pattern). |
| `sidebar.tint`, `sidebar.tint_opacity` | A colour over the sidebar (0–100, default 60). |
| `sidebar.blur` | 0–20 pixels, applied once when the skin loads. |
| `banner.in_sidebar` | Each instance's Play background, faintly, behind its sidebar row. |
| `hero.image` | The Play tab's image for instances without their own. |
| `play.color`, `play.gradient` | The Play button's colour, and a second colour to fade to. Text is white, or dark on a light colour. |
| `play.shape` | `rounded` (default), `pill` or `pixel`. |
| `play.label` | Instead of "PLAY", at most 16 characters. |
| `play.glow` | A soft glow in the button's colour. |

Colours are `#RRGGBB`. Every key but `schema` and `name` is optional.

## What importing checks

- `skin.json` must parse with only the keys above; anything else is refused.
- The pack holds `skin.json` and the images it names, nothing more: no scripts, HTML or
  other files. A `.zip` may have them in one top folder.
- Image names are plain file names (`.png`, `.jpg`, `.jpeg`, `.webp`): no folders, no
  `..`, no absolute paths.
- Each image must decode as PNG, JPEG or WebP (at most 8192 px a side). It is re-encoded
  and scaled down: the sidebar's to 1024 × 2048, the Play image's to 2560 × 1600.
- The pack may hold at most 20 MB.

## Safe mode

Start the launcher with `--no-skin`, or set `LODESTONE_LAUNCHER_NO_SKIN=1`, to show the
default look whatever skin is chosen. A skin that can't be read also falls back to the
default look; Settings → Skin says why.
