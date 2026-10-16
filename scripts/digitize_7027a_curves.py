"""Digitaliza curvas del datasheet RCA 7027-A (8-59) a partir del escaneo 1-bit.

Fuente: RCA 7027-A, DATA 1/2 (8-59), `assets/reference/7027A_RCA.pdf` (fuera de
git). Este script lee las imágenes ya extraídas por `scripts/extract_pdfs.py`
en `extracted/` (también fuera de git) y escribe:

    assets/fixtures/reference/power_7027a/7027a_rca_digitized.csv

Figuras usadas:

- p05 (rotada 90°): "Average plate characteristics", GRID-NO.1 = 0 V, familia
  de Ec2 = 50..450 V en pasos de 50 V (9 curvas). Los trazos se dibujan hasta
  Vp ≈ 630 V. Anclaje independiente: la curva Ec2 = 300 V debe pasar por
  Ip ≈ 250 mA en Vp = 300 V (transferencia de p07).
- p07 (sin rotar): "Average characteristics", Vp = 300 V, Ec2 = 300 V;
  transferencia Ip(Vg1) e Ig2(Vg1).

p06 (familia Ip/Ig2 con Vg1 como parámetro a Ec2 = 300 V): la mayoría de sus
curvas no se digitaliza automáticamente (las de Ig2 y las de corte profundo se
entrelazan cerca del eje y las etiquetas se superponen). Se digitaliza **una**
curva de Ig2 sin etiqueta legible (trazada con anclas guiadas) para fijar la
**forma** Ig2(Vp): su subida en la rodilla (Vp ≲ 200 V) es el dato que calibra
la división de pantalla de B18-011. El nivel absoluto se fija con la tabla de
operación típica del datasheet (páginas 1-3), que
`scripts/fit_7027a_model.py` incorpora como puntos tabulados.

Calibración (px por unidad, verificada contra las etiquetas de los ejes):
- p05: x = 142.95 + 1.915·Vp  |  y = 1015.0 − 1.9367·Ip[mA]
- p06: x = 142.0 + 2.3286·Vp  |  y = 925.0 − 2.355·Ig2[mA]
- p07: x = 1010.0 + 19.475·Vg1 |  y = 1865.3 − 3.8267·Ip[mA]
       y = 1850.0 − 17.8·Ig2[mA]

Incertidumbres declaradas: ±5 V y ±3 mA (p05), ±1 V (p07 x, paso de rejilla),
±2 mA (p07 Ip), ±1 mA (p07 Ig2). Los valores se obtienen siguiendo los trazos
del escaneo; no son mediciones de laboratorio.

Uso:
    python scripts/extract_pdfs.py assets/reference extracted
    python scripts/digitize_7027a_curves.py
"""

from __future__ import annotations

import argparse
import csv
from dataclasses import dataclass
from pathlib import Path

import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
EXTRACTED = ROOT / "extracted"
DEFAULT_OUT = (
    ROOT / "assets" / "fixtures" / "reference" / "power_7027a" / "7027a_rca_digitized.csv"
)

SOURCE = "RCA 7027-A datasheet 8-59 (assets/reference/7027A_RCA.pdf)"


@dataclass(frozen=True)
class PageCalibration:
    """Mapeo pixel↔unidad y rejilla de una figura."""

    image: str
    rotate_cw: bool
    x_px_at_unit0: float
    px_per_x: float
    y_px_at_unit0: float
    px_per_y: float
    grid_x_start: float
    grid_x_pitch: float
    grid_y_start: float
    grid_y_pitch: float
    # área de datos (y0, y1, x0, x1)
    plot_box: tuple[int, int, int, int]


