"""Referencia independiente de la red de tono (implementación numpy).

Resuelve el mismo netlist documentado en `docs/CIRCUIT_TRACEABILITY.md` y en
`crates/b18n-dsp/src/stages/tone_stack.rs`, pero con código y solver
independientes (numpy). No valida la topología por sí misma: sirve para
contrastar el solver Rust contra una segunda implementación y como referencia
interina hasta poder ejecutar SPICE (ngspice) del mismo netlist.

Netlist (por canal):
    IN  --220k-- B
    IN  --0.00047µF-- T1
    B   --0.001µF-- W
    C   --0.01µF-- W
    POTE GRAVES 1M: B-W-C (cursor W)
    22k: C -- GND
    270k: W -- GND
    120k: W -- T1
    POTE AGUDOS 1M: T1-OUT-T2 (cursor OUT)
    0.0047µF: T2 -- GND

Fuente y carga documentadas: Rfuente = 100 kΩ, Rcarga = 270 kΩ.

Uso:
    python scripts/tone_stack_reference.py assets/fixtures/reference/tone_stack/tone_stack_numpy.csv
"""

from __future__ import annotations

import csv
import sys
from pathlib import Path

import numpy as np

# Valores del JSON histórico (identificados por valor, no por posición).
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

# Rejilla ISO de tercios de octava de 20 Hz a 20 kHz.
FREQUENCIES_HZ = [
    20.0, 25.0, 31.5, 40.0, 50.0, 63.0, 80.0, 100.0, 125.0, 160.0, 200.0,
    250.0, 315.0, 400.0, 500.0, 630.0, 800.0, 1000.0, 1250.0, 1600.0,
    2000.0, 2500.0, 3150.0, 4000.0, 5000.0, 6300.0, 8000.0, 10000.0,
    12500.0, 16000.0, 20000.0,
]

# Nodos del sistema (masa implícita, no forma parte de la matriz).
N_IN, N_B, N_W, N_C, N_T1, N_T2, N_OUT = range(7)
NODE_COUNT = 7
GND = -1


def stamp_admittance(y, node_a: int, node_b: int, admittance: complex) -> None:
    if node_a >= 0:
        y[node_a, node_a] += admittance
    if node_b >= 0:
        y[node_b, node_b] += admittance
    if node_a >= 0 and node_b >= 0:
        y[node_a, node_b] -= admittance
        y[node_b, node_a] -= admittance


def stamp_resistor(y, node_a: int, node_b: int, ohms: float) -> None:
    stamp_admittance(y, node_a, node_b, 1.0 / ohms)


def stamp_capacitor(y, node_a: int, node_b: int, farads: float, omega: float) -> None:
    stamp_admittance(y, node_a, node_b, 1j * omega * farads)


def response(bass_position: float, treble_position: float, freq_hz: float) -> complex:
    omega = 2.0 * np.pi * freq_hz
    y = np.zeros((NODE_COUNT, NODE_COUNT), dtype=complex)
    rhs = np.zeros(NODE_COUNT, dtype=complex)

    p = min(max(bass_position, 1e-6), 1.0 - 1e-6)
    r_bw = (1.0 - p) * POT_BASS
    r_wc = p * POT_BASS
    q = min(max(treble_position, 1e-6), 1.0 - 1e-6)
    r_t_out = (1.0 - q) * POT_TREBLE
    r_out_t2 = q * POT_TREBLE

    stamp_resistor(y, N_IN, GND, SOURCE_OHMS)
    stamp_resistor(y, N_IN, N_B, R_SERIES)
    stamp_capacitor(y, N_IN, N_T1, C_TREBLE_SERIES, omega)
    stamp_capacitor(y, N_B, N_W, C_BASS_TOP, omega)
    stamp_capacitor(y, N_C, N_W, C_BASS_BOTTOM, omega)
    stamp_resistor(y, N_B, N_W, r_bw)
    stamp_resistor(y, N_W, N_C, r_wc)
    stamp_resistor(y, N_C, GND, R_BASS_STOP)
    stamp_resistor(y, N_W, GND, R_BASS_CUT)
    stamp_resistor(y, N_W, N_T1, R_MID)
    stamp_resistor(y, N_T1, N_OUT, r_t_out)
    stamp_resistor(y, N_OUT, N_T2, r_out_t2)
    stamp_capacitor(y, N_T2, GND, C_TREBLE_SHUNT, omega)
    stamp_resistor(y, N_OUT, GND, LOAD_OHMS)

    rhs[N_IN] = 1.0
    voltages = np.linalg.solve(y, rhs)
    return complex(voltages[N_OUT] / voltages[N_IN])


def main(argv: list[str]) -> int:
    destination = Path(argv[1]) if len(argv) > 1 else Path(
        "assets/fixtures/reference/tone_stack/tone_stack_numpy.csv"
    )
    destination.parent.mkdir(parents=True, exist_ok=True)

    rows = 0
    with destination.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.writer(handle)
        writer.writerow(["bass", "treble", "freq_hz", "mag_db", "phase_deg"])
        for bass in POSITIONS:
            for treble in POSITIONS:
                for freq in FREQUENCIES_HZ:
                    h = response(bass, treble, freq)
                    writer.writerow(
                        [
                            f"{bass:.2f}",
                            f"{treble:.2f}",
                            f"{freq:.1f}",
                            f"{20.0 * np.log10(abs(h)):.9f}",
                            f"{np.degrees(np.angle(h)):.6f}",
                        ]
                    )
                    rows += 1

    print(f"escrito {destination} ({rows} filas)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
