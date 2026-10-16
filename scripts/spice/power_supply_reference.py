"""Referencia SPICE de la fuente dinamica B18-012 (RC + rectificador promedio).

Ejecuta `b18n_power_supply_1967.cir` (.op) y un transitorio con escalon de
+20 mA en H2, y escribe:

    assets/fixtures/reference/power_supply/b18n_power_supply_dc_1967_golden.txt
    assets/fixtures/reference/power_supply/b18n_power_supply_tran_1967_golden.csv

Uso:
    python scripts/spice/power_supply_reference.py

Requiere ngspice 47 (véase scripts/spice/README.md).
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

from run_spice import find_ngspice

REPO_ROOT = Path(__file__).resolve().parents[2]
SPICE_DIR = REPO_ROOT / "scripts" / "spice"
OUT_DIR = REPO_ROOT / "assets" / "fixtures" / "reference" / "power_supply"
WORK_DIR = REPO_ROOT / "target" / "reports" / "spice_power_supply"

DC_NETLIST = SPICE_DIR / "b18n_power_supply_1967.cir"
DC_GOLDEN = OUT_DIR / "b18n_power_supply_dc_1967_golden.txt"
TRAN_GOLDEN = OUT_DIR / "b18n_power_supply_tran_1967_golden.csv"

VOLT_RE = re.compile(r"v\((h2|h1|h3|h4|vbias)\)\s*=\s*([-+0-9.eE]+)", re.IGNORECASE)


def run_ngspice(netlist_text: str, name: str) -> str:
    WORK_DIR.mkdir(parents=True, exist_ok=True)
    netlist = WORK_DIR / name
    netlist.write_text(netlist_text, encoding="utf-8")
    binary = find_ngspice()
    log = netlist.with_suffix(netlist.suffix + ".log")
    completed = subprocess.run(
        [str(binary), "-b", "-o", str(log), str(netlist.name)],
        cwd=str(WORK_DIR),
        check=False,
        capture_output=True,
        text=True,
    )
    out = log.read_text(encoding="utf-8", errors="replace") if log.is_file() else ""
    if completed.returncode != 0:
        print(out[-4000:])
        raise SystemExit(f"ngspice fallo ({completed.returncode}) en {name}")
    return out


def dc_reference() -> dict[str, float]:
    out = run_ngspice(
        DC_NETLIST.read_text(encoding="utf-8"), "b18n_power_supply_dc.cir"
    )
    values: dict[str, float] = {}
    for match in VOLT_RE.finditer(out):
        values[match.group(1).lower()] = float(match.group(2))
    for node in ("h2", "h1", "h3", "h4", "vbias"):
        if node not in values:
            raise SystemExit(f"sin {node} en la salida DC:\n{out[-2000:]}")
    return values


TRAN_NETLIST = """* Fuente B18-012 - transitorio con escalon +20 mA en H2
Vsrc hv_src 0 DC 518.935
Rrect hv_src h2 150
Ch2 h2 0 30u
R1k h2 h1 1k
Ch1 h1 0 40u
R10k h1 h3 10k
Ch3 h3 0 40u
R22k h1 h4 22k
Ch4 h4 0 40u
Ict h2 0 DC 0.083
Iscr h1 0 DC 0.003
Itri h3 0 DC 0.0023
Ipre h4 0 DC 0.0046
Vb_src bias_src 0 DC -55
Rb bias_src vbias 15k
Cb vbias 0 100u
Ibias vbias 0 DC 0
Istep h2 0 PULSE(0 0.02 50m 1u 1u 100m 400m)
.tran 0.25m 200m
.control
set numdgt=10
run
wrdata tran_out.txt v(h2) v(h1) v(h3) v(h4)
.endc
.end
"""


def tran_reference() -> list[tuple[float, float, float, float, float]]:
    out = run_ngspice(TRAN_NETLIST, "b18n_power_supply_tran.cir")
    data_file = WORK_DIR / "tran_out.txt"
    if not data_file.is_file():
        raise SystemExit(f"sin tran_out.txt:\n{out[-2000:]}")
    rows: list[tuple[float, float, float, float, float]] = []
    for line in data_file.read_text(encoding="utf-8", errors="replace").splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        parts = line.split()
        if len(parts) < 5:
            continue
        try:
            values = tuple(float(p) for p in parts[:5])
        except ValueError:
            continue
        rows.append(values)  # type: ignore[arg-type]
    if len(rows) < 100:
        raise SystemExit(f"transitorio incompleto ({len(rows)} filas)")
    return rows


def main() -> int:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    dc = dc_reference()
    DC_GOLDEN.write_text(
        "\n".join(
            [
                "# B-18N 1967 - Fuente B18-012, punto DC con cargas de reposo del modelo",
                "# ngspice 47 sobre scripts/spice/b18n_power_supply_1967.cir",
                "# Condiciones: Vsrc=518.935 V, Rrect=150 ohm, 1k/10k/22k,",
                "#   30/40/40/40 uF, bias -55 V/15k/100 uF,",
                "#   Ict=83 mA, Iscr=3 mA, Itri=2.3 mA, Ipre=4.6 mA, Ibias=0.",
                f"v(h2) = {dc['h2']:.6f}",
                f"v(h1) = {dc['h1']:.6f}",
                f"v(h3) = {dc['h3']:.6f}",
                f"v(h4) = {dc['h4']:.6f}",
                f"v(vbias) = {dc['vbias']:.6f}",
                "",
            ]
        ),
        encoding="utf-8",
    )
    print(f"DC: {dc}")

    rows = tran_reference()
    with TRAN_GOLDEN.open("w", encoding="utf-8", newline="") as handle:
        handle.write("time_s,v_h2,v_h1,v_h3,v_h4\n")
        for row in rows:
            handle.write(f"{row[0]:.9f},{row[1]:.6f},{row[2]:.6f},{row[3]:.6f},{row[4]:.6f}\n")
    print(f"tran: {len(rows)} filas -> {TRAN_GOLDEN}")
    # Métricas rápidas para el log.
    v_h2 = [r[1] for r in rows]
    t = [r[0] for r in rows]
    pre = sum(v for tt, v in zip(t, v_h2) if tt < 0.04) / max(
        1, sum(1 for tt in t if tt < 0.04)
    )
    sag = min(v for tt, v in zip(t, v_h2) if 0.05 < tt < 0.15)
    print(f"B+ pre = {pre:.2f} V, sag min = {sag:.2f} V, caida = {pre - sag:.2f} V")
    return 0


if __name__ == "__main__":
    sys.exit(main())
