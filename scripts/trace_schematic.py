"""Extrae segmentos ortogonales (cables) de una zona del esquema rasterizado.

Apoya la digitalización del netlist desde el PDF: los cables del esquema 5/67
son horizontales o verticales; las resistencias (zigzag) y los potenciómetros
no aparecen como segmentos largos y se ubican con los recortes ampliados.

Uso:
    python scripts/trace_schematic.py X0 Y0 X1 Y1 [umbral]

Coordenadas en píxeles de `extracted/b18n-b15nd_schematic_5_67_page01_image01.png`.
"""

from __future__ import annotations

import sys

import numpy as np
from PIL import Image

SOURCE = "extracted/b18n-b15nd_schematic_5_67_page01_image01.png"


def runs(bools: np.ndarray, min_len: int) -> list[tuple[int, int]]:
    out: list[tuple[int, int]] = []
    start = None
    for index, value in enumerate(bools):
        if value and start is None:
            start = index
        elif not value and start is not None:
            if index - start >= min_len:
                out.append((start, index - 1))
            start = None
    if start is not None and len(bools) - start >= min_len:
        out.append((start, len(bools) - 1))
    return out


def group(segments: list[tuple[int, int, int]]) -> list[dict]:
    groups: list[dict] = []
    for (line, start, end) in sorted(segments):
        for g in groups:
            if line - g["line_max"] <= 2 and not (end < g["start"] - 4 or start > g["end"] + 4):
                g["line_max"] = line
                g["start"] = min(g["start"], start)
                g["end"] = max(g["end"], end)
                break
        else:
            groups.append({"line_min": line, "line_max": line, "start": start, "end": end})
    return groups


def main(argv: list[str]) -> int:
    if len(argv) < 5:
        print(__doc__)
        return 2
    x0, y0, x1, y1 = (int(value) for value in argv[1:5])
    threshold = int(argv[5]) if len(argv) > 5 else 175

    image = Image.open(SOURCE).convert("L")
    region = np.array(image.crop((x0, y0, x1, y1)))
    mask = region < threshold

    horizontal: list[tuple[int, int, int]] = []
    for y in range(mask.shape[0]):
        for (start, end) in runs(mask[y], 11):
            horizontal.append((y, start, end))
    vertical: list[tuple[int, int, int]] = []
    for x in range(mask.shape[1]):
        for (start, end) in runs(mask[:, x], 11):
            vertical.append((x, start, end))

    print("--- cables horizontales (y, x0..x1) ---")
    for g in group(horizontal):
        y = y0 + (g["line_min"] + g["line_max"]) / 2
        print(f"y={y:6.1f}  x={x0 + g['start']:4d}..{x0 + g['end']:4d}")
    print("--- cables verticales (x, y0..y1) ---")
    for g in group(vertical):
        x = x0 + (g["line_min"] + g["line_max"]) / 2
        print(f"x={x:6.1f}  y={y0 + g['start']:4d}..{y0 + g['end']:4d}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
