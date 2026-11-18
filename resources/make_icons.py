"""Generate the LegionSwitch application and tray icons.

Run with any Python 3 + Pillow:
    python resources/make_icons.py
"""

from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parent
SIZES = [16, 20, 24, 32, 40, 48, 64, 128, 256]

COLORS = {
    "legion-switch": "#5AA7FF",
    "quiet": "#3C8DFF",
    "balanced": "#D7DEE8",
    "beast": "#E23C48",
}


def font_path() -> str:
    candidates = [
        r"C:\Windows\Fonts\seguisb.ttf",
        r"C:\Windows\Fonts\segoeuib.ttf",
        r"C:\Windows\Fonts\arialbd.ttf",
    ]
    for candidate in candidates:
        if Path(candidate).exists():
            return candidate
    raise FileNotFoundError("No suitable bold font found")


def draw_icon(size: int, accent: str) -> Image.Image:
    scale = 8
    side = size * scale
    image = Image.new("RGBA", (side, side), (0, 0, 0, 0))
    draw = ImageDraw.Draw(image)

    margin = round(side * 0.055)
    box = (margin, margin, side - margin - 1, side - margin - 1)
    radius = round(side * 0.23)

    # Dark rounded tile with a thin, bright accent ring.
    draw.rounded_rectangle(box, radius=radius, fill="#11151D")
    ring = max(scale * 2, round(side * 0.052))
    draw.rounded_rectangle(box, radius=radius, outline=accent, width=ring)

    # A soft inner highlight keeps the icon from looking flat at large sizes.
    inset = margin + ring + round(side * 0.035)
    if size >= 32:
        draw.rounded_rectangle(
            (inset, inset, side - inset - 1, side - inset - 1),
            radius=max(1, radius - ring),
            outline="#FFFFFF18",
            width=max(1, scale),
        )

    # White "L" monogram.
    font_size = round(side * 0.53)
    font = ImageFont.truetype(font_path(), font_size)
    text = "L"
    left, top, right, bottom = draw.textbbox((0, 0), text, font=font)
    x = (side - (right - left)) / 2 - left
    y = (side - (bottom - top)) / 2 - top - side * 0.025
    draw.text((x, y), text, font=font, fill="#F7FAFF")

    # A small accent notch in the lower-right corner (the "switch" cue).
    dot_r = max(scale * 2, round(side * 0.055))
    cx = round(side * 0.705)
    cy = round(side * 0.705)
    draw.ellipse((cx - dot_r, cy - dot_r, cx + dot_r, cy + dot_r), fill=accent)

    return image.resize((size, size), Image.Resampling.LANCZOS)


def make_ico(name: str, color: str) -> None:
    frames = [draw_icon(size, color) for size in SIZES]
    out = ROOT / f"{name}.ico"
    frames[-1].save(out, format="ICO", sizes=[(s, s) for s in SIZES])
    # Pillow only writes the base image plus requested sizes; save explicitly
    # through append_images to retain every frame.
    frames[-1].save(
        out,
        format="ICO",
        sizes=[(s, s) for s in SIZES],
        append_images=frames[:-1],
    )
    print(f"wrote {out}")


def make_preview() -> None:
    cell = 128
    preview = Image.new("RGBA", (cell * len(COLORS), cell + 24), "#20242C")
    draw = ImageDraw.Draw(preview)
    font = ImageFont.truetype(font_path(), 14)
    for index, (name, color) in enumerate(COLORS.items()):
        icon = draw_icon(128, color)
        preview.alpha_composite(icon, (index * cell + 8, 8))
        label = {"legion-switch": "app", "balanced": "balanced"}.get(name, name)
        draw.text((index * cell + 8, cell + 2), label, font=font, fill="#E7ECF5")
    preview.save(ROOT / "icon-preview.png")
    print(f"wrote {ROOT / 'icon-preview.png'}")


if __name__ == "__main__":
    for name, color in COLORS.items():
        make_ico(name, color)
    make_preview()