P05 = PageCalibration(
    image="7027A_RCA_page05_image01.png",
    rotate_cw=True,
    x_px_at_unit0=142.95,   # Vp = 0 V
    px_per_x=1.915,
    y_px_at_unit0=1015.0,   # Ip = 0 mA
    px_per_y=1.9367,
    grid_x_start=143.0,
    grid_x_pitch=19.22,
    grid_y_start=141.0,
    grid_y_pitch=19.32,
    plot_box=(146, 1012, 150, 1770),
)
P05_CURVES_EC2 = (450.0, 400.0, 350.0, 300.0, 250.0, 200.0, 150.0, 100.0, 50.0)

P07 = PageCalibration(
    image="7027A_RCA_page07_image01.png",
    rotate_cw=False,
    x_px_at_unit0=1010.0,   # Vg1 = 0 V (x crece con Vg1 hacia la derecha)
    px_per_x=19.475,
    y_px_at_unit0=1865.3,   # Ip = 0 mA
    px_per_y=3.8267,
    grid_x_start=234.0,
    grid_x_pitch=19.44,
    grid_y_start=237.0,
    grid_y_pitch=19.36,
    plot_box=(240, 1862, 238, 1004),
)
P07_IG2_PX_AT_ZERO = 1850.0
P07_IG2_PX_PER_MA = 17.8

P06 = PageCalibration(
    image="7027A_RCA_page06_image01.png",
    rotate_cw=True,
    x_px_at_unit0=142.0,    # Vp = 0 V
    px_per_x=2.3286,
    y_px_at_unit0=925.0,    # Ig2 = 0 mA
    px_per_y=2.355,
    grid_x_start=142.0,
    grid_x_pitch=11.66,
    grid_y_start=141.0,
    grid_y_pitch=11.68,
    plot_box=(146, 920, 150, 1770),
)


def load_page(spec: PageCalibration) -> np.ndarray:
    image = Image.open(EXTRACTED / spec.image).convert("L")
    if spec.rotate_cw:
        image = image.transpose(Image.ROTATE_270)
    return np.array(image) < 128


def refined_grid_lines(
    dark: np.ndarray, spec: PageCalibration
) -> tuple[list[float], list[float]]:
    """Ajusta localmente cada línea sintética de la rejilla a la línea real."""

    y0, y1, x0, x1 = spec.plot_box
    vertical: list[float] = []
    x = spec.grid_x_start
    while x < x1:
        best: tuple[float, int] | None = None
        for candidate in np.arange(x - 4.0, x + 4.5, 0.5):
            column = dark[y0:y1, int(round(candidate))]
            coverage = int(column.sum())
            if best is None or coverage > best[1]:
                best = (float(candidate), coverage)
        assert best is not None
        vertical.append(best[0])
        x += spec.grid_x_pitch

    horizontal: list[float] = []
    y = spec.grid_y_start
    while y < y1 - 4:
        best = None
        for candidate in np.arange(y - 4.0, y + 4.5, 0.5):
            row = dark[int(round(candidate)), x0:x1]
            coverage = int(row.sum())
            if best is None or coverage > best[1]:
                best = (float(candidate), coverage)
        assert best is not None
        horizontal.append(best[0])
        y += spec.grid_y_pitch
    return vertical, horizontal


def grid_mask(
    shape: tuple[int, int],
    spec: PageCalibration,
    vertical: list[float],
    horizontal: list[float],
) -> np.ndarray:
    """Máscara de la rejilla (líneas ±2 px) dentro del área de datos."""

    y0, y1, x0, x1 = spec.plot_box
    mask = np.zeros(shape, dtype=bool)
    for gx in vertical:
        lo = max(0, int(round(gx)) - 2)
        hi = min(shape[1], int(round(gx)) + 3)
        mask[y0:y1, lo:hi] = True
    for gy in horizontal:
        lo = max(0, int(round(gy)) - 2)
        hi = min(shape[0], int(round(gy)) + 3)
        mask[lo:hi, x0:x1] = True
    return mask


