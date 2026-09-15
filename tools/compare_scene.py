"""Compares screenshots of the login scene against an H5 client screenshot.

Samples the same scene points in each image (positions relative to the view center, in fractions of
the view width, since both use a 50° horizontal field of view) and prints each point's average color
with the summed per-channel error against the reference.

Usage: python tools/compare_scene.py <h5.png> <x0,y0,x1,y1> <ours.png> [<ours.png> ...]
The rectangle is the H5 screenshot's view area; our captures use the Canastra window's view area.
"""

import sys

from PIL import Image

OURS_VIEW = (8, 31, 1288, 751)
POINTS = {
    "sky top-mid": (-0.10, -0.24),
    "sky top-right": (0.40, -0.24),
    "sky mid-left": (-0.40, -0.05),
    "sky right mid": (0.35, 0.02),
    "sky low-left": (-0.45, 0.10),
    "sky low-mid": (0.00, 0.12),
    "sky low-right": (0.35, 0.12),
    "moon center": (0.30, -0.14),
    "trunk": (-0.285, 0.16),
    "hill left": (-0.35, 0.225),
    "hill right": (0.35, 0.225),
    "canopy": (-0.27, -0.02),
}


def sample(image, view, point, radius=6):
    x0, y0, x1, y1 = view
    width = x1 - x0
    x = int(x0 + width / 2 + point[0] * width)
    y = int(y0 + (y1 - y0) / 2 + point[1] * width)
    x = max(x0 + radius, min(x1 - radius - 1, x))
    y = max(y0 + radius, min(y1 - radius - 1, y))
    pixels = [image.getpixel((x + dx, y + dy)) for dx in range(-radius, radius + 1) for dy in range(-radius, radius + 1)]
    return tuple(sum(pixel[channel] for pixel in pixels) // len(pixels) for channel in range(3))


def main():
    reference = Image.open(sys.argv[1]).convert("RGB")
    reference_view = tuple(int(value) for value in sys.argv[2].split(","))
    captures = [(path, Image.open(path).convert("RGB")) for path in sys.argv[3:]]
    errors = [0] * len(captures)
    for name, point in POINTS.items():
        expected = sample(reference, reference_view, point)
        row = f"{name:14s} {str(expected):>16s}"
        for index, (_, capture) in enumerate(captures):
            color = sample(capture, OURS_VIEW, point)
            errors[index] += sum(abs(a - b) for a, b in zip(color, expected))
            row += f" {str(color):>16s}"
        print(row)
    print(f"{'error':14s} {'':>16s}" + "".join(f" {error:>16d}" for error in errors))


if __name__ == "__main__":
    main()
