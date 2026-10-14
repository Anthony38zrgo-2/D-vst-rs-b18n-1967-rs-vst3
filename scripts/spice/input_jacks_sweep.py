"""Genera el barrido SPICE de las redes de jack de entrada (B18-007).

Replica las tres redes trazadas del raster SCH-567 (bass, guitar, instrument)
con dos impedancias de fuente (0 Ω ideal y 10 kΩ de instrumento pasivo como
condición de ensayo) y escribe un CSV:

    jack,source_ohms,freq_hz,mag_db,phase_deg

donde `H = v(grid)/v(src)`. Las redes son resistivas: la respuesta es plana en
frecuencia y el golden comprueba los divisores contra ngspice 47.

Uso:
    python scripts/spice/input_jacks_sweep.py [salida.csv]

Por defecto: assets/fixtures/reference/input_jacks/input_jacks_spice.csv.
"""

from __future__ import annotations

import csv
import math
import re
import subprocess
import sys
from pathlib import Path

from run_spice import find_ngspice

REPO_ROOT = Path(__file__).resolve().parents[2]
WORK_DIR = REPO_ROOT / "target" / "reports" / "spice_input_jacks"
DEFAULT_OUTPUT = (
    REPO_ROOT / "assets" / "fixtures" / "reference" / "input_jacks" / "input_jacks_spice.csv"
)

# Trazado SCH-567 (B18-007).
R_SERIES_CH1 = 120_000.0
R_LEAK_CH1 = 5_600_000.0
R_SHUNT_FREE_JACK = 120_000.0 + 100_000.0  # llave del jack libre
R_LEAK_CH2 = 2_200_000.0

SOURCE_IMPEDANCES = [0.0, 10_000.0]
FREQUENCIES_HZ = [
    20.0, 25.0, 31.5, 40.0, 50.0, 63.0, 80.0, 100.0, 125.0, 160.0, 200.0,
    250.0, 315.0, 400.0, 500.0, 630.0, 800.0, 1000.0, 1250.0, 1600.0,
    2000.0, 2500.0, 3150.0, 4000.0, 5000.0, 6300.0, 8000.0, 10000.0,
    12500.0, 16000.0, 20000.0,
]

MAG_RE = re.compile(r"mag\(h\)\s*=\s*([-+0-9.eE]+)")
PH_RE = re.compile(r"ph\(h\)\s*=\s*([-+0-9.eE]+)")


def networks() -> dict[str, tuple[float, float]]:
    """(serie, carga de rejilla) por jack con el jack en uso."""
    bass_leak = (R_LEAK_CH1 * R_SHUNT_FREE_JACK) / (R_LEAK_CH1 + R_SHUNT_FREE_JACK)
    return {
        "bass": (R_SERIES_CH1, bass_leak),
        "guitar": (R_SERIES_CH1, R_LEAK_CH1),
        "instrument": (1e-3, R_LEAK_CH2),
    }


def netlist(name: str, series: float, leak: float, source_ohms: float) -> str:
    lines = [
        f"* input jack {name} - Rsrc={source_ohms:.0f}",
        "* generado por scripts/spice/input_jacks_sweep.py (no editar a mano)",
        "vin src 0 ac 1",
        f"rsrc src tip {source_ohms:.6f}",
        f"rseries tip grid {series:.6f}",
        f"rleak grid 0 {leak:.6f}",
        ".control",
        "set numdgt=10",
    ]
    for frequency in FREQUENCIES_HZ:
        lines.append(f"ac lin 1 {frequency:.10g} {frequency:.10g}")
        lines.append("let h = v(grid)/v(src)")
        lines.append("print mag(h) ph(h)")
    lines.extend([".endc", ".end", ""])
    return "\n".join(lines)


def run_case(binary: Path, name: str, series: float, leak: float, source_ohms: float):
    stem = f"jack_{name}_rs{source_ohms:.0f}"
    netlist_path = WORK_DIR / f"{stem}.cir"
    log_path = WORK_DIR / f"{stem}.log"
    netlist_path.write_text(netlist(name, series, leak, source_ohms), encoding="utf-8")
    completed = subprocess.run(
        [str(binary), "-b", "-o", str(log_path), str(netlist_path.name)],
        cwd=str(WORK_DIR),
        check=False,
        capture_output=True,
        text=True,
    )
    if completed.returncode != 0:
        raise SystemExit(f"ngspice falló en {stem}: {completed.stderr.strip()}")
    text = log_path.read_text(encoding="utf-8", errors="replace")
    magnitudes = [float(value) for value in MAG_RE.findall(text)]
    phases_rad = [float(value) for value in PH_RE.findall(text)]
    if len(magnitudes) != len(FREQUENCIES_HZ) or len(phases_rad) != len(FREQUENCIES_HZ):
        raise SystemExit(
            f"{stem}: se esperaban {len(FREQUENCIES_HZ)} puntos, "
            f"se leyeron {len(magnitudes)} magnitudes y {len(phases_rad)} fases"
        )
    return list(zip(magnitudes, phases_rad))


def main(argv: list[str]) -> int:
    destination = Path(argv[1]) if len(argv) > 1 else DEFAULT_OUTPUT
    destination.parent.mkdir(parents=True, exist_ok=True)
    WORK_DIR.mkdir(parents=True, exist_ok=True)
    binary = find_ngspice()

    rows = 0
    with destination.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.writer(handle)
        writer.writerow(["jack", "source_ohms", "freq_hz", "mag_db", "phase_deg"])
        for name, (series, leak) in networks().items():
            for source_ohms in SOURCE_IMPEDANCES:
                values = run_case(binary, name, series, leak, source_ohms)
                for frequency, (magnitude, phase_rad) in zip(FREQUENCIES_HZ, values):
                    writer.writerow(
                        [
                            name,
                            f"{source_ohms:.0f}",
                            f"{frequency:.1f}",
                            f"{20.0 * math.log10(magnitude):.9f}",
                            f"{math.degrees(phase_rad):.6f}",
                        ]
                    )
                    rows += 1

    print(f"escrito {destination} ({rows} filas, ngspice {binary})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