def runs_of_mask(mask: np.ndarray, x: int, y0: int, y1: int, min_len: int = 3) -> list[float]:
    """Centros de runs de la máscara sin rejilla en la columna `x`."""

    column = mask[y0:y1, x]
    runs: list[float] = []
    y = 0
    n = len(column)
    while y < n:
        if column[y]:
            start = y
            while y < n and column[y]:
                y += 1
            if y - start >= min_len:
                runs.append(0.5 * (start + y) + y0)
        else:
            y += 1
    return runs


def classified_runs(
    dark: np.ndarray,
    x: int,
    y0: int,
    y1: int,
    horizontal: list[float],
) -> list[float]:
    """Runs de una imagen cruda descartando los que son renglones de rejilla."""

    column = dark[y0:y1, x]
    runs: list[float] = []
    y = 0
    n = len(column)
    while y < n:
        if column[y]:
            start = y
            while y < n and column[y]:
                y += 1
            center = 0.5 * (start + y) + y0
            length = y - start
            on_grid_row = any(abs(center - gy) <= 2.5 for gy in horizontal)
            if length >= 5 or (length >= 3 and not on_grid_row):
                runs.append(center)
        else:
            y += 1
    return runs


class Track:
    """Fragmento de trazo continuo muestreado columna a columna."""

    __slots__ = ("xs", "ys", "slope", "miss")

    def __init__(self, x: int, y: float) -> None:
        self.xs: list[int] = [x]
        self.ys: list[float] = [y]
        self.slope = 0.0
        self.miss = 0


def track_columns(
    runs_source,
    spec: PageCalibration,
    x_start: int,
    x_end: int,
    max_coast: int = 80,
    max_active: int = 16,
) -> list[Track]:
    """Sigue trazos de derecha (`x_start`) a izquierda (`x_end`)."""

    active: list[Track] = []
    finished: list[Track] = []
    for x in range(x_start, x_end - 1, -1):
        centers = runs_source(x)
        used: set[int] = set()
        for track in active:
            predicted = track.ys[-1] - track.slope
            best: tuple[float, int] | None = None
            for index, center in enumerate(centers):
                if index in used:
                    continue
                distance = abs(center - predicted)
                if distance <= 14.0 and (best is None or distance < best[0]):
                    best = (distance, index)
            if best is None:
                track.miss += 1
                continue
            used.add(best[1])
            center = centers[best[1]]
            if len(track.ys) >= 2:
                slope = track.ys[-1] - center
                track.slope = 0.7 * track.slope + 0.3 * slope
            track.ys.append(center)
            track.xs.append(x)
            track.miss = 0
        finished.extend(track for track in active if track.miss > max_coast)
        active = [track for track in active if track.miss <= max_coast]
        for index, center in enumerate(centers):
            if index not in used and len(active) < max_active:
                active.append(Track(x, center))
    finished.extend(active)
    return [track for track in finished if len(track.xs) >= 30]


def merge_tracks(tracks: list[Track], max_gap: int = 90, max_offset: float = 10.0) -> list[Track]:
    """Une fragmentos del mismo trazo separados por etiquetas o texto.

    La comparación en la junta extrapola la pendiente del fragmento izquierdo,
    porque en tramos inclinados el hueco de una etiqueta desplaza la altura.
    """

    changed = True
    while changed:
        changed = False
        tracks.sort(key=lambda track: track.xs[0])
        for i in range(len(tracks)):
            for j in range(i + 1, len(tracks)):
                left, right = tracks[i], tracks[j]
                if left.xs[0] >= right.xs[0]:
                    left, right = right, left
                gap = left.xs[-1] - right.xs[0]
                if 0 <= gap <= max_gap:
                    expected = left.ys[-1] - left.slope * gap
                    if abs(expected - right.ys[0]) <= max_offset:
                        left.xs.extend(right.xs)
                        left.ys.extend(right.ys)
                        tracks.pop(j)
                        changed = True
                        break
            if changed:
                break
    return tracks


