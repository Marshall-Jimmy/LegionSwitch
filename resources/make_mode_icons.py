"""Generate LegionZone-inspired metallic mode buttons.

These are original drawings that follow the visual language of the mode
buttons found in LegionZone's local HTML UI (metallic bevel ring, dark inner
face, colored double-arrow glyph). No Lenovo image files are embedded.
"""

from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFilter, ImageFont

ROOT = Path(__file__).resolve().parent
S = 512
CX = CY = (S - 1) / 2.0
SIZES = [48, 96, 144, 192]

MODES = {
    "quiet": "#3C8DFF",
    "balanced": "#D7DEE8",
    "beast": "#E23C48",
}


def rgba(hex_color: str, alpha: float = 1.0) -> np.ndarray:
    hex_color = hex_color.lstrip("#")
    return np.array(
        [int(hex_color[0:2], 16), int(hex_color[2:4], 16), int(hex_color[4:6], 16), 255 * alpha],
        dtype=np.float64,
    )


def lerp(a, b, t):
    return a + (b - a) * t


def vertical_gradient(stops):
    """stops: list of (position 0..1, hex color)."""
    ys = np.linspace(0, 1, S)
    out = np.zeros((S, 3), dtype=np.float64)
    positions = [p for p, _ in stops]
    colors = [rgba(c)[:3] for _, c in stops]
    for i in range(S):
        y = ys[i]
        for j in range(len(stops) - 1):
            if positions[j] <= y <= positions[j + 1]:
                t = (y - positions[j]) / max(1e-6, positions[j + 1] - positions[j])
                out[i] = lerp(colors[j], colors[j + 1], t)
                break
    return out


def radial_gradient(inner_hex, outer_hex, radius):
    yy, xx = np.mgrid[0:S, 0:S]
    r = np.sqrt((xx - CX) ** 2 + (yy - CY) ** 2) / radius
    r = np.clip(r, 0, 1)
    inner = rgba(inner_hex)[:3]
    outer = rgba(outer_hex)[:3]
    out = np.zeros((S, S, 3), dtype=np.float64)
    for i in range(3):
        out[..., i] = inner[i] + (outer[i] - inner[i]) * (r**1.6)
    return out


def circle_mask(radius, feather=1.0):
    yy, xx = np.mgrid[0:S, 0:S]
    r = np.sqrt((xx - CX) ** 2 + (yy - CY) ** 2)
    return np.clip((radius - r) / max(feather, 0.001), 0, 1)


def ring_mask(r_inner, r_outer, feather=1.0):
    return np.clip(circle_mask(r_outer, feather) - circle_mask(r_inner, feather), 0, 1)


def color_image(rgb):
    out = np.zeros((S, S, 4), dtype=np.float64)
    out[..., :3] = rgb
    return out


def compose(base, layer, mask):
    a = mask[..., None]
    base[..., :3] = layer[..., :3] * a + base[..., :3] * (1 - a)
    base[..., 3:4] = np.maximum(base[..., 3:4], mask[..., None] * 255.0)


