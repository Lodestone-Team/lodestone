#!/usr/bin/env python3
"""Generates the instance icons in images/instance-icons/: original pixel art of blocks
(drawn as isometric cubes from procedural textures) and a few items, each a 128x128 PNG.

The textures are made here from noise and simple patterns, in the spirit of a voxel game
but not copied from any. Only the standard library is used:

    python3 launcher/assets/images/generate_instance_icons.py
"""
import math, os, struct, zlib

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "instance-icons")
GRID = 32  # the art's pixel grid
SCALE = 4  # 32 x 4 = 128 px, scaled with nearest neighbour so the pixels stay sharp


def write_png(path, pixels, size):
    """`pixels` is a list of rows of (r, g, b, a)."""
    raw = b"".join(b"\x00" + bytes(c for px in row for c in px) for row in pixels)
    def chunk(kind, data):
        body = kind + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body) & 0xFFFFFFFF)
    png = b"\x89PNG\r\n\x1a\n"
    png += chunk(b"IHDR", struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0))
    png += chunk(b"IDAT", zlib.compress(raw, 9))
    png += chunk(b"IEND", b"")
    with open(path, "wb") as f:
        f.write(png)


def scaled(grid):
    rows = []
    for row in grid:
        wide = [px for px in row for _ in range(SCALE)]
        rows.extend([wide] * SCALE)
    return rows


def noise(x, y, seed=0):
    """A stable pseudo-random number in [0, 1) for a texel."""
    n = (x * 374761393 + y * 668265263 + seed * 2147483647) & 0xFFFFFFFF
    n = (n ^ (n >> 13)) * 1274126177 & 0xFFFFFFFF
    return ((n ^ (n >> 16)) & 0xFFFF) / 65536.0


def mix(c, amount):
    """Lightens (amount > 0) or darkens (amount < 0) a colour."""
    if amount >= 0:
        return tuple(round(v + (255 - v) * amount) for v in c)
    return tuple(round(v * (1 + amount)) for v in c)


def speckle(base, u, v, seed, spread=0.16):
    return mix(base, (noise(u, v, seed) - 0.5) * 2 * spread)


# --- block textures: (face, u, v) -> rgb, with u and v texel coordinates in 0..15 ----


def tex_solid(base, spread=0.16, seed=1):
    return lambda face, u, v: speckle(base, u, v, seed, spread)


def tex_grass(face, u, v):
    green, dirt = (98, 160, 58), (134, 96, 67)
    if face == "top":
        return speckle(green, u, v, 3, 0.18)
    edge = 3 + (noise(u, 0, 9) > 0.5) + (noise(u, 1, 9) > 0.7)
    return speckle(green if v < edge else dirt, u, v, 4, 0.16)


def tex_snowy(face, u, v):
    snow, dirt = (240, 246, 250), (134, 96, 67)
    if face == "top":
        return speckle(snow, u, v, 5, 0.04)
    edge = 3 + (noise(u, 0, 11) > 0.5)
    return speckle(snow if v < edge else dirt, u, v, 6, 0.08 if v < edge else 0.16)


def tex_cobble(face, u, v):
    stone = (122, 122, 122)
    # irregular cells: the nearest of a few seeded points, with dark mortar between
    best, second = 99, 99
    for cx in range(-1, 3):
        for cy in range(-1, 3):
            px = cx * 6 + noise(cx, cy, 21) * 6
            py = cy * 6 + noise(cx, cy, 22) * 6
            d = math.hypot(u - px, v - py)
            best, second = min(best, d), (second if d >= second else (best if d < best else d))
    if second - best < 1.1:
        return mix(stone, -0.45)
    return speckle(stone, u, v, 23, 0.14)


