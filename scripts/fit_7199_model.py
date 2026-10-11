"""Ajusta los parametros Koren de la 7199 (pentodo y triodo) a anclas RCA.

Fuente: RCA 7199, DATA 1/2, 7-63 (Medium-Mu Triode - Sharp-Cutoff Pentode).
Anclas del boletin (caracteristicas Class A1 tipicas y punto de corte):

    Triodo:
      A1: Vp=215 V, Vgk=-8.5 V  -> Ip=9 mA
      gm=2100 umhos, rp=8.1 kohm, mu=17 en A1
      Corte: Vp=215 V, Vgk=-40 V -> Ip=10 uA

    Pentodo (Vs = pantalla-catodo):
      A1: Vp=100 V, Vs=50 V,  Vgk=-1.1 V   -> Ip=1.1 mA,  Ig2=0.35 mA
      A2: Vp=220 V, Vs=130 V, Vgk=-0.775 V -> Ip=12.5 mA, Ig2=3.5 mA
      Corte: Vp=100 V, Vs=50 V, Vgk=-4 V   -> Ip=10 uA
      gm=1500 umhos y rp=1 Mohm en A1

Las ecuaciones replican exactamente
`crates/b18n-dsp/src/stages/tube_models.rs` (Koren publico). Los parametros
resultantes se registran en el overlay de calibracion con origen `datasheet` y
metodo documentado; no son valores historicos del JSON.

Uso:
    python scripts/fit_7199_model.py
"""

from __future__ import annotations

import json
import math

import numpy as np
from scipy.optimize import least_squares

DATE = "2026-10-09"
METHOD = "least_squares (scipy) a anclas del boletin RCA 7199 DATA 1/2 (7-63)"
SOURCE = "RCA 7199 datasheet 1963 (assets/reference/7199_RCA_1963.pdf)"


def softplus(x: float) -> float:
    if x > 40.0:
        return x
    if x < -40.0:
        return 0.0
    return math.log1p(math.exp(x))


def triode_ip(vp: float, vg: float, p: np.ndarray) -> float:
    mu, ex, kg1, kp, kvb = p
    vp = max(vp, 0.0)
    root = math.sqrt(kvb + vp * vp)
    e1 = vp / kp * softplus(kp * (1.0 / mu + vg / root))
    if e1 <= 0.0:
        return 0.0
    return e1**ex / kg1


def pentode_ip(vp: float, vs: float, vg: float, p: np.ndarray) -> float:
    mu, ex, kg1, kp, kvb = p[:5]
    e1 = vs / kp * softplus(kp * (1.0 / mu + vg / vs))
    if e1 <= 0.0:
        return 0.0
    return 2.0 * e1**ex / kg1 * math.atan(max(vp, 0.0) / kvb)


def pentode_ig2(vs: float, vg: float, p: np.ndarray) -> float:
    mu, ex, kg1, kp, kvb = p[:5]
    base = vs / mu + vg
    if base <= 1e-9:
        return 0.0
    return base**ex / kg2_from(p)


def kg2_from(p: np.ndarray) -> float:
    return p[5]


def triode_residuals(u: np.ndarray) -> np.ndarray:
    p = np.exp(u)
    out = []
    ip_a1 = triode_ip(215.0, -8.5, p)
    out.append(1.5 * math.log(max(ip_a1, 1e-12) / 9e-3))

    dv = 1e-3
    gm = (triode_ip(215.0, -8.5 + dv, p) - triode_ip(215.0, -8.5 - dv, p)) / (2 * dv)
    rp_inv = (triode_ip(215.0 + dv, -8.5, p) - triode_ip(215.0 - dv, -8.5, p)) / (2 * dv)
    rp = 1.0 / max(rp_inv, 1e-12)
    out.append(1.5 * math.log(max(gm, 1e-12) / 2.1e-3))
    out.append(1.5 * math.log(max(rp, 1e-12) / 8.1e3))
    out.append(0.05 * math.log(max(p[1], 1e-12) / 1.40))
    return np.array(out)


