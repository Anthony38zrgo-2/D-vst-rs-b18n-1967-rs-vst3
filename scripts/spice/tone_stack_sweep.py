"""Genera el barrido SPICE de la red de tono del B-18N 1967 y lo compara.

Replica exactamente el netlist y las condiciones de `tone_stack_reference.py`
(misma fuente Norton de 1 A, Rfuente 100 kΩ, Rcarga 270 kΩ, mismas divisiones
de potenciómetro con recorte 1e-6) pero resuelve con ngspice 47 en vez de
numpy. Escribe un CSV con el mismo esquema:

    bass,treble,freq_hz,mag_db,phase_deg

Uso:
    python scripts/spice/tone_stack_sweep.py [salida.csv]

Por defecto: assets/fixtures/reference/tone_stack/tone_stack_spice.csv.
Los netlists generados y sus logs quedan en target/reports/spice_tone/.
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
WORK_DIR = REPO_ROOT / "target" / "reports" / "spice_tone"
DEFAULT_OUTPUT = REPO_ROOT / "assets" / "fixtures" / "reference" / "tone_stack" / "tone_stack_spice.csv"

# Valores del JSON histórico (idénticos a tone_stack_reference.py).
R_SERIES = 220_000.0
R_BASS_STOP = 22_000.0
R_BASS_CUT = 270_000.0
R_MID = 120_000.0
C_BASS_TOP = 1e-9
C_BASS_BOTTOM = 1e-8
C_TREBLE_SERIES = 4.7e-10
C_TREBLE_SHUNT = 4.7e-9
POT_BASS = 1_000_000.0
POT_TREBLE = 1_000_000.0

SOURCE_OHMS = 100_000.0
LOAD_OHMS = 270_000.0

POSITIONS = [0.0, 0.25, 0.5, 0.75, 1.0]

FREQUENCIES_HZ = [
    20.0, 25.0, 31.5, 40.0, 50.0, 63.0, 80.0, 100.0, 125.0, 160.0, 200.0,
    250.0, 315.0, 400.0, 500.0, 630.0, 800.0, 1000.0, 1250.0, 1600.0,
    2000.0, 2500.0, 3150.0, 4000.0, 5000.0, 6300.0, 8000.0, 10000.0,
    12500.0, 16000.0, 20000.0,
]

MAG_RE = re.compile(r"mag\(h\)\s*=\s*([-+0-9.eE]+)")
PH_RE = re.compile(r"ph\(h\)\s*=\s*([-+0-9.eE]+)")


def pot_split(position: float) -> tuple[float, float]:
    """Misma división que Rust/numpy: cursor 1.0 en el extremo superior."""
    p = min(max(position, 1e-6), 1.0 - 1e-6)
    return (1.0 - p) * POT_BASS, p * POT_BASS


def netlist(bass: float, treble: float) -> str:
    r_bw, r_wc = pot_split(bass)
    r_t_out, r_out_t2 = pot_split(treble)
    lines = [
        f"* tone stack B-18N 1967 - bass={bass:.2f} treble={treble:.2f}",
        "* generado por scripts/spice/tone_stack_sweep.py (no editar a mano)",
        "iin 0 in ac 1",
        f"rsource in 0 {SOURCE_OHMS:.6f}",
        f"rseries in b {R_SERIES:.6f}",
        f"ct1 in t1 {C_TREBLE_SERIES:.6e}",
        f"cb_top b w {C_BASS_TOP:.6e}",
        f"cb_bot c w {C_BASS_BOTTOM:.6e}",
        f"rbw b w {r_bw:.6f}",
        f"rwc w c {r_wc:.6f}",
        f"rstop c 0 {R_BASS_STOP:.6f}",
        f"rcut w 0 {R_BASS_CUT:.6f}",
        f"rmid w t1 {R_MID:.6f}",
        f"rt_out t1 out {r_t_out:.6f}",
        f"rout_t2 out t2 {r_out_t2:.6f}",
        f"ct2 t2 0 {C_TREBLE_SHUNT:.6e}",
        f"rload out 0 {LOAD_OHMS:.6f}",
        ".control",
        "set numdgt=10",
    ]
    for frequency in FREQUENCIES_HZ:
        lines.append(f"ac lin 1 {frequency:.10g} {frequency:.10g}")
        lines.append("let h = v(out)/v(in)")
        lines.append("print mag(h) ph(h)")
    lines.extend([".endc", ".end", ""])
    return "\n".join(lines)


def run_combination(binary: Path, bass: float, treble: float) -> list[tuple[float, float]]:
    stem = f"tone_b{bass:.2f}_t{treble:.2f}"
    netlist_path = WORK_DIR / f"{stem}.cir"
    log_path = WORK_DIR / f"{stem}.log"
    netlist_path.write_text(netlist(bass, treble), encoding="utf-8")
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
        writer.writerow(["bass", "treble", "freq_hz", "mag_db", "phase_deg"])
        for bass in POSITIONS:
            for treble in POSITIONS:
                values = run_combination(binary, bass, treble)
                for frequency, (magnitude, phase_rad) in zip(FREQUENCIES_HZ, values):
                    writer.writerow(
                        [
                            f"{bass:.2f}",
                            f"{treble:.2f}",
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
