"""Lazo de NFB completo del B-18N 1967 en ngspice (B18-009, caracterizacion fisica).

Referencia independiente del motor Rust: inversor 7199 (Koren RCA), dos 7027A
(Koren + division de pantalla calibrada, mismas ecuaciones que el DSP), OT-471
diferencial (Lm, Lleak, Cp, R_core, DCR; parametros `estimated` del overlay),
carga 8 ohm o RLC estimada del altavoz y la NFB de 22 kohm desde la toma de
16 ohm (N8/N16 = 23,18/16,39) al catodo del pentodo 7199.

Variantes:
  - carga:  nominal (8 ohm) | reactiva (RLC estimada, B18-015)
  - NFB:    cerrada (22 kohm) | abierta (1e12 ohm)
  - capacitancias: datasheet (RCA 7199 7-63, RCA 7027-A 8-59) | ninguna

Sin z^-1 ni polo de 2,5 kHz: es la fisica analogica pura del lazo, que es
justo lo que B18-009 debe reproducir en el solver discreto.

Uso:
    python scripts/spice/nfb_loop_reference.py            # ejecuta todas
    python scripts/spice/nfb_loop_reference.py --keep     # conserva netlists
"""

from __future__ import annotations

import argparse
import math
import os
import re
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
NGSPICE = REPO_ROOT / "tools" / "ngspice" / "Spice64" / "bin" / "ngspice_con.exe"
WORK = REPO_ROOT / "target" / "spice_loop"

N8 = 23.18
N16 = 16.39
TAP_GAIN = N8 / N16  # toma de 16 ohm = (N8/N16) * V8 (como feedback_tap_gain)

SUBCKTS = """
.subckt k7199p p g2 g k params: mu=22.2795 ex=1.63827 kg1=3712.07 kp=66.5138 kvb=12.5222 kg2=3807.56
Bvp vp 0 V = {max(v(p,k),0)}
Be1 e1 0 V = {v(g2,k)/kp*ln(1+exp(kp*(1/mu+v(g,k)/v(g2,k))))}
Bip p k I = {2*pwr(max(v(e1),0),ex)/kg1*atan(v(vp)/kvb)}
Big2 g2 k I = {pwr(max(v(g2,k)/mu+v(g,k),0),ex)/kg2}
.ends

.subckt k7199t p g k params: mu=17 ex=0.978282 kg1=448.917 kp=272.472 kvb=160
Bvp vp 0 V = {max(v(p,k),0)}
Be1 e1 0 V = {v(vp)/kp*ln(1+exp(kp*(1/mu+v(g,k)/sqrt(kvb+v(vp)*v(vp)))))}
Bip p k I = {pwr(max(v(e1),0),ex)/kg1}
.ends

* 7027A: Koren + division de pantalla Ig2 = Ip*(F + G*exp(-Vp/V0)) (B18-011).
* Parametros de calibracion/overlay (tube_models.7027a.koren.*).
.subckt k7027a p g2 g k
Be1 e1 0 V = {v(g2,k)/42.405486*ln(1+exp(42.405486*(1/9.0+v(g,k)/v(g2,k))))}
Bip p k I = {2*pwr(max(v(e1),0),1.298317)/1087.896529*atan(max(v(p,k),0)/38.352094)}
Big2 g2 k I = {2*pwr(max(v(e1),0),1.298317)/1087.896529*atan(max(v(p,k),0)/38.352094)*(0.015971+0.142476*exp(-max(v(p,k),0)/255.305))}
.ends
"""