def sample_track(track: Track, x: int, tolerance: int = 5) -> float | None:
    best: tuple[int, float] | None = None
    for tx, ty in zip(track.xs, track.ys):
        distance = abs(tx - x)
        if distance <= tolerance and (best is None or distance < best[0]):
            best = (distance, ty)
    return None if best is None else best[1]


def digitize_p05() -> list[dict[str, object]]:
    dark = load_page(P05)
    vertical, horizontal = refined_grid_lines(dark, P05)
    mask = grid_mask(dark.shape, P05, vertical, horizontal)
    curves = dark & ~mask
    y0, y1, x_left, x_right = P05.plot_box

    x_start = int(round(P05.x_px_at_unit0 + P05.px_per_x * 620.0))
    runs_source = lambda x: runs_of_mask(curves, x, y0, y1)  # noqa: E731
    tracks = track_columns(runs_source, P05, x_start, x_left + 20)
    print(f"p05: {len(tracks)} fragmentos antes de fusionar")
    tracks = merge_tracks(tracks)
    tracks = [track for track in tracks if (track.xs[0] - track.xs[-1]) >= 150]
    print(f"p05: {len(tracks)} pistas tras fusionar")

    vp_grid = [float(v) for v in range(60, 621, 20)]
    sampled: list[tuple[Track, dict[float, float]]] = []
    for track in tracks:
        values: dict[float, float] = {}
        for vp in vp_grid:
            x = int(round(P05.x_px_at_unit0 + P05.px_per_x * vp))
            y = sample_track(track, x)
            if y is None:
                continue
            ip_ma = (P05.y_px_at_unit0 - y) / P05.px_per_y
            if 2.0 <= ip_ma <= 470.0:
                values[vp] = ip_ma
        if len(values) >= 5:
            sampled.append((track, values))

    # anclaje: la pista que pasa por Ip ~ 250 mA en Vp = 300 es Ec2 = 300
    for _, values in sorted(sampled, key=lambda item: -item[1].get(300.0, -1.0)):
        marker = f"{values[300.0]:7.1f}" if 300.0 in values else "   --- "
        print(f"p05: pista Vp=300 -> {marker} mA  (n={len(values)})")
    candidates = [
        index
        for index, (_, values) in enumerate(sampled)
        if 300.0 in values and abs(values[300.0] - 250.0) < 15.0
    ]
    if len(candidates) != 1:
        raise SystemExit(f"p05: anclaje Ec2=300 ambiguo: {candidates}")
    anchor = candidates[0]
    sampled.sort(key=lambda item: -item[1].get(300.0, -1.0))
    anchor = next(
        index
        for index, (_, values) in enumerate(sampled)
        if 300.0 in values and abs(values[300.0] - 250.0) < 15.0
    )
    print(f"p05: ancla Ec2=300 en índice {anchor} de {len(sampled)}")
    if anchor != 3:
        print("p05: aviso: se esperaba índice 3 (450/400/350/300)")
    if len(sampled) < len(P05_CURVES_EC2):
        raise SystemExit(f"p05: solo {len(sampled)} pistas utilizables")

    lo = max(0, anchor - 3)
    hi = lo + len(P05_CURVES_EC2)
    if hi > len(sampled):
        hi = len(sampled)
        lo = hi - len(P05_CURVES_EC2)
    order = sampled[lo:hi]

    rows: list[dict[str, object]] = []
    for ec2, (_, values) in zip(P05_CURVES_EC2, order):
        for vp, ip_ma in sorted(values.items()):
            rows.append(
                {
                    "figure": "p05",
                    "vg1_v": 0.0,
                    "ec2_v": ec2,
                    "vp_v": vp,
                    "ip_ma": round(ip_ma, 1),
                    "ig2_ma": "",
                    "source": SOURCE,
                }
            )

    for ec2, (_, values) in zip(P05_CURVES_EC2, order):
        if ec2 == 300.0 and 300.0 in values:
            print(f"p05: Ip(Ec2=300, Vp=300, Vg1=0) ~ {values[300.0]:.1f} mA (p07: 250)")

    overlay = np.stack([255 - dark.astype(np.uint8) * 255] * 3, axis=-1)
    for _, values in order:
        for vp, ip_ma in values.items():
            x = int(round(P05.x_px_at_unit0 + P05.px_per_x * vp))
            y = int(round(P05.y_px_at_unit0 - P05.px_per_y * ip_ma))
            overlay[max(0, y - 3) : y + 4, x - 3 : x + 4] = (255, 0, 0)
    Image.fromarray(overlay).save(EXTRACTED / "p05_digitized_overlay.png")
    return rows