def tex_log(face, u, v):
    bark, wood = (104, 82, 52), (176, 143, 90)
    if face == "top":
        r = math.hypot(u - 7.5, v - 7.5)
        if r > 6.6:
            return speckle(bark, u, v, 31, 0.1)
        return mix(wood, -0.18 if int(r) % 3 == 0 else 0.0)
    stripe = (u + int(noise(0, v // 4, 32) * 3)) % 4 == 0
    return speckle(mix(bark, -0.25) if stripe else bark, u, v, 33, 0.1)


def tex_planks(face, u, v):
    wood = (166, 132, 80)
    if v % 4 == 3:
        return mix(wood, -0.35)
    if (u + (v // 4) * 7) % 16 == 0:
        return mix(wood, -0.3)
    return speckle(wood, u, v // 2, 41, 0.08)


def tex_leaves(face, u, v):
    leaf = (64, 124, 44)
    n = noise(u, v, 51)
    if n < 0.12:
        return mix(leaf, -0.5)
    return mix(leaf, (n - 0.5) * 0.5)


def tex_ore(color, density=0.82):
    def f(face, u, v):
        stone = speckle((125, 125, 125), u, v, 61, 0.12)
        # small clusters: a texel is ore if a cluster centre is close
        for k in range(4):
            cx = 2 + noise(k, 0, 62 + len(color)) * 11
            cy = 2 + noise(k, 1, 62 + len(color)) * 11
            if abs(u - cx) + abs(v - cy) < 1.6 and noise(u, v, 63) < density:
                return mix(color, (noise(u, v, 64) - 0.5) * 0.3)
        return stone
    return f


def tex_pumpkin(face, u, v):
    orange = (212, 128, 28)
    if face == "top":
        if abs(u - 7.5) < 1 and abs(v - 7.5) < 1:
            return (92, 70, 30)
        return speckle(orange, u, v, 71, 0.1)
    return mix(orange, -0.28) if u % 4 == 0 else speckle(orange, u, v, 72, 0.08)


def tex_bricks(face, u, v):
    brick, mortar = (150, 74, 58), (176, 168, 156)
    if v % 4 == 3:
        return mortar
    if (u + (4 if (v // 4) % 2 else 0)) % 8 == 7:
        return mortar
    return speckle(brick, u, v, 81, 0.12)


def tex_glow(face, u, v):
    n = noise(u // 2, v // 2, 91)
    return mix((222, 168, 72), (n - 0.4) * 0.6)


def tex_obsidian(face, u, v):
    n = noise(u, v, 95)
    base = (24, 18, 36)
    return mix((82, 52, 120), -0.3) if n > 0.9 else mix(base, n * 0.3)


def tex_lodestone(face, u, v):
    rim, core = (84, 88, 98), (95, 209, 163)
    if u in (0, 15) or v in (0, 15):
        return mix(rim, 0.25)
    if 5 <= u <= 10 and 5 <= v <= 10:
        edge = u in (5, 10) or v in (5, 10)
        return mix(core, -0.3) if edge else speckle(core, u, v, 99, 0.12)
    return speckle(rim, u, v, 98, 0.1)


def tex_amethyst(face, u, v):
    n = noise(u // 2, v // 2, 97)
    return mix((140, 96, 200), (n - 0.5) * 0.5)


BLOCKS = [
    ("grass", tex_grass),
    ("dirt", tex_solid((134, 96, 67), 0.18, 2)),
    ("stone", tex_solid((125, 125, 125), 0.12, 7)),
    ("cobblestone", tex_cobble),
    ("sand", tex_solid((219, 205, 160), 0.08, 8)),
    ("gravel", tex_solid((132, 124, 122), 0.3, 10)),
    ("log", tex_log),
    ("planks", tex_planks),
    ("leaves", tex_leaves),
    ("snow", tex_snowy),
    ("ice", tex_solid((150, 188, 250), 0.08, 12)),
    ("diamond-ore", tex_ore((92, 226, 236))),
    ("gold-ore", tex_ore((248, 214, 64))),
    ("emerald-ore", tex_ore((40, 204, 96))),
    ("redstone-ore", tex_ore((220, 30, 30))),
    ("netherrack", tex_solid((112, 52, 50), 0.22, 13)),
    ("end-stone", tex_solid((222, 224, 162), 0.1, 14)),
    ("obsidian", tex_obsidian),
    ("pumpkin", tex_pumpkin),
    ("bricks", tex_bricks),
    ("copper", tex_solid((92, 168, 140), 0.16, 15)),
    ("amethyst", tex_amethyst),
    ("glowstone", tex_glow),
    ("lodestone", tex_lodestone),
]

# --- the isometric cube --------------------------------------------------------------

TOP, RIGHT, BOTTOM, LEFT = (16, 1), (30, 8), (16, 15), (2, 8)
SHADE = {"top": 0.0, "left": -0.22, "right": -0.4}


def solve(p, o, a, b):
    """p = o + s*a + t*b; returns (s, t)."""
    px, py = p[0] - o[0], p[1] - o[1]
    det = a[0] * b[1] - a[1] * b[0]
    return ((px * b[1] - py * b[0]) / det, (a[0] * py - a[1] * px) / det)


def face_at(x, y):
    p = (x + 0.5, y + 0.5)
    s, t = solve(p, TOP, (RIGHT[0] - TOP[0], RIGHT[1] - TOP[1]), (LEFT[0] - TOP[0], LEFT[1] - TOP[1]))
    if 0 <= s < 1 and 0 <= t < 1:
        return "top", t, s
    s, t = solve(p, LEFT, (BOTTOM[0] - LEFT[0], BOTTOM[1] - LEFT[1]), (0, 16))
    if 0 <= s < 1 and 0 <= t < 1:
        return "left", s, t
    s, t = solve(p, BOTTOM, (RIGHT[0] - BOTTOM[0], RIGHT[1] - BOTTOM[1]), (0, 16))
    if 0 <= s < 1 and 0 <= t < 1:
        return "right", s, t
    return None


def cube(texture):
    grid = [[(0, 0, 0, 0)] * GRID for _ in range(GRID)]
    for y in range(GRID):
        for x in range(GRID):
            hit = face_at(x, y)
            if not hit:
                continue
            face, u, v = hit
            rgb = texture(face, min(int(u * 16), 15), min(int(v * 16), 15))
            grid[y][x] = mix(rgb, SHADE[face]) + (255,)
    return outlined(grid)


def outlined(grid, color=(22, 22, 26, 255)):
    """A one-pixel dark outline around the shape, so it reads on any background."""
    out = [row[:] for row in grid]
    for y in range(GRID):
        for x in range(GRID):
            if grid[y][x][3]:
                continue
            for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
                nx, ny = x + dx, y + dy
                if 0 <= nx < GRID and 0 <= ny < GRID and grid[ny][nx][3]:
                    out[y][x] = color
                    break
    return out


# --- items, drawn on a 16-pixel grid and doubled onto the 32-pixel one -----------------


def item(draw):
    small = [[(0, 0, 0, 0)] * 16 for _ in range(16)]
    def put(x, y, rgb):
        if 0 <= x < 16 and 0 <= y < 16:
            small[y][x] = tuple(rgb) + (255,)
    draw(put)
    grid = [[(0, 0, 0, 0)] * GRID for _ in range(GRID)]
    # 16 -> 28 pixels: each pixel doubled, centred with a 2-pixel margin for the outline
    for y in range(16):
        for x in range(16):
            for dy in range(2):
                for dx in range(2):
                    gx, gy = x * 2 + dx, y * 2 + dy
                    if gx < GRID and gy < GRID:
                        grid[gy][gx] = small[y][x]
    return outlined(grid)


def draw_sword(put):
    for k in range(9):
        put(12 - k, 2 + k, (232, 236, 240))
        put(13 - k, 2 + k, (168, 176, 186))
    put(13, 1, (200, 206, 214))
    for x, y in ((1, 9), (2, 10), (3, 11), (4, 12), (5, 13)):
        put(x, y, (200, 160, 60))
    put(2, 12, (110, 74, 40)); put(1, 13, (110, 74, 40))
    put(0, 14, (200, 160, 60))


def draw_pickaxe(put):
    for k in range(10):
        put(3 + k, 13 - k, (122, 86, 48))
    head = [(4, 2), (5, 2), (6, 2), (7, 2), (8, 3), (9, 3), (10, 4), (11, 5), (12, 6), (12, 7), (13, 8), (13, 9), (13, 10)]
    for x, y in head:
        put(x, y, (92, 226, 236))
        put(x + 1, y + 1, (48, 168, 182)) if (x + 1, y + 1) not in head else None


def draw_heart(put):
    for y in range(16):
        for x in range(16):
            u, v = (x - 7.5) / 6.2, -(y - 7.2) / 6.2
            if (u * u + v * v - 1) ** 3 - u * u * v ** 3 <= 0:
                put(x, y, (214, 40, 52))
    put(4, 5, (255, 140, 140)); put(5, 4, (255, 140, 140))


def draw_apple(put):
    for y in range(16):
        for x in range(16):
            if math.hypot((x - 7.5) * 1.05, y - 9) < 5.6:
                put(x, y, (204, 32, 40))
    put(5, 7, (255, 150, 150)); put(5, 8, (255, 150, 150))
    put(8, 3, (100, 70, 30)); put(8, 2, (100, 70, 30))
    put(9, 2, (70, 160, 50)); put(10, 2, (70, 160, 50)); put(10, 1, (70, 160, 50))


def draw_potion(put):
    for y in range(16):
        for x in range(16):
            if math.hypot(x - 7.5, y - 10) < 5:
                put(x, y, (176, 64, 200) if y > 8 else (210, 220, 236))
    for y in range(2, 6):
        put(7, y, (210, 220, 236)); put(8, y, (210, 220, 236))
    put(7, 1, (140, 100, 60)); put(8, 1, (140, 100, 60))
    put(5, 10, (240, 180, 255))


def draw_gem(put):
    for y in range(3, 14):
        half = (y - 2) if y < 7 else (13 - y) * 0.75
        for x in range(16):
            if abs(x - 7.5) <= half + 0.5:
                light = x < 7.5 and y < 9
                put(x, y, (150, 240, 246) if light else (60, 196, 214))


def draw_torch(put):
    for y in range(6, 15):
        put(7, y, (122, 86, 48)); put(8, y, (100, 70, 40))
    put(7, 5, (255, 220, 90)); put(8, 5, (255, 200, 60))
    put(7, 4, (255, 170, 40)); put(8, 4, (255, 240, 150))
    put(7, 3, (255, 140, 30))


def draw_star(put):
    pts = []
    for k in range(10):
        r = 8 if k % 2 == 0 else 3.4
        a = -math.pi / 2 + k * math.pi / 5
        pts.append((8 + r * math.cos(a), 8.6 + r * math.sin(a)))
    def inside(x, y):
        c = False
        for i in range(len(pts)):
            (x1, y1), (x2, y2) = pts[i], pts[i - 1]
            if (y1 > y) != (y2 > y) and x < (x2 - x1) * (y - y1) / (y2 - y1) + x1:
                c = not c
        return c
    for y in range(16):
        for x in range(16):
            if inside(x + 0.5, y + 0.5):
                put(x, y, (255, 214, 64) if x < 8 else (236, 176, 36))


ITEMS = [
    ("sword", draw_sword),
    ("pickaxe", draw_pickaxe),
    ("heart", draw_heart),
    ("apple", draw_apple),
    ("potion", draw_potion),
    ("gem", draw_gem),
    ("torch", draw_torch),
    ("star", draw_star),
]


def main():
    os.makedirs(OUT, exist_ok=True)
    for name, texture in BLOCKS:
        write_png(os.path.join(OUT, f"{name}.png"), scaled(cube(texture)), GRID * SCALE)
    for name, draw in ITEMS:
        write_png(os.path.join(OUT, f"{name}.png"), scaled(item(draw)), GRID * SCALE)
    print(f"wrote {len(BLOCKS) + len(ITEMS)} icons to {OUT}")


if __name__ == "__main__":
    main()