def pentode_residuals(u: np.ndarray) -> np.ndarray:
    p = np.exp(u)
    out = []
    out.append(1.5 * math.log(max(pentode_ip(100.0, 50.0, -1.1, p), 1e-12) / 1.1e-3))
    out.append(1.5 * math.log(max(pentode_ip(220.0, 130.0, -0.775, p), 1e-12) / 12.5e-3))
    out.append(1.0 * math.log(max(pentode_ip(100.0, 50.0, -4.0, p), 1e-12) / 10e-6))
    out.append(1.0 * math.log(max(pentode_ig2(50.0, -1.1, p), 1e-12) / 0.35e-3))
    out.append(1.0 * math.log(max(pentode_ig2(130.0, -0.775, p), 1e-12) / 3.5e-3))

    dv = 1e-4
    gm = (
        pentode_ip(100.0, 50.0, -1.1 + dv, p) - pentode_ip(100.0, 50.0, -1.1 - dv, p)
    ) / (2 * dv)
    rp_inv = (
        pentode_ip(100.0 + 0.5, 50.0, -1.1, p) - pentode_ip(100.0 - 0.5, 50.0, -1.1, p)
    ) / (2 * 0.5)
    rp = 1.0 / max(rp_inv, 1e-12)
    out.append(0.7 * math.log(max(gm, 1e-12) / 1.5e-3))
    out.append(0.5 * math.log(max(rp, 1e-12) / 1.0e6))
    out.append(0.15 * math.log(max(p[0], 1e-12) / 20.0))
    out.append(0.2 * math.log(max(p[1], 1e-12) / 1.35))
    return np.array(out)


def fit_triode() -> np.ndarray:
    """Ajusta ex/kg1/kp con mu=17 y kvb=160 fijados.

    El boletin da un unico punto de placa (Vp=215, Vgk=-8.5 -> 9 mA, rp=8.1 k,
    gm=2100) que no determina `kvb` (dependencia con Vp). Se fija el borde
    inferior del rango compatible con la curvatura de la familia de placa y con
    el reposo del B-18N (~2.1 mA a Vgk~-11, Vp~350); valores mayores disparan
    la extrapolacion a Vp alto. Queda documentado como limitacion.
    """
    kvb = 160.0

    def residuals(u: np.ndarray) -> np.ndarray:
        ex, kg1, kp = np.exp(u)
        return triode_residuals(np.log([17.0, ex, kg1, kp, kvb]))

    lower = np.log([0.5, 10.0, 10.0])
    upper = np.log([3.0, 1.0e5, 2.0e3])
    best = None
    for start in (
        [1.0, 500.0, 900.0],
        [1.3, 1000.0, 100.0],
        [1.35, 800.0, 600.0],
    ):
        result = least_squares(residuals, np.log(start), bounds=(lower, upper), max_nfev=50000)
        cost = float(np.linalg.norm(residuals(result.x)))
        if best is None or cost < best[0]:
            best = (cost, result.x)
    ex, kg1, kp = np.exp(best[1])
    return np.array([17.0, ex, kg1, kp, kvb])


def fit(residuals, initial: np.ndarray, kg2: float | None) -> np.ndarray:
    lower = np.log(np.array([3.0, 0.8, 10.0, 10.0, 5.0]))
    upper = np.log(np.array([100.0, 2.5, 1.0e5, 2.0e3, 2.0e3]))
    if kg2 is not None:
        lower = np.append(lower, math.log(1.0))
        upper = np.append(upper, math.log(1.0e6))
        initial = np.append(initial, math.log(kg2))
    result = least_squares(residuals, np.log(initial), bounds=(lower, upper), max_nfev=20000)
    return np.exp(result.x)


