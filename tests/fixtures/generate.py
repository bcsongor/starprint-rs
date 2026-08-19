"""Generates golden fixtures from the reference Python implementation in
../starprint/packages/starprint-gui (the hardware-tuned GUI code).

The Rust test suite compares its output byte-for-byte against these files
for everything that is exactly portable: dithering, ESC ^ bit-image
serialisation, and the LUT-based tone-mapping stages (autocontrast, gamma,
equalize, brightness, contrast).

Run with the GUI's virtualenv Python (needs Pillow), from this directory:

    ..\\..\\..\\starprint\\packages\\starprint-gui\\.venv\\Scripts\\python.exe generate.py
"""

from __future__ import annotations

import pathlib
import sys

HERE = pathlib.Path(__file__).resolve().parent
GUI = HERE.parents[2] / "starprint" / "packages" / "starprint-gui"
sys.path.insert(0, str(GUI))

from PIL import Image, ImageEnhance, ImageOps  # noqa: E402

from image import (  # noqa: E402
    PrinterImage,
    _apply_gamma,
    atkinson_dither,
    bayer_dither,
    floyd_steinberg_dither,
    threshold_dither,
)
from star import print_9dot_bit_image, serialise_commands  # noqa: E402

W, H = 37, 23
SRC = bytes(((x * 7 + y * 13 + (x * y) % 31) * 3) % 256 for y in range(H) for x in range(W))
# Low dynamic range variant to exercise autocontrast's rescaling path.
SRC_LOW = bytes(40 + (((x * 7 + y * 13 + (x * y) % 31) * 3) % 156) for y in range(H) for x in range(W))


def write(name: str, data: bytes) -> None:
    (HERE / name).write_bytes(data)
    print(f"{name}: {len(data)} bytes")


write("src.bin", SRC)
write("src_low.bin", SRC_LOW)

img = PrinterImage(W, H, SRC)
write("dither_floyd_steinberg.bin", floyd_steinberg_dither(128)(img).data)
write("dither_atkinson.bin", atkinson_dither(128)(img).data)
write("dither_threshold.bin", threshold_dither(128)(img).data)
write("dither_bayer.bin", bayer_dither()(img).data)

fs = floyd_steinberg_dither(128)(img)
write("bit_image_single.bin", serialise_commands(print_9dot_bit_image(fs, is_double=False)))
write("bit_image_double.bin", serialise_commands(print_9dot_bit_image(fs, is_double=True)))

pil = Image.frombytes("L", (W, H), SRC)
pil_low = Image.frombytes("L", (W, H), SRC_LOW)
write("autocontrast.bin", ImageOps.autocontrast(pil_low).tobytes())
write("gamma_1_8.bin", _apply_gamma(pil, 1.8).tobytes())
write("equalize.bin", ImageOps.equalize(pil).tobytes())
write("brightness_1_2.bin", ImageEnhance.Brightness(pil).enhance(1.2).tobytes())
write("brightness_0_8.bin", ImageEnhance.Brightness(pil).enhance(0.8).tobytes())
write("contrast_1_3.bin", ImageEnhance.Contrast(pil).enhance(1.3).tobytes())
