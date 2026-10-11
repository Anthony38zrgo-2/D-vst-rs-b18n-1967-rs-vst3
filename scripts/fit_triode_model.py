"""Ajusta los parámetros Koren de un triodo a puntos documentados.

Ecuaciones de Norman Koren (públicas):
    E1 = (Vp/Kp)·ln(1 + exp(Kp·(1/µ + Vg_ef/sqrt(Kvb + Vp²))))
    Ip = E1^Ex / Kg1   (E1 > 0; Ip = 0 en caso contrario)

Anclas del proyecto (todas del JSON histórico, procedentes de datasheet RCA
y de las tensiones típicas del esquema 5/67):

    6SL7GT  µ = 70 (datasheet)
    A1: Vp=175 V, Vgk=-2.4 V, Ip=0.4286 mA  (triodo 1: 470k/5.6k, 175 V, 2.4 V)
    A2: Vp=185 V, Vgk=-2.0 V, Ip=0.9091 mA  (triodo 2: 220k/2.2k, 185 V, 2.0 V)
    A3: Vp=250 V, Vgk=-2.0 V, Ip=2.3 mA     (referencia de pequeña señal a 250 V)
    En A3: gm = 1.6 mS y rp = 44 kΩ         (referencia del JSON)

Uso:
    python scripts/fit_triode_model.py
Imprime el fragmento JSON listo para el archivo de calibración.
"""

from __future__ import annotations

import json
import math

import numpy as np

MU_REFERENCE = 70.0
EX_REFERENCE = 1.35

ANCHORS = [
    # (vp, vgk, ip, peso)
    (175.0, -2.4, 0.4286e-3, 1.0),
    (185.0, -2.0, 0.9091e-3, 1.0),
    (250.0, -2.0, 2.3e-3, 1.5),
]
GM_TARGET = 1.6e-3
GM_WEIGHT = 0.4
RP_TARGET = 44e3
RP_WEIGHT = 0.4
MU_WEIGHT = 0.3
EX_WEIGHT = 0.3


def plate_current(
    vp: float, vgk: float, mu: float, kp: float, kvb: float, ex: float, kg1: float
) -> float:
    vp = max(vp, 0.0)
    inner = kp * (1.0 / mu + vgk / math.sqrt(kvb + vp * vp))
    if inner > 40.0:
        e1 = vp / kp * inner
    elif inner < -40.0:
        return 0.0
    else:
        e1 = vp / kp * math.log1p(math.exp(inner))
    if e1 <= 0.0:
        return 0.0
    return e1**ex / kg1


def residuals(params: np.ndarray) -> np.ndarray:
    mu, kp, kvb, ex, kg1 = params
    out = []
    for vp, vgk, ip, weight in ANCHORS:
        model = plate_current(vp, vgk, mu, kp, kvb, ex, kg1)
        out.append(weight * math.log(max(model, 1e-12) / ip))
    # Derivadas numéricas en el ancla A3.
    vp, vgk, _, _ = ANCHORS[2]
    dv = 1e-3
    gm = (
        plate_current(vp, vgk + dv, mu, kp, kvb, ex, kg1)
        - plate_current(vp, vgk - dv, mu, kp, kvb, ex, kg1)
    ) / (2.0 * dv)
    rp_inv = (
        plate_current(vp + dv, vgk, mu, kp, kvb, ex, kg1)
        - plate_current(vp - dv, vgk, mu, kp, kvb, ex, kg1)
    ) / (2.0 * dv)
    rp = 1.0 / max(rp_inv, 1e-12)
    out.append(GM_WEIGHT * math.log(max(gm, 1e-12) / GM_TARGET))
    out.append(RP_WEIGHT * math.log(max(rp, 1e-12) / RP_TARGET))
    # Suavizado hacia los valores típicos de Koren/datasheet.
    out.append(MU_WEIGHT * math.log(max(mu, 1e-12) / MU_REFERENCE))
    out.append(EX_WEIGHT * math.log(max(ex, 1e-12) / EX_REFERENCE))
    return np.array(out)


def gauss_newton(initial: np.ndarray, iterations: int = 200) -> np.ndarray:
    params = initial.copy()
    damping = 1e-3
    for _ in range(iterations):
        r = residuals(params)
        jac = np.zeros((len(r), len(params)))
        for index in range(len(params)):
            step = np.zeros(len(params))
            step[index] = max(1e-6, abs(params[index]) * 1e-4)
            jac[:, index] = (residuals(params + step) - residuals(params - step)) / (
                2.0 * step[index]
            )
        jtj = jac.T @ jac + damping * np.eye(len(params))
        jtr = jac.T @ r
        delta = np.linalg.solve(jtj, -jtr)
        params = params + delta
        if np.linalg.norm(delta) < 1e-10:
            break
    return params


def main() -> None:
    initial = np.array([70.0, 600.0, 300.0, 1.35, 811.0])
    fitted = gauss_newton(initial)
    mu, kp, kvb, ex, kg1 = fitted

    print("Parametros ajustados (Koren):")
    for name, value in zip(["mu", "kp", "kvb", "ex", "kg1"], fitted):
        print(f"  {name} = {value:.6g}")

    print("\nContraste de anclas:")
    for vp, vgk, ip, _ in ANCHORS:
        model = plate_current(vp, vgk, mu, kp, kvb, ex, kg1)
        print(f"  Vp={vp:5.0f} Vgk={vgk:+.1f}: Ip modelo {model*1e3:7.4f} mA vs {ip*1e3:7.4f} mA")

    vp, vgk, _, _ = ANCHORS[2]
    dv = 1e-3
    gm = (
        plate_current(vp, vgk + dv, mu, kp, kvb, ex, kg1)
        - plate_current(vp, vgk - dv, mu, kp, kvb, ex, kg1)
    ) / (2.0 * dv)
    rp = 1.0 / (
        (
            plate_current(vp + dv, vgk, mu, kp, kvb, ex, kg1)
            - plate_current(vp - dv, vgk, mu, kp, kvb, ex, kg1)
        )
        / (2.0 * dv)
    )
    print(
        f"  A3: gm {gm*1e3:.3f} mS (objetivo {GM_TARGET*1e3}), "
        f"rp {rp/1e3:.2f} kohm (objetivo {RP_TARGET/1e3})"
    )

    entry = {
        "mu": round(float(mu), 4),
        "ex": round(float(ex), 6),
        "kg1": round(float(kg1), 4),
        "kp": round(float(kp), 4),
        "kvb": round(float(kvb), 4),
        "vct": 0.0,
    }
    print("\nFragmento para la calibracion:")
    print(json.dumps(entry, indent=2))


if __name__ == "__main__":
    main()
