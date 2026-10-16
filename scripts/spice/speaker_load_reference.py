"""Referencia SPICE del OT-471 con carga reactiva del altavoz (B18-015).

Ejecuta `b18n_speaker_load.cir` (.ac de 5 Hz a 200 kHz) y escribe

    assets/fixtures/reference/speaker_load/speaker_load_spice.csv

con la respuesta compleja `vload/vs` (dB y grados) del terminal del altavoz:
red diferencial del OT (parametros estimated de B18-013) con la carga RLC
estimada del altavoz (overlay `speaker.reactive_load.*`, NO medida):

    Re = 6.5 ohm, Le = 0.8 mH, Res = 23.5 ohm, Les = 69.3 mH, Ces = 180.6 uF
    (fs = 45 Hz, Q = 1.2, pico |Z| = 30 ohm)

El script calcula ademas la respuesta analitica con numpy (implementacion
independiente de la red) y comprueba que ngspice coincide antes de escribir el
golden; si no coincide (topologia o signos), falla en vez de escribir.

Uso:
    python scripts/spice/speaker_load_reference.py

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
OUT_DIR = REPO_ROOT / "assets" / "fixtures" / "reference" / "speaker_load"
WORK_DIR = REPO_ROOT / "target" / "reports" / "spice_speaker_load"

NETLIST = SPICE_DIR / "b18n_speaker_load.cir"
GOLDEN = OUT_DIR / "speaker_load_spice.csv"

TURNS_RATIO = 23.18
DC_SECONDARY_OHMS = 0.4
DC_PRIMARY_OHMS = 300.0
MAGNETIZING_H = 28.7
LEAKAGE_H = 0.012
PARASITIC_F = 100e-12
CORE_LOSS_OHMS = 500.0
SOURCE_OHMS = 45_000.0

LOAD_RE_OHMS = 6.5
LOAD_LE_H = 0.0008
LOAD_RES_OHMS = 23.5
LOAD_LES_H = 0.0693
LOAD_CES_F = 180.6e-6


def load_impedance(frequency_hz: float) -> complex:
    """Z(f) del equivalente estimado: Re + jwLe + (Res || jwLes || 1/jwCes)."""
    omega = 2.0 * cmath.pi * frequency_hz
    motional = 1.0 / (
        1.0 / LOAD_RES_OHMS
        + 1.0 / (1j * omega * LOAD_LES_H)
        + 1j * omega * LOAD_CES_F
    )
    return LOAD_RE_OHMS + 1j * omega * LOAD_LE_H + motional


def analytic_transfer(frequency_hz: float) -> complex:
    """Respuesta compleja vload/vs de la red continua (convencion del motor).

    Topologia: vs -- Rs -- Rdc(2 medias) -- nodo A; en A: Cp placa-placa;
    A -- R_core -- nodo F; en F: Lm y Lleak + N^2*(Zcarga + DCR_sec) por el
    ideal; el secundario se acopla al EMF del flujo y vload = -N*i_ref*Zcarga.
    """
    omega = 2.0 * cmath.pi * frequency_hz
    z_load = load_impedance(frequency_hz)
    reflected = TURNS_RATIO * TURNS_RATIO * (DC_SECONDARY_OHMS + z_load)
    z_magnetizing = 1j * omega * MAGNETIZING_H
    z_branch = reflected + 1j * omega * LEAKAGE_H
    z_cp = 1.0 / (1j * omega * PARASITIC_F)
    z_node_f = 1.0 / (1.0 / z_magnetizing + 1.0 / z_branch)
    z_internal = CORE_LOSS_OHMS + z_node_f
    z_total = 1.0 / (1.0 / z_internal + 1.0 / z_cp)
    i_port = 1.0 / (SOURCE_OHMS + DC_PRIMARY_OHMS + z_total)
    u_a = z_total * i_port
    v_f = u_a * z_node_f / z_internal
    i_ref = v_f / z_branch
    return -TURNS_RATIO * i_ref * z_load


def run_ngspice() -> list[tuple[float, float, float]]:
    WORK_DIR.mkdir(parents=True, exist_ok=True)
    binary = find_ngspice()
    log = WORK_DIR / "speaker_load.log"
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
    data_file = WORK_DIR / "b18n_speaker_load_ac.txt"
    if not data_file.is_file():
        print(out[-4000:])
        raise SystemExit("sin b18n_speaker_load_ac.txt")
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
            "# B-18N 1967 - OT-471 + carga RLC estimada del altavoz 18\" (B18-015)\n"
            "# vload/vs con Rs=45 kohm; OT: Lm=28.7 H, Lleak=12 mH, Cp=100 pF, R_core=500 ohm,\n"
            "# DCR 300/0.4 ohm, N8=23.18; carga (estimated): Re=6.5 ohm, Le=0.8 mH,\n"
            "# Res=23.5 ohm, Les=69.3 mH, Ces=180.6 uF (fs=45 Hz, Q=1.2, pico |Z|=30 ohm).\n"
            "# ngspice 47 sobre scripts/spice/b18n_speaker_load.cir (AC, dec 20).\n"
            "frequency_hz,vload_over_vs_db,vload_over_vs_phase_deg\n"
        )
        for frequency, magnitude, phase_rad in rows:
            db = 20.0 * cmath.log10(magnitude).real if magnitude > 0 else -999.0
            phase_deg = phase_rad * 180.0 / cmath.pi
            handle.write(f"{frequency:.9g},{db:.9f},{phase_deg:.9f}\n")
    print(f"{len(rows)} filas -> {GOLDEN}")

    for probe in (20.0, 45.0, 200.0, 1_000.0, 20_000.0):
        nearest = min(rows, key=lambda row: abs(row[0] - probe))
        impedance = load_impedance(nearest[0])
        print(
            f"{nearest[0]:10.2f} Hz: {20 * cmath.log10(nearest[1]).real:+7.2f} dB, "
            f"{nearest[2] * 180.0 / cmath.pi:+8.2f} deg, "
            f"|Z| = {abs(impedance):5.2f} ohm"
        )
    return 0


if __name__ == "__main__":
    sys.exit(main())
