"""Referencia SPICE del OT-471 lineal (B18-013): contraste AC del motor.

Ejecuta `b18n_ot471_linear.cir` (.ac de 5 Hz a 200 kHz) y escribe

    assets/fixtures/reference/output_transformer/ot471_linear_spice.csv

con la respuesta compleja `vsec/vs` (dB y grados) de la red diferencial del
transformador con los parametros estimados del overlay:

    Lm = 28.7 H (corte LF 25 Hz), Lleak = 12 mH (corte HF ~64 kHz),
    Cp = 100 pF (resonancia ~145 kHz), R_core = 500 ohm (perdidas de flujo),
    Rs = 45 kohm (2 * rp de la 7027A del datasheet RCA a clase A1),
    N8 = 23.18, DCR 300 / 0.4 ohm, carga 8 ohm.

El script calcula ademas la respuesta analitica con numpy (implementacion
independiente de la red) y comprueba que ngspice coincide antes de escribir el
golden; si no coincide (topologia o signos), falla en vez de escribir.

Uso:
    python scripts/spice/output_transformer_reference.py

Requiere ngspice 47 (véase scripts/spice/README.md).
"""

from __future__ import annotations

import cmath
import subprocess
import sys
from pathlib import Path

from run_spice import find_ngspice

REPO_ROOT = Path(__file__).resolve().parents[2]
SPICE_DIR = REPO_ROOT / "scripts" / "spice"
OUT_DIR = REPO_ROOT / "assets" / "fixtures" / "reference" / "output_transformer"
WORK_DIR = REPO_ROOT / "target" / "reports" / "spice_ot471"

NETLIST = SPICE_DIR / "b18n_ot471_linear.cir"
GOLDEN = OUT_DIR / "ot471_linear_spice.csv"

TURNS_RATIO = 23.18
LOAD_OHMS = 8.0
DC_SECONDARY_OHMS = 0.4
DC_PRIMARY_OHMS = 300.0
MAGNETIZING_H = 28.7
LEAKAGE_H = 0.012
PARASITIC_F = 100e-12
CORE_LOSS_OHMS = 500.0
SOURCE_OHMS = 45_000.0


def analytic_transfer(frequency_hz: float) -> complex:
    """Respuesta compleja vsec/vs de la red continua (convencion del motor).

    Topologia: vs -- Rs -- Rdc(2 medias) -- nodo A; en A: Cp placa-placa;
    A -- R_core -- nodo F (perdidas de flujo); en F: Lm y (via ideal)
    Lleak + N^2*(Rcarga + DCR_sec); el secundario se acopla a v_F.
    """
    omega = 2.0 * cmath.pi * frequency_hz
    reflected = TURNS_RATIO * TURNS_RATIO * (LOAD_OHMS + DC_SECONDARY_OHMS)
    z_magnetizing = 1j * omega * MAGNETIZING_H
    z_load = reflected + 1j * omega * LEAKAGE_H
    z_cp = 1.0 / (1j * omega * PARASITIC_F)
    z_node_f = 1.0 / (1.0 / z_magnetizing + 1.0 / z_load)
    z_internal = CORE_LOSS_OHMS + z_node_f
    z_total = 1.0 / (1.0 / z_internal + 1.0 / z_cp)
    i_port = 1.0 / (SOURCE_OHMS + DC_PRIMARY_OHMS + z_total)
    u_a = z_total * i_port
    v_f = u_a * z_node_f / z_internal
    i_ref = v_f / z_load
    # El motor: vsec = -N * i_ref * Rcarga (id positivo -> secundario negativo).
    return -TURNS_RATIO * i_ref * LOAD_OHMS


def run_ngspice() -> list[tuple[float, float, float]]:
    WORK_DIR.mkdir(parents=True, exist_ok=True)
    binary = find_ngspice()
    log = WORK_DIR / "ot471.log"
    completed = subprocess.run(
        [str(binary), "-b", "-o", str(log), str(NETLIST)],
        cwd=str(WORK_DIR),
        check=False,
        capture_output=True,
        text=True,
    )
    out = log.read_text(encoding="utf-8", errors="replace") if log.is_file() else ""
    if completed.returncode != 0:
        print(out[-4000:])
        raise SystemExit(f"ngspice fallo ({completed.returncode})")
    data_file = WORK_DIR / "ot471_ac.txt"
    if not data_file.is_file():
        print(out[-4000:])
        raise SystemExit("sin ot471_ac.txt")
    rows: list[tuple[float, float, float]] = []
    for line in data_file.read_text(encoding="utf-8", errors="replace").splitlines():
        parts = line.split()
        # wrdata con dos expresiones: freq, vm, freq, vp (4 columnas).
        # ngspice escribe la fase de `vp()` en RADIANES.
        if len(parts) < 4:
            continue
        try:
            frequency = float(parts[0])
            magnitude = float(parts[1])
            phase_rad = float(parts[3])
        except ValueError:
            continue
        rows.append((frequency, magnitude, phase_rad))
    if len(rows) < 50:
        raise SystemExit(f"barrido incompleto ({len(rows)} filas)")
    return rows


def main() -> int:
    rows = run_ngspice()
    worst_deviation = 0.0
    for frequency, magnitude, phase_rad in rows:
        spice = cmath.rect(magnitude, phase_rad)
        expected = analytic_transfer(frequency)
        deviation = abs(spice - expected) / max(abs(expected), 1e-12)
        worst_deviation = max(worst_deviation, deviation)
    print(f"ngspice vs analitico: desviacion relativa maxima {worst_deviation:.2e}")
    if worst_deviation > 1e-3:
        raise SystemExit(
            "el netlist no coincide con la red analitica; no se escribe el golden"
        )

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    with GOLDEN.open("w", encoding="utf-8", newline="") as handle:
        handle.write(
            "# B-18N 1967 - OT-471 lineal (B18-013): equivalente diferencial\n"
            "# vsec/vs con Rs=45 kohm; Lm=28.7 H, Lleak=12 mH, Cp=100 pF, R_core=500 ohm,\n"
            "# N8=23.18, DCR 300/0.4 ohm, carga 8 ohm (parametros estimated del overlay).\n"
            "# ngspice 47 sobre scripts/spice/b18n_ot471_linear.cir (AC, dec 20).\n"
            "frequency_hz,vsec_over_vs_db,vsec_over_vs_phase_deg\n"
        )
        for frequency, magnitude, phase_rad in rows:
            db = 20.0 * cmath.log10(magnitude).real if magnitude > 0 else -999.0
            phase_deg = phase_rad * 180.0 / cmath.pi
            handle.write(f"{frequency:.9g},{db:.9f},{phase_deg:.9f}\n")
    print(f"{len(rows)} filas -> {GOLDEN}")

    for probe in (20.0, 200.0, 1_000.0, 20_000.0):
        nearest = min(rows, key=lambda row: abs(row[0] - probe))
        print(
            f"{nearest[0]:10.2f} Hz: {20 * cmath.log10(nearest[1]).real:+7.2f} dB, "
            f"{nearest[2] * 180.0 / cmath.pi:+8.2f} deg"
        )
    return 0


if __name__ == "__main__":
    sys.exit(main())