def detailed_runs(dark: np.ndarray, x: int, y0: int, y1: int) -> list[tuple[float, int]]:
    column = dark[y0:y1, x]
    runs: list[tuple[float, int]] = []
    y = 0
    n = len(column)
    while y < n:
        if column[y]:
            start = y
            while y < n and column[y]:
                y += 1
            runs.append((0.5 * (start + y) + y0, y - start))
        else:
            y += 1
    return runs


# Anclas de los dos trazos de p07 (x, y), leídas del escaneo con la rejilla
# visible; el muestreo busca el trazo real a ±12 px de la interpolación lineal
# entre anclas. La Ig2 solo se muestrea hasta Vg1 ≈ −12.5: más abajo el trazo
# se confunde con la cola de la curva de placa (documentado).
P07_IP_ANCHORS = (
    (1004, 940.0),
    (950, 1054.0),
    (850, 1249.0),
    (753, 1385.0),
    (650, 1557.0),
    (610, 1605.0),
    (500, 1698.0),
    (400, 1759.0),
    (300, 1801.0),
    (250, 1808.0),
)
P07_IG2_ANCHORS = (
    (1004, 1540.0),
    (950, 1593.0),
    (850, 1673.0),
    (753, 1718.0),
    (650, 1803.0),
)


def guided_sample(
    dark: np.ndarray,
    spec: PageCalibration,
    x: int,
    expected_y: float,
    tolerance: float = 12.0,
) -> float | None:
    """Centro de trazo más cercano a `expected_y` en la columna `x`."""

    y0, y1, _, _ = spec.plot_box
    best: tuple[float, float] | None = None
    for center, length in detailed_runs(dark, x, y0, y1):
        if length < 4:
            continue
        distance = abs(center - expected_y)
        if distance <= tolerance and (best is None or distance < best[0]):
            best = (distance, center)
    return None if best is None else best[1]


def interpolate_anchors(anchors: tuple[tuple[int, float], ...], x: int) -> float:
    ordered = sorted(anchors)
    if x <= ordered[0][0]:
        return ordered[0][1]
    if x >= ordered[-1][0]:
        return ordered[-1][1]
    for index in range(len(ordered) - 1):
        x0, y0 = ordered[index]
        x1, y1 = ordered[index + 1]
        if x0 <= x <= x1:
            fraction = (x - x0) / (x1 - x0)
            return y0 + fraction * (y1 - y0)
    return ordered[-1][1]