def make_icon(accent_hex: str) -> Image.Image:
    img = np.zeros((S, S, 4), dtype=np.float64)

    outer_r = 249
    outer_ring_inner = 203
    mid_r = 180
    inner_face_r = 151

    # Soft ambient shadow / colored aura.
    aura = circle_mask(260, 26) * 0.42
    aura_color = color_image(rgba(accent_hex)[:3])
    compose(img, aura_color, aura)

    # Main metallic bezel.
    grad = vertical_gradient(
        [
            (0.00, "#F7F9FC"),
            (0.16, "#D6DDE6"),
            (0.42, "#A7B1BE"),
            (0.70, "#737D8B"),
            (0.88, "#59626E"),
            (1.00, "#3F4854"),
        ]
    )
    yy, xx = np.mgrid[0:S, 0:S]
    theta = np.arctan2(yy - CY, xx - CX)
    bevel = 0.82 + 0.28 * np.cos(theta + np.pi * 0.75)
    bevel = np.clip(bevel, 0.55, 1.15)
    bezel_rgb = np.clip(grad[:, None, :] * bevel[..., None], 0, 255)
    bezel = color_image([0, 0, 0])
    bezel[..., :3] = bezel_rgb
    compose(img, bezel, ring_mask(outer_ring_inner, outer_r, 2.0))

    # Bright top-left rim.
    rim = color_image(rgba("#FFFFFF")[:3])
    rim_mask = ring_mask(outer_r - 8, outer_r - 1, 2.0) * np.clip((np.cos(theta + 2.35) - 0.1) * 2, 0, 1)
    compose(img, rim, rim_mask * 0.8)

    # Dark separation groove.
    groove = color_image(rgba("#11161D")[:3])
    compose(img, groove, ring_mask(outer_ring_inner - 5, outer_ring_inner + 2, 1.0))

    # Secondary metal ring.
    second = color_image(np.array([190, 198, 208], dtype=np.float64))
    compose(img, second, ring_mask(mid_r, outer_ring_inner - 6, 1.0) * 0.95)
    dark_second = color_image(np.array([35, 42, 52], dtype=np.float64))
    compose(img, dark_second, ring_mask(mid_r - 3, mid_r + 2, 1.0))

    # Inner face.
    face = color_image([0, 0, 0])
    face[..., :3] = radial_gradient("#2A323D", "#080C11", inner_face_r + 22)
    compose(img, face, circle_mask(inner_face_r + 22, 1.5))

    # Colored lower arc (the mode color peeks out under the bezel).
    arc_mask = ring_mask(211, 222, 2.0)
    lower = np.clip((np.sin(theta) + 0.22) * 2.2, 0, 1)
    colored = color_image(rgba(accent_hex)[:3])
    compose(img, colored, arc_mask * lower)

    # Inner dark glass with a subtle top highlight.
    glass = color_image([0, 0, 0])
    glass[..., :3] = radial_gradient("#222A35", "#0B1017", inner_face_r)
    compose(img, glass, circle_mask(inner_face_r, 1.2))
    top_light = np.clip((inner_face_r - np.sqrt((xx - CX) ** 2 + (yy - CY) ** 2)) / inner_face_r, 0, 1)
    top_light = top_light * np.clip((CY - yy) / inner_face_r + 0.2, 0, 1) * 0.16
    highlight = color_image(np.array([120, 145, 175], dtype=np.float64))
    compose(img, highlight, circle_mask(inner_face_r, 1.0) * top_light)

    # Glow + double-arrow glyph.
    glyph = Image.new("L", (S, S), 0)
    draw = ImageDraw.Draw(glyph)
    top = [(-112, -48), (10, -48), (10, -70), (72, -33), (10, 4), (10, -18), (-112, -18)]
    bottom = [(112, 48), (-10, 48), (-10, 70), (-72, 33), (-10, -4), (-10, 18), (112, 18)]
    draw.polygon([(x + CX, y + CY) for x, y in top], fill=255)
    draw.polygon([(x + CX, y + CY) for x, y in bottom], fill=255)

    glow = glyph.filter(ImageFilter.GaussianBlur(9))
    glow_arr = np.array(glow, dtype=np.float64) / 255.0
    glow_color = color_image(rgba(accent_hex)[:3])
    compose(img, glow_color, glow_arr * 0.65)

    glyph_arr = np.array(glyph, dtype=np.float64) / 255.0
    accent = color_image(rgba(accent_hex)[:3])
    compose(img, accent, glyph_arr)

    # Slight top highlight on the arrows.
    arrow_light = glyph_arr * np.clip((CY - yy + 55) / 110, 0, 1) * 0.25
    compose(img, color_image(np.array([255, 255, 255], dtype=np.float64)), arrow_light)

    out = Image.fromarray(np.clip(img, 0, 255).astype(np.uint8), "RGBA")
    return out


def save_ico(name: str, accent_hex: str):
    big = make_icon(accent_hex)
    frames = [big.resize((s, s), Image.Resampling.LANCZOS) for s in SIZES]
    out = ROOT / f"{name}.ico"
    frames[-1].save(
        out,
        format="ICO",
        sizes=[(s, s) for s in SIZES],
        append_images=frames[:-1],
    )
    # Preview sheet: 96px next to a 2x render.
    preview = Image.new("RGBA", (300, 200), "#151A22")
    preview.alpha_composite(frames[SIZES.index(96)], (10, 20))
    preview.alpha_composite(frames[SIZES.index(192)].resize((160, 160), Image.Resampling.LANCZOS), (130, 20))
    preview.save(ROOT / f"{name}-preview.png")
    print(f"wrote {out}")


if __name__ == "__main__":
    for mode, color in MODES.items():
        save_ico(f"mode-{mode}", color)