def report_triode(p: np.ndarray) -> None:
    print("  anclas triodo:")
    for vp, vg, target, label in [
        (215.0, -8.5, 9e-3, "A1 Ip"),
        (215.0, -40.0, 10e-6, "corte Ip"),
    ]:
        model = triode_ip(vp, vg, p)
        print(f"    {label:9s} Vp={vp:.0f} Vg={vg:+.1f}: {model*1e3:9.4f} mA vs {target*1e3:9.4f} mA")
    dv = 1e-3
    gm = (triode_ip(215.0, -8.5 + dv, p) - triode_ip(215.0, -8.5 - dv, p)) / (2 * dv)
    rp = 1.0 / (
        (triode_ip(215.0 + dv, -8.5, p) - triode_ip(215.0 - dv, -8.5, p)) / (2 * dv)
    )
    print(f"    gm        {gm*1e6:.0f} umhos (objetivo 2100)")
    print(f"    rp        {rp/1e3:.2f} kohm (objetivo 8.10)")


def report_pentode(p: np.ndarray) -> None:
    print("  anclas pentodo:")
    for vp, vs, vg, target, label in [
        (100.0, 50.0, -1.1, 1.1e-3, "A1 Ip"),
        (220.0, 130.0, -0.775, 12.5e-3, "A2 Ip"),
        (100.0, 50.0, -4.0, 10e-6, "corte Ip"),
    ]:
        model = pentode_ip(vp, vs, vg, p)
        print(
            f"    {label:9s} Vp={vp:.0f} Vs={vs:.0f} Vg={vg:+.3f}: "
            f"{model*1e3:9.4f} mA vs {target*1e3:9.4f} mA"
        )
    for vs, vg, target, label in [
        (50.0, -1.1, 0.35e-3, "A1 Ig2"),
        (130.0, -0.775, 3.5e-3, "A2 Ig2"),
    ]:
        model = pentode_ig2(vs, vg, p)
        print(f"    {label:9s} Vs={vs:.0f} Vg={vg:+.3f}: {model*1e3:9.4f} mA vs {target*1e3:9.4f} mA")
    dv = 1e-4
    gm = (
        pentode_ip(100.0, 50.0, -1.1 + dv, p) - pentode_ip(100.0, 50.0, -1.1 - dv, p)
    ) / (2 * dv)
    rp = 1.0 / (
        (pentode_ip(100.5, 50.0, -1.1, p) - pentode_ip(99.5, 50.0, -1.1, p)) / (2 * 0.5)
    )
    print(f"    gm        {gm*1e6:.0f} umhos (objetivo 1500)")
    print(f"    rp        {rp/1e6:.3f} Mohm (objetivo 1.0)")


def calibration_entries(prefix: str, names, values) -> dict:
    entries = {}
    for name, value in zip(names, values):
        entries[f"tube_models.7199.{prefix}.koren.{name}"] = {
            "value": value,
            "unit": "1",
            "origin": "datasheet",
            "method": METHOD,
            "date": DATE,
            "source": SOURCE,
        }
    return entries


def main() -> None:
    triode_p = fit_triode()
    pentode_p = fit(
        pentode_residuals, np.array([20.0, 1.35, 800.0, 500.0, 20.0]), kg2=8000.0
    )

    print("Triodo 7199 (mu, ex, kg1, kp, kvb):")
    print("  " + ", ".join(f"{v:.6g}" for v in triode_p))
    report_triode(triode_p)
    print("\nPentodo 7199 (mu, ex, kg1, kp, kvb, kg2):")
    print("  " + ", ".join(f"{v:.6g}" for v in pentode_p))
    report_pentode(pentode_p)

    entries = {}
    entries.update(
        calibration_entries(
            "triode", ["mu", "ex", "kg1", "kp", "kvb"], [round(float(v), 6) for v in triode_p]
        )
    )
    entries.update(
        calibration_entries(
            "pentode",
            ["mu", "ex", "kg1", "kp", "kvb", "kg2"],
            [round(float(v), 6) for v in pentode_p],
        )
    )
    print("\nFragmento para el overlay de calibracion:")
    print(json.dumps(entries, indent=2))


if __name__ == "__main__":
    main()