def digitize_p07() -> list[dict[str, object]]:
    dark = load_page(P07)
    rows: list[dict[str, object]] = []
    ip_count = 0
    ig2_count = 0
    for vg1_step in range(0, 39):
        vg1 = round(0.5 - 1.0 * vg1_step, 1)
        x = int(round(P07.x_px_at_unit0 + P07.px_per_x * vg1))
        y = guided_sample(dark, P07, x, interpolate_anchors(P07_IP_ANCHORS, x))
        if y is not None:
            ip_ma = (P07.y_px_at_unit0 - y) / P07.px_per_y
            if 0.0 < ip_ma < 280.0:
                ip_count += 1
                rows.append(
                    {
                        "figure": "p07",
                        "vg1_v": vg1,
                        "ec2_v": 300.0,
                        "vp_v": 300.0,
                        "ip_ma": round(ip_ma, 1),
                        "ig2_ma": "",
                        "source": SOURCE,
                    }
                )
        if vg1 < -12.5:
            continue
        y = guided_sample(dark, P07, x, interpolate_anchors(P07_IG2_ANCHORS, x))
        if y is not None:
            ig2_ma = (P07_IG2_PX_AT_ZERO - y) / P07_IG2_PX_PER_MA
            if 0.0 < ig2_ma < 30.0:
                ig2_count += 1
                rows.append(
                    {
                        "figure": "p07",
                        "vg1_v": vg1,
                        "ec2_v": 300.0,
                        "vp_v": 300.0,
                        "ip_ma": "",
                        "ig2_ma": round(ig2_ma, 2),
                        "source": SOURCE,
                    }
                )
    print(f"p07: Ip {ip_count} muestras, Ig2 {ig2_count} muestras")

    overlay = np.stack([255 - dark.astype(np.uint8) * 255] * 3, axis=-1)
    for row in rows:
        x = int(round(P07.x_px_at_unit0 + P07.px_per_x * float(row["vg1_v"])))
        if row["ip_ma"] != "":
            y = int(round(P07.y_px_at_unit0 - P07.px_per_y * float(row["ip_ma"])))
            color = (255, 0, 0)
        else:
            y = int(round(P07_IG2_PX_AT_ZERO - P07_IG2_PX_PER_MA * float(row["ig2_ma"])))
            color = (0, 0, 255)
        overlay[max(0, y - 3) : y + 4, max(0, x - 3) : x + 4] = color
    Image.fromarray(overlay).save(EXTRACTED / "p07_digitized_overlay.png")
    return rows


# Curva de Ig2 sin etiqueta legible de p06 (Ec2 = 300 V), leída con anclas
# guiadas; aporta la forma Ig2(Vp) (subida en rodilla). El nivel absoluto y la
# identificación de Vg1 no se usan: `vg1_v` queda vacío en el CSV.
P06_IG2_ANCHORS = (
    (1000, 911.5),
    (900, 912.0),
    (800, 912.0),
    (700, 910.5),
    (600, 908.0),
    (500, 900.0),
    (400, 892.0),
    (350, 890.5),
    (300, 897.0),
    (250, 897.5),
    (200, 897.5),
    (170, 898.0),
)


def digitize_p06() -> list[dict[str, object]]:
    dark = load_page(P06)
    rows: list[dict[str, object]] = []
    for x in range(170, 1001, 10):
        expected = interpolate_anchors(P06_IG2_ANCHORS, x)
        y = guided_sample(dark, P06, x, expected)
        if y is None:
            continue
        ig2_ma = (P06.y_px_at_unit0 - y) / P06.px_per_y
        vp = (x - P06.x_px_at_unit0) / P06.px_per_x
        if 1.0 < ig2_ma < 40.0:
            rows.append(
                {
                    "figure": "p06",
                    "vg1_v": "",
                    "ec2_v": 300.0,
                    "vp_v": round(vp / 5.0) * 5.0,
                    "ip_ma": "",
                    "ig2_ma": round(ig2_ma, 2),
                    "source": SOURCE
                    + " (curva Ig2 sin etiqueta; solo forma vs Vp)",
                }
            )
    print(f"p06: Ig2 {len(rows)} muestras de forma (sin Vg1)")
    return rows


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, default=DEFAULT_OUT)
    args = parser.parse_args()

    if not EXTRACTED.exists():
        print("falta extracted/: ejecuta primero scripts/extract_pdfs.py")
        return 1

    rows = digitize_p05() + digitize_p07() + digitize_p06()
    args.out.parent.mkdir(parents=True, exist_ok=True)
    with args.out.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(
            handle,
            fieldnames=[
                "figure",
                "vg1_v",
                "ec2_v",
                "vp_v",
                "ip_ma",
                "ig2_ma",
                "source",
            ],
        )
        writer.writeheader()
        writer.writerows(rows)
    print(f"escrito {args.out} con {len(rows)} puntos")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
