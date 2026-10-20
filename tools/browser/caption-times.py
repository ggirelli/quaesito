"""Finds when the on-screen caption pill changes in the demo video: those are the moments each
narration line should start. Prints the times in milliseconds as JSON.

    python3 tools/browser/caption-times.py docs/media/demo.mp4
"""

import json
import subprocess
import sys

FPS = 20
# The caption pill sits at the top centre (in a 1280-wide video).
CROP = "520:44:380:2"
WIDTH, HEIGHT = 520, 44


def main() -> None:
    video = sys.argv[1]
    raw = subprocess.run(
        ["ffmpeg", "-loglevel", "error", "-i", video, "-vf", f"fps={FPS},scale=1280:-2,crop={CROP},format=gray",
         "-f", "rawvideo", "-"],
        check=True, capture_output=True,
    ).stdout
    size = WIDTH * HEIGHT
    frames = [raw[i:i + size] for i in range(0, len(raw) - size + 1, size)]
    changes = []
    for i in range(1, len(frames)):
        a, b = frames[i - 1], frames[i]
        diff = sum(abs(x - y) for x, y in zip(a[::7], b[::7])) / (size / 7)
        if diff > 6 and (not changes or i * 1000 // FPS - changes[-1] > 1000):
            changes.append(i * 1000 // FPS)
    print(json.dumps(changes))


if __name__ == "__main__":
    main()
