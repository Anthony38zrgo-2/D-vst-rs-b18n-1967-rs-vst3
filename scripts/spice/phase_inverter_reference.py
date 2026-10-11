"""Genera el golden AC del inversor 7199 desde ngspice.

Ejecuta `scripts/spice/b18n_7199_ac_1967.cir` (excitacion AC unitaria en la
rejilla del pentodo; salidas tras el acoplo de 0.05 uF con 100 kOhm) y escribe
`assets/fixtures/reference/phase_7199/b18n_7199_ac_1967_golden.csv` con:

    freq_hz,mag_up,phase_up_deg,mag_down,phase_down_deg

Uso:
    python scripts/spice/phase_inverter_reference.py
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
NETLIST = REPO_ROOT / "scripts" / "spice" / "b18n_7199_ac_1967.cir"
GOLDEN = (
    REPO_ROOT
    / "assets"
    / "fixtures"
    / "reference"
    / "phase_7199"
    / "b18n_7199_ac_1967_golden.csv"
)

MAG_RE = re.compile(r"mag\((hu|hd)\)\s*=\s*([-+0-9.eE]+)")
PH_RE = re.compile(r"ph\((hu|hd)\)\s*=\s*([-+0-9.eE]+)")


def main() -> int:
    binary = find_ngspice()
    log = NETLIST.with_suffix(NETLIST.suffix + ".log")
    completed = subprocess.run(
        [str(binary), "-b", "-o", str(log), NETLIST.name],
        cwd=str(NETLIST.parent),
        check=False,
        capture_output=True,
        text=True,
    )
    if completed.returncode != 0:
        raise SystemExit(f"ngspice fallo: {completed.stderr.strip()}")

    text = log.read_text(encoding="utf-8", errors="replace")
    magnitudes = MAG_RE.findall(text)
    phases = PH_RE.findall(text)
    if len(magnitudes) != 20 or len(phases) != 20:
        raise SystemExit(
            f"se esperaban 20 valores, se leyeron {len(magnitudes)}/{len(phases)}"
        )

    frequencies = [20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0, 20000.0]
    rows = []
    for index, frequency in enumerate(frequencies):
        mag_up = float(magnitudes[index * 2][1])
        mag_dn = float(magnitudes[index * 2 + 1][1])
        phase_up = float(phases[index * 2][1])
        phase_dn = float(phases[index * 2 + 1][1])
        rows.append(
            (
                frequency,
                mag_up,
                math.degrees(phase_up),
                mag_dn,
                math.degrees(phase_dn),
            )
        )

    GOLDEN.parent.mkdir(parents=True, exist_ok=True)
    with GOLDEN.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.writer(handle)
        writer.writerow(["freq_hz", "mag_up", "phase_up_deg", "mag_down", "phase_down_deg"])
        for row in rows:
            writer.writerow(
                [
                    f"{row[0]:.1f}",
                    f"{row[1]:.6f}",
                    f"{row[2]:.4f}",
                    f"{row[3]:.6f}",
                    f"{row[4]:.4f}",
                ]
            )
    print(f"escrito {GOLDEN} ({len(rows)} filas)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
