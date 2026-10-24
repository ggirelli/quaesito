# /// script
# requires-python = ">=3.11"
# dependencies = ["pillow>=11"]
# ///
"""Prepare the ready-made team pictures from their full-size originals.

Reads assets/teams/originals/<name>.png (git-ignored) and writes assets/teams/<name>.png: the
character cut out (a checkerboard drawn in place of transparency is removed), centred in a square
with room for the round frame it is shown in, at the app's team picture size.
Run with `uv run tools/assets/team-mascots.py`.
"""

from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[2]
ORIGINALS, OUT = ROOT / "assets/teams/originals", ROOT / "assets/teams"
SIZE = 256  # the app's team picture size (AVATAR_SIZE in crates/server/src/media.rs)
FILL = 0.84  # share of the square the character spans


def remove_checkerboard(image):
    """Make transparent the grey checkerboard squares connected to the image's edge.

    Generators sometimes draw a checkerboard instead of leaving the background transparent. Its
    squares are mid greys; the sticker's white border and dark outline stop the fill.
    """
    rgb = image.convert("RGB")
    mask = Image.new("L", rgb.size, 0)
    px, mpx = rgb.load(), mask.load()
    w, h = rgb.size
    for y in range(h):
        for x in range(w):
            r, g, b = px[x, y]
            if max(r, g, b) - min(r, g, b) < 28 and 100 <= (r + g + b) / 3 <= 225:
                mpx[x, y] = 255
    for x, y in [(x, 0) for x in range(w)] + [(x, h - 1) for x in range(w)] + \
                [(0, y) for y in range(h)] + [(w - 1, y) for y in range(h)]:
        if mpx[x, y] == 255:
            ImageDraw.floodfill(mask, (x, y), 128, thresh=0)
    background = mask.point(lambda v: 255 if v == 128 else 0)
    out = image.convert("RGBA")
    out.putalpha(Image.eval(background, lambda v: 255 - v))
    return out


def main():
    for source in sorted(ORIGINALS.glob("*.png")):
        image = Image.open(source)
        if image.mode != "RGBA" or image.getpixel((0, 0))[3] != 0:
            image = remove_checkerboard(image)
        alpha = image.getchannel("A")
        box = alpha.point(lambda v: 255 if v > 16 else 0).getbbox()
        character = image.crop(box)
        scale = FILL * SIZE / max(character.size)
        character = character.resize(
            (round(character.width * scale), round(character.height * scale)), Image.LANCZOS)
        square = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
        square.alpha_composite(
            character, ((SIZE - character.width) // 2, (SIZE - character.height) // 2))
        target = OUT / source.name
        square.save(target, optimize=True)
        print(f"{source.name}: {target.stat().st_size // 1024} KB")


if __name__ == "__main__":
    main()