def netlist(load: str, nfb: bool, caps: bool) -> str:
    rfb = 22e3 if nfb else 1e12
    lines = [
        "* B-18N 1967 lazo NFB completo (B18-009, caracterizacion fisica)",
        f"* load={load} nfb={'on' if nfb else 'off'} caps={'datasheet' if caps else 'none'}",
        "vb rail_p 0 dc 380",
        "vt rail_t 0 dc 460",
        "vbias bias 0 dc -55",
        "vscr scr 0 dc 500",
        "vbp bpl 0 dc 505",
        # entrada (mezcla): fuente de 270k
        "vin inx 0 dc 0 ac 1",
        "rsrc inx pin7 270k",
        "rg7 pin7 0 5.6meg",
        # inversor 7199
        "r270 rail_p pin2 270k",
        "r470 rail_p pin3 470k",
        "r150 pin3 0 150k",
        "c01 pin3 pin6 0.1u",
        "r15 pin6 0 1.5k",
        "c02 pin2 pin9 0.02u",
        "r10m pin1 pin9 10meg",
        "r33m pin9 0 3.3meg",
        "r47p rail_t pin1 47k",
        "r47k pin8 0 47k",
        # NFB: toma de 16 ohm (fase negativa como en el motor, feedback_polarity=-1)
        f"rfb tp pin6 {rfb:g}",
        f"Btp tp 0 V = {{-{TAP_GAIN:.6f}*v(s4)}}",
        "xpent pin2 pin3 pin7 pin6 k7199p",
        "xtri pin1 pin9 pin8 k7199t",
        # acoplo a las rejillas de la 7027A (50 nF, 1 kohm, retorno 100 kohm a -55 V)
        "cg1 pin1 n1 50n",
        "rs1 n1 g1 1k",
        "rg1 g1 bias 100k",
        "cg2 pin8 n2 50n",
        "rs2 n2 g2 1k",
        "rg2 g2 bias 100k",
        # etapa de potencia push-pull
        "xp1 pa scr g1 0 k7027a",
        "xp2 pb scr g2 0 k7027a",
        # OT-471 diferencial (overlay estimated); DCR 150 ohm por mitad.
        # El CT (B+ 505 V, AC gnd) alimenta a traves del punto medio de Lm.
        "rdc1 pa a 150",
        "rdc2 pb b 150",
        "rcore a f 500",
        "lm1 f ctm 14.35",
        "lm2 ctm b 14.35",
        "vct bpl ctm 0",
        "ll f g 12m",
        "cp a b 100p",
        f"esec s1 0 b g {{1/{N8}}}",
        "vsense s1 s2 0",
        "rdsec s2 s4 0.4",
        "fpri g b vsense " + f"{{-1/{N8}}}",
    ]
    if load == "nominal":
        lines.append("rload s4 0 8")
    else:
        lines += [
            "re s4 s5 6.5",
            "le s5 m 0.8m",
            "res m 0 23.5",
            "les m 0 0.0693",
            "ces m 0 180.6u",
        ]
    if caps:
        lines += [
            # 7199 pentodo (RCA 7-63): g1-k 5 pF, g1-p 0,06 pF, p-k 2 pF
            "cgk_p pin7 pin6 5p",
            "cgp_p pin7 pin2 0.06p",
            "cpk_p pin2 pin6 2p",
            # 7199 triodo (RCA 7-63): g-p 2 pF, g-k 2,3 pF, p-k 0,3 pF
            "cgp_t pin9 pin1 2p",
            "cgk_t pin9 pin8 2.3p",
            "cpk_t pin1 pin8 0.3p",
            # 7027-A (RCA 8-59): g1-p 1,5 pF, g1-k 10 pF, p-k 7,5 pF (por tubo)
            "cg1p_1 g1 pa 1.5p",
            "cg1k_1 g1 0 10p",
            "cpk_1 pa 0 7.5p",
            "cg1p_2 g2 pb 1.5p",
            "cg1k_2 g2 0 10p",
            "cpk_2 pb 0 7.5p",
        ]
    lines += [
        SUBCKTS,
        ".op",
        ".options method=gear",
        ".control",
        "set numdgt=10",
        "op",
        "print v(pin6) v(pin2) v(pin1) v(pin8) v(pa) v(pb) v(s4)",
        "ac dec 40 20 2meg",
        f"wrdata {WORK.as_posix()}/ac_{load}_{'nfb' if nfb else 'ol'}_{'cap' if caps else 'nocap'}.txt vm(s4)",
        ".endc",
        ".end",
    ]
    return "\n".join(lines) + "\n"


def run(name: str, text: str) -> tuple[str, str]:
    WORK.mkdir(parents=True, exist_ok=True)
    cir = WORK / f"{name}.cir"
    cir.write_text(text, encoding="ascii")
    log = WORK / f"{name}.log"
    result = subprocess.run(
        [str(NGSPICE), "-b", "-o", str(log), str(cir)],
        capture_output=True,
        text=True,
        timeout=600,
    )
    out = log.read_text(errors="ignore") if log.exists() else result.stdout
    return out, str(result.returncode)


def read_ac(path: Path) -> list[tuple[float, float]]:
    rows = []
    for line in path.read_text().splitlines():
        parts = line.split()
        if len(parts) >= 2:
            try:
                rows.append((float(parts[0]), float(parts[1])))
            except ValueError:
                pass
    return rows


def summarize(rows: list[tuple[float, float]]) -> dict:
    def gain_at(f: float) -> float:
        best = min(rows, key=lambda r: abs(math.log(r[0] / f)))
        return 20 * math.log10(best[1])

    peak_f, peak_v = max(rows, key=lambda r: r[1])
    return {
        "gain_1k_db": round(gain_at(1000.0), 2),
        "gain_20k_db": round(gain_at(20000.0), 2),
        "gain_100k_db": round(gain_at(100000.0), 2),
        "gain_500k_db": round(gain_at(500000.0), 2),
        "gain_2M_db": round(gain_at(2000000.0), 2),
        "peak_db_rel_1k": round(20 * math.log10(peak_v) - gain_at(1000.0), 2),
        "peak_f_hz": round(peak_f, 1),
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--keep", action="store_true")
    args = parser.parse_args()
    if not NGSPICE.exists():
        print(f"no encontrado: {NGSPICE}", file=sys.stderr)
        return 2

    variants = [
        ("nominal", False, True),
        ("nominal", True, True),
        ("reactive", False, True),
        ("reactive", True, True),
        ("nominal", True, False),
        ("reactive", True, False),
    ]
    for load, nfb, caps in variants:
        name = f"ac_{load}_{'nfb' if nfb else 'ol'}_{'cap' if caps else 'nocap'}"
        out, rc = run(name, netlist(load, nfb, caps))
        errors = [l for l in out.splitlines() if re.search(r"error|fatal|singular", l, re.I)]
        data = WORK / f"{name}.txt"
        print(f"== {name} rc={rc} errores={len(errors)}")
        for e in errors[:3]:
            print("   ", e.strip())
        if data.exists():
            print("   ", summarize(read_ac(data)))
        op = [l for l in out.splitlines() if l.strip().startswith("v(")]
        for l in op[:2]:
            print("   op:", l.strip())
    return 0


if __name__ == "__main__":
    sys.exit(main())
