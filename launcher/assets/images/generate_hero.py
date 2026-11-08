#!/usr/bin/env python3
"""Generates hero.svg, the Play tab's default banner: original artwork in the dashboard's
palette (a field of isometric lodestone blocks under soft glows)."""
import math, random, sys
random.seed(7)
W, H = 1600, 640
out = []
o = out.append
o(f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" width="{W}" height="{H}">')
o('<defs>')
o('<linearGradient id="sky" x1="0" y1="0" x2="1" y2="1">'
  '<stop offset="0" stop-color="#1D1E21"/><stop offset="0.55" stop-color="#1B2B3C"/>'
  '<stop offset="1" stop-color="#13668A"/></linearGradient>')
o('<radialGradient id="glowBlue" cx="0.5" cy="0.5" r="0.5">'
  '<stop offset="0" stop-color="#59B2F3" stop-opacity="0.55"/>'
  '<stop offset="1" stop-color="#59B2F3" stop-opacity="0"/></radialGradient>')
o('<radialGradient id="glowViolet" cx="0.5" cy="0.5" r="0.5">'
  '<stop offset="0" stop-color="#8736C7" stop-opacity="0.45"/>'
  '<stop offset="1" stop-color="#8736C7" stop-opacity="0"/></radialGradient>')
o('<linearGradient id="fade" x1="0" y1="0" x2="1" y2="0">'
  '<stop offset="0" stop-color="#1D1E21" stop-opacity="0.92"/>'
  '<stop offset="0.5" stop-color="#1D1E21" stop-opacity="0.35"/>'
  '<stop offset="1" stop-color="#1D1E21" stop-opacity="0"/></linearGradient>')
o('<linearGradient id="floor" x1="0" y1="0" x2="0" y2="1">'
  '<stop offset="0" stop-color="#1D1E21" stop-opacity="0"/>'
  '<stop offset="1" stop-color="#1D1E21" stop-opacity="0.85"/></linearGradient>')
o('</defs>')
o(f'<rect width="{W}" height="{H}" fill="url(#sky)"/>')
o(f'<ellipse cx="1180" cy="230" rx="520" ry="360" fill="url(#glowBlue)"/>')
o(f'<ellipse cx="1450" cy="520" rx="420" ry="300" fill="url(#glowViolet)"/>')
o(f'<ellipse cx="760" cy="90" rx="300" ry="200" fill="url(#glowViolet)" opacity="0.5"/>')
palettes = [
    ("#3C85BA", "#13668A", "#334675"),   # blue lodestone
    ("#59B2F3", "#1D8EB2", "#13668A"),   # bright blue
    ("#8736C7", "#5B2A8C", "#3E2366"),   # violet
    ("#44464B", "#303338", "#26282C"),   # stone
    ("#36393F", "#2B2D32", "#212327"),   # dark stone
]
def cube(cx, cy, s, top, left, right, edge="#E3E3E4", edge_op=0.10, op=1.0):
    h = s * 0.5
    t = [(cx, cy - s), (cx + s * 0.866, cy - s + h), (cx, cy), (cx - s * 0.866, cy - s + h)]
    l = [(cx - s * 0.866, cy - s + h), (cx, cy), (cx, cy + s), (cx - s * 0.866, cy + h)]
    r = [(cx, cy), (cx + s * 0.866, cy - s + h), (cx + s * 0.866, cy + h), (cx, cy + s)]
    for pts, c in ((l, left), (r, right), (t, top)):
        d = ' '.join(f'{x:.1f},{y:.1f}' for x, y in pts)
        o(f'<polygon points="{d}" fill="{c}" fill-opacity="{op}" stroke="{edge}" stroke-opacity="{edge_op}" stroke-width="1"/>')

# A low hill of blocks on the right, drawn back to front on an isometric grid.
S = 46
ox, oy = 1090, 150
N = 9
def height(i, j):
    d = ((i - 3.2) ** 2 + (j - 3.6) ** 2) / 9.0
    return max(0, round(4.2 * math.exp(-d) + random.choice([0, 0, 0, 1, -1])))
cells = [(i, j, height(i, j)) for i in range(N) for j in range(N)]
cells.sort(key=lambda c: (c[0] + c[1], c[0]))
for i, j, h in cells:
    for level in range(max(h, 1)):
        x = ox + (i - j) * S * 0.866
        y = oy + (i + j) * S * 0.5 - level * S
        top = level == max(h, 1) - 1
        r = random.random()
        if top and r < 0.16:
            p = palettes[1]
        elif top and r < 0.26:
            p = palettes[2]
        elif r < 0.12:
            p = palettes[0]
        else:
            p = palettes[3] if (i + j + level) % 3 else palettes[4]
        cube(x, y, S, *p)
# Floating crystals above the hill.
for (x, y, s, p) in [(920, 120, 22, 1), (1240, 70, 18, 2), (1380, 170, 26, 0), (820, 260, 14, 1), (1140, 30, 12, 0)]:
    cube(x, y, s, *palettes[p], edge_op=0.25)
o(f'<rect width="{W}" height="{H}" fill="url(#fade)"/>')
o(f'<rect y="{H*0.55:.0f}" width="{W}" height="{H*0.45:.0f}" fill="url(#floor)"/>')
o('</svg>')
open(sys.argv[1], 'w').write('\n'.join(out) + '\n')
