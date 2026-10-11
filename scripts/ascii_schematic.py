"""Imprime un mapa ASCII de una zona del esquema rasterizado (1 char = 1 px).

Permite leer conexiones exactas sin ambigüedad de escala.

Uso:
    python scripts/ascii_schematic.py X0 Y0 X1 Y1 [umbral]
"""

from __future__ import annotations

import sys

import numpy as np
from PIL import Image

SOURCE = "extracted/b18n-b15nd_schematic_5_67_page01_image01.png"


def main(argv: list[str]) -> int:
    if len(argv) < 5:
        print(__doc__)
        return 2
    x0, y0, x1, y1 = (int(value) for value in argv[1:5])
    threshold = int(argv[5]) if len(argv) > 5 else 175

    image = Image.open(SOURCE).convert("L")
    region = np.array(image.crop((x0, y0, x1, y1)))
    mask = region < threshold

    header = "     " + "".join(str((x0 + i) // 100 % 10) for i in range(mask.shape[1]))
    print(header)
    header2 = "     " + "".join(str((x0 + i) // 10 % 10) for i in range(mask.shape[1]))
    print(header2)
    for y in range(mask.shape[0]):
        row = "".join("#" if value else "." for value in mask[y])
        print(f"{y0 + y:4d} {row}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
