"""Genera el barrido SPICE de la red de volumen del canal (B18-005).

Replica la red trazada: placa del 6SL7#1 (`Vsrc` detrás de `Ro`) → capacitor
de acoplo de 0.022 µF → potenciómetro de 1 MΩ log (Rtop–cursor–Rbottom a masa)
→ rejilla del 6SL7#2 (circuito abierto). El barrido cubre las posiciones del
mando con la ley documentada `f(p) = (10^(2p) − 1)/99` y varias frecuencias,
y escribe:

    position,fraction,ro_ohms,coupling_farads,pot_ohms,freq_hz,mag_db,phase_deg

Uso:
    python scripts/spice/volume_network_sweep.py [salida.csv]

Por defecto: assets/fixtures/reference/volume_network/volume_network_spice.csv.
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
WORK_DIR = REPO_ROOT / "target" / "reports" / "spice_volume_network"
DEFAULT_OUTPUT = (
    REPO_ROOT
    / "assets"
    / "fixtures"
    / "reference"
    / "volume_network"
    / "volume_network_spice.csv"
)

RO_OHMS = 227_700.0
COUPLING_FARADS = 2.2e-8
POT_OHMS = 1_000_000.0
POSITIONS = [0.25, 0.5, 0.75, 1.0]
FREQUENCIES_HZ = [100.0, 1_000.0, 10_000.0]

MAG_RE = re.compile(r"mag\(h\)\s*=\s*([-+0-9.eE]+)")
PH_RE = re.compile(r"ph\(h\)\s*=\s*([-+0-9.eE]+)")


def volume_fraction(position: float) -> float:
    """Ley documentada del potenciómetro de volumen (fracción de resistencia)."""
    return (10.0 ** (2.0 * position) - 1.0) / 99.0


def netlist(fraction: float, frequency: float) -> str:
    # ngspice no admite resistencias de 0 Ω (estampado singular): se usa un
    # piso de 1 mΩ, despreciable frente a los 227.7 kΩ de `Ro`.
    r_bottom = max(fraction * POT_OHMS, 1e-3)
    r_top = max((1.0 - fraction) * POT_OHMS, 1e-3)
    return "\n".join(
        [
            f"* volume network - f={fraction:.6f} f0={frequency:.0f}",
            "* generado por scripts/spice/volume_network_sweep.py (no editar a mano)",
            "vin src 0 ac 1",
            f"ro src s {RO_OHMS:.6f}",
            f"cc s t {COUPLING_FARADS:.6e}",
            f"rtop t w {r_top:.6f}",
            f"rbot w 0 {r_bottom:.6f}",
            ".control",
            "set numdgt=10",
            f"ac lin 1 {frequency:.10g} {frequency:.10g}",
            "let h = v(w)/v(src)",
            "print mag(h) ph(h)",
            ".endc",
            ".end",
            "",
        ]
    )


def run_case(binary: Path, position: float, frequency: float):
    fraction = volume_fraction(position)
    stem = f"vol_p{position:.2f}_f{frequency:.0f}"
    netlist_path = WORK_DIR / f"{stem}.cir"
    log_path = WORK_DIR / f"{stem}.log"
    netlist_path.write_text(netlist(fraction, frequency), encoding="utf-8")
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
    magnitude = float(MAG_RE.findall(text)[0])
    phase_rad = float(PH_RE.findall(text)[0])
    return magnitude, phase_rad


def main(argv: list[str]) -> int:
    destination = Path(argv[1]) if len(argv) > 1 else DEFAULT_OUTPUT
    destination.parent.mkdir(parents=True, exist_ok=True)
    WORK_DIR.mkdir(parents=True, exist_ok=True)
    binary = find_ngspice()

    rows = 0
    with destination.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.writer(handle)
        writer.writerow(
            [
                "position",
                "fraction",
                "ro_ohms",
                "coupling_farads",
                "pot_ohms",
                "freq_hz",
                "mag_db",
                "phase_deg",
            ]
        )
        for position in POSITIONS:
            fraction = volume_fraction(position)
            for frequency in FREQUENCIES_HZ:
                magnitude, phase_rad = run_case(binary, position, frequency)
                writer.writerow(
                    [
                        f"{position:.2f}",
                        f"{fraction:.9f}",
                        f"{RO_OHMS:.0f}",
                        f"{COUPLING_FARADS:.6e}",
                        f"{POT_OHMS:.0f}",
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
