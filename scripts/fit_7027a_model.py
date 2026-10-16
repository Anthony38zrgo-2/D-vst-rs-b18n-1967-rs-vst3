"""Ajusta los parametros Koren de la 7027A a datos del datasheet RCA (B18-010/011).

Fuentes:
1. `assets/fixtures/reference/power_7027a/7027a_rca_digitized.csv` (curvas
   digitalizadas del escaneo por `scripts/digitize_7027a_curves.py`).
2. Tablas del propio datasheet RCA 7027-A (8-59), transcritas aqui como anclas:
   - Clase A1 (una valvula): Va=250 V, Vg2=250 V, Vg1=-14 V -> Ip=72 mA,
     Ig2=5 mA, gm=6000 umhos, rp=22.5 kOhm.
   - Bias fija (valores por valvula = tabla/2):
       (400 V, 300 V, -25 V): Ip0=51 mA, Ip_max=76 mA, Ig2_0=3.0 mA, Ig2_max=8.5 mA
       (450 V, 350 V, -30 V): Ip0=47.5 mA, Ip_max=97 mA, Ig2_0=1.7 mA, Ig2_max=9.6 mA
       (540 V, 400 V, -38 V): Ip0=50 mA, Ip_max=110 mA, Ig2_0=2.5 mA, Ig2_max=10.7 mA
     La Ig2 de "max signal" se evalua con la rejilla en 0 V (la tabla define el
     pico de excitacion como "hasta 0 V") y Vp de rodilla estimada 75 V +-30 V
     (no tabulada; documentado como supuesto de ajuste).

Las ecuaciones de placa replican exactamente
`crates/b18n-dsp/src/stages/tube_models.rs`:

    E1  = (Vs/Kp)*softplus(Kp*(1/mu + (Vg+Vct)/Vs))
    Ip  = 2*max(E1,0)**Ex/Kg1 * atan(max(Vp,0)/Kvb)

Para la corriente de pantalla se calibra la **division de corriente**
(B18-011), extendiendo el modelo con tres parametros:

    Ig2 = Ip(Vp, Vs, Vg) * (F + G*exp(-max(Vp,0)/V0))

`F` (screen_ratio_floor) es la fraccion de placa desviada a pantalla en
conduccion normal; `G` (screen_ratio_boost) y `V0` (screen_ratio_volts)
anaden la desviacion extra en la rodilla de placa. La motivacion: las tablas
RCA dan Ig2/Ip = 3.6-5.9 % y la curva digitalizada de p07 da 6-7 % con la
misma forma de transferencia que la placa: la forma Koren `base^ex2/kg2` de un
solo exponente no reproduce a la vez la tabla y la curva (la tabla exige
ex2 ~ 0.8 y p07 ~ 1.9), mientras que la division si. Con los tres parametros
ausentes (0, 0, *) el modelo de pantalla conserva la forma Koren publicada
(uso del 7199 y de la 6L6GC sin recalibrar).

Los parametros resultantes se registran en el overlay de calibracion con
origen `datasheet` (valores explicitos) / `estimated` (inferidos del ajuste),
metodo y fecha; nunca modifican nominales del JSON canonico.

Uso:
    python scripts/fit_7027a_model.py
"""

from __future__ import annotations

import csv
import json
import math
from pathlib import Path

import numpy as np
from scipy.optimize import least_squares

ROOT = Path(__file__).resolve().parent.parent
DIGITIZED = ROOT / "assets/fixtures/reference/power_7027a/7027a_rca_digitized.csv"

DATE = "2026-10-14"
SOURCE = "RCA 7027-A datasheet 8-59 (assets/reference/7027A_RCA.pdf)"
METHOD_FIT = (
    "ajuste least_squares (scipy) a curvas digitalizadas del datasheet RCA y a "
    "sus tablas de operacion tipica (clase A1 y bias fija)"
)
METHOD_KNEE = (
    "ajuste least_squares de la division Ig2/Ip = F + G*exp(-Vp/V0) a la curva "
    "digitalizada p07 y a las tablas de clase A1 y bias fija (Ig2 en rodilla con "
    "Vp estimado 75 V +-30 V, no tabulado); extension de B18-011"
)

VCT = 0.0

# Anclas tabuladas (por valvula).
CLASS_A1 = dict(vp=250.0, vs=250.0, vg=-14.0, ip=72e-3, ig2=5e-3, gm=6000e-6, rp=22.5e3)
FIXED_BIAS = (
    dict(va=400.0, vs=300.0, vg=-25.0, ip0=51e-3, ip_max=76e-3, ig2_0=3.0e-3, ig2_max=8.5e-3),
    dict(va=450.0, vs=350.0, vg=-30.0, ip0=47.5e-3, ip_max=97e-3, ig2_0=1.7e-3, ig2_max=9.6e-3),
    dict(va=540.0, vs=400.0, vg=-38.0, ip0=50e-3, ip_max=110e-3, ig2_0=2.5e-3, ig2_max=10.7e-3),
)
VKNEE_ESTIMATE_V = 75.0


def softplus(x: float) -> float:
    if x > 40.0:
        return x
    if x < -40.0:
        return 0.0
    return math.log1p(math.exp(x))


def plate_current(vp: float, vs: float, vg: float, p: np.ndarray) -> float:
    mu, ex, kg1, kp, kvb = p[:5]
    if vs <= 1e-6:
        return 0.0
    inner = kp * (1.0 / mu + (vg + VCT) / vs)
    e1 = vs / kp * softplus(inner)
    if e1 <= 0.0:
        return 0.0
    return 2.0 * e1**ex / kg1 * math.atan(max(vp, 0.0) / kvb)


def screen_current(vp: float, vs: float, vg: float, p: np.ndarray, ratio: np.ndarray) -> float:
    floor, boost, volts = ratio
    fraction = floor + boost * math.exp(-max(vp, 0.0) / volts)
    return plate_current(vp, vs, vg, p) * fraction


def load_digitized() -> tuple[list[tuple], list[tuple]]:
    p05: list[tuple] = []
    p07: list[tuple] = []
    with DIGITIZED.open(encoding="utf-8") as handle:
        for row in csv.DictReader(handle):
            if not row["ip_ma"] or row["figure"] == "p06":
                continue
            vg1 = float(row["vg1_v"])
            vs = float(row["ec2_v"])
            vp = float(row["vp_v"])
            if row["figure"] == "p05":
                p05.append((vp, vs, vg1, float(row["ip_ma"]) * 1e-3))
            elif row["figure"] == "p07":
                p07.append((vp, vs, vg1, float(row["ip_ma"]) * 1e-3))
    return p05, p07


def plate_residuals(u: np.ndarray) -> np.ndarray:
    p = np.exp(u)
    out: list[float] = []
    p05, p07 = load_digitized()
    for vp, vs, vg, ip in p05:
        model = plate_current(vp, vs, vg, p)
        out.append(math.log(max(model, 1e-9) / max(ip, 1e-9)))
    for vp, vs, vg, ip in p07:
        model = plate_current(vp, vs, vg, p)
        out.append(math.log(max(model, 1e-9) / max(ip, 1e-9)))

    # Tablas: peso mayor (numeros impresos, no digitalizados).
    a1 = CLASS_A1
    weight = 1.4
    out.append(weight * math.log(max(plate_current(a1["vp"], a1["vs"], a1["vg"], p), 1e-12) / a1["ip"]))
    for row in FIXED_BIAS:
        model = plate_current(row["va"], row["vs"], row["vg"], p)
        out.append(weight * math.log(max(model, 1e-12) / row["ip0"]))

    dv = 1e-3
    gm = (
        plate_current(a1["vp"], a1["vs"], a1["vg"] + dv, p)
        - plate_current(a1["vp"], a1["vs"], a1["vg"] - dv, p)
    ) / (2 * dv)
    rp = 1.0 / max(
        (
            plate_current(a1["vp"] + dv, a1["vs"], a1["vg"], p)
            - plate_current(a1["vp"] - dv, a1["vs"], a1["vg"], p)
        )
        / (2 * dv),
        1e-12,
    )
    out.append(1.5 * weight * math.log(max(gm, 1e-12) / a1["gm"]))
    out.append(1.5 * weight * math.log(max(rp, 1e-12) / a1["rp"]))
    return np.array(out)


def load_p06_shape() -> list[tuple[float, float]]:
    """Muestras de la curva Ig2 de p06 (sin Vg1): solo aportan la forma vs Vp."""

    samples: list[tuple[float, float]] = []
    with DIGITIZED.open(encoding="utf-8") as handle:
        for row in csv.DictReader(handle):
            if row["figure"] == "p06" and row["ig2_ma"]:
                samples.append((float(row["vp_v"]), float(row["ig2_ma"]) * 1e-3))
    return samples


def screen_residuals(u: np.ndarray, plate: np.ndarray) -> np.ndarray:
    ratio = np.exp(u)
    out: list[float] = []
    with DIGITIZED.open(encoding="utf-8") as handle:
        for row in csv.DictReader(handle):
            if row["figure"] == "p07" and row["ig2_ma"]:
                model = screen_current(
                    float(row["vp_v"]),
                    float(row["ec2_v"]),
                    float(row["vg1_v"]),
                    plate,
                    ratio,
                )
                target = float(row["ig2_ma"]) * 1e-3
                out.append(1.0 * math.log(max(model, 1e-12) / max(target, 1e-12)))

    # Forma Ig2(Vp) de la curva sin etiqueta de p06: relación normalizada al
    # valor de meseta (>330 V); no depende de Vg1.
    shape = load_p06_shape()
    plateau = [ig2 for vp, ig2 in shape if vp >= 330.0]
    if plateau:
        reference = float(np.mean(plateau))
        model_ref = ratio[0] + ratio[1] * math.exp(-345.0 / ratio[2])
        for vp, ig2 in shape:
            model = ratio[0] + ratio[1] * math.exp(-vp / ratio[2])
            out.append(1.5 * math.log(model / max(model_ref, 1e-12) / (ig2 / reference)))

    a1 = CLASS_A1
    out.append(
        1.2
        * math.log(
            max(screen_current(a1["vp"], a1["vs"], a1["vg"], plate, ratio), 1e-12) / a1["ig2"]
        )
    )
    for row in FIXED_BIAS:
        zero = screen_current(row["va"], row["vs"], row["vg"], plate, ratio)
        out.append(1.5 * math.log(max(zero, 1e-12) / row["ig2_0"]))
    # `Max.-Signal Grid-No.2 Current` de la tabla es un valor medio del ciclo
    # de señal, no un punto (Vp, Vg1): no entra en el ajuste puntual.
    return np.array(out)


def report_plate(p: np.ndarray) -> None:
    p05, p07 = load_digitized()
    print("  error relativo por fuente (p05/p07):")
    for tag, data in (("p05", p05), ("p07", p07)):
        errors = []
        for vp, vs, vg, ip in data:
            model = plate_current(vp, vs, vg, p)
            errors.append(abs(model - ip) / max(ip, 1e-12))
        errors = np.array(errors)
        print(
            f"    {tag}: n={len(errors)} mediana {np.median(errors)*100:.1f}% "
            f"p90 {np.percentile(errors, 90)*100:.1f}% max {errors.max()*100:.1f}%"
        )
    a1 = CLASS_A1
    gm = (
        plate_current(a1["vp"], a1["vs"], a1["vg"] + 1e-3, p)
        - plate_current(a1["vp"], a1["vs"], a1["vg"] - 1e-3, p)
    ) / 2e-3
    rp = 1.0 / (
        (
            plate_current(a1["vp"] + 1e-3, a1["vs"], a1["vg"], p)
            - plate_current(a1["vp"] - 1e-3, a1["vs"], a1["vg"], p)
        )
        / 2e-3
    )
    print("  anclas tabuladas:")
    print(
        f"    clase A1 Ip {plate_current(a1['vp'], a1['vs'], a1['vg'], p)*1e3:7.2f} mA"
        f" (objetivo {a1['ip']*1e3:.2f})"
    )
    print(f"    gm {gm*1e6:7.0f} umhos (objetivo {a1['gm']*1e6:.0f})")
    print(f"    rp {rp/1e3:7.2f} kOhm (objetivo {a1['rp']/1e3:.2f})")
    for row in FIXED_BIAS:
        model = plate_current(row["va"], row["vs"], row["vg"], p)
        print(
            f"    bias fija ({row['va']:.0f}/{row['vs']:.0f}/{row['vg']:.0f})"
            f" Ip0 {model*1e3:7.2f} mA (objetivo {row['ip0']*1e3:.1f})"
        )


def report_screen(p: np.ndarray, ratio: np.ndarray) -> None:
    print("  Ig2 contra tablas:")
    a1 = CLASS_A1
    model = screen_current(a1["vp"], a1["vs"], a1["vg"], p, ratio)
    print(f"    clase A1 Ig2 {model*1e3:6.2f} mA (objetivo {a1['ig2']*1e3:.2f})")
    for row in FIXED_BIAS:
        zero = screen_current(row["va"], row["vs"], row["vg"], p, ratio)
        peak = screen_current(VKNEE_ESTIMATE_V, row["vs"], 0.0, p, ratio)
        print(
            f"    bias fija ({row['va']:.0f}/{row['vs']:.0f}/{row['vg']:.0f})"
            f" Ig2_0 {zero*1e3:6.2f} (obj {row['ig2_0']*1e3:.1f})"
            f" | pico puntual en rodilla {peak*1e3:6.2f}"
            f" (la tabla da {row['ig2_max']*1e3:.1f} como media de ciclo)"
        )
    print(
        f"    division: floor {ratio[0]:.4f}, boost {ratio[1]:.4f}, escala {ratio[2]:.1f} V"
    )
    print("  Ig2 contra la curva digitalizada p07 (Vp=300, Vs=300):")
    errors = []
    with DIGITIZED.open(encoding="utf-8") as handle:
        for row in csv.DictReader(handle):
            if row["figure"] == "p07" and row["ig2_ma"]:
                target = float(row["ig2_ma"]) * 1e-3
                model = screen_current(
                    float(row["vp_v"]), float(row["ec2_v"]), float(row["vg1_v"]), p, ratio
                )
                errors.append(abs(model - target) / target)
                print(
                    f"    Vg1 {float(row['vg1_v']):+5.1f}: modelo {model*1e3:6.2f} mA"
                    f" vs {target*1e3:6.2f} mA"
                )
    errors = np.array(errors)
    print(f"    p07 Ig2: mediana {np.median(errors)*100:.1f}% max {errors.max()*100:.1f}%")


def calibrate() -> tuple[np.ndarray, np.ndarray]:
    # Cota de mu: el reposo documentado del B-18N (pantalla ~500 V, bias -55 V,
    # JSON histórico) exige Vs/mu > 55 -> mu < 9.09 para conservar conduccion;
    # la tabla de bias fija (400 V, -38 V) exige ademas mu < 10.5.
    lower = np.log([3.0, 0.8, 10.0, 10.0, 5.0])
    upper = np.log([9.0, 2.5, 1.0e5, 2.0e3, 2.0e3])
    best = None
    for start in (
        [8.7, 1.35, 1460.0, 48.0, 12.0],
        [8.9, 1.5, 800.0, 60.0, 30.0],
        [8.5, 1.2, 2000.0, 40.0, 8.0],
    ):
        result = least_squares(
            plate_residuals, np.log(start), bounds=(lower, upper), max_nfev=40000
        )
        cost = float(np.linalg.norm(plate_residuals(result.x)))
        if best is None or cost < best[0]:
            best = (cost, result.x)
    plate = np.exp(best[1])
    print(f"etapa 1 (placa): coste {best[0]:.3f}")

    lower2 = np.log([0.01, 1e-4, 5.0])
    upper2 = np.log([0.20, 0.50, 400.0])
    best = None
    for start in (
        [0.05, 0.02, 60.0],
        [0.06, 0.08, 150.0],
        [0.04, 0.30, 20.0],
    ):
        result = least_squares(
            lambda u: screen_residuals(u, plate),
            np.log(start),
            bounds=(lower2, upper2),
            max_nfev=40000,
        )
        cost = float(np.linalg.norm(screen_residuals(result.x, plate)))
        if best is None or cost < best[0]:
            best = (cost, result.x)
    print(f"etapa 2 (pantalla): coste {best[0]:.3f}")
    return plate, np.exp(best[1])


def main() -> None:
    p, ratio = calibrate()
    print("\n7027A (mu, ex, kg1, kp, kvb):")
    print("  " + ", ".join(f"{v:.6g}" for v in p))
    print("  division de pantalla (floor, boost, volts): " + ", ".join(f"{v:.6g}" for v in ratio))
    report_plate(p)
    report_screen(p, ratio)

    entries = {
        "tube_models.7027a.koren.mu": (round(float(p[0]), 6), "1", "datasheet", METHOD_FIT),
        "tube_models.7027a.koren.ex": (round(float(p[1]), 6), "1", "datasheet", METHOD_FIT),
        "tube_models.7027a.koren.kg1": (round(float(p[2]), 6), "1", "datasheet", METHOD_FIT),
        "tube_models.7027a.koren.kp": (round(float(p[3]), 6), "1", "datasheet", METHOD_FIT),
        "tube_models.7027a.koren.kvb": (round(float(p[4]), 6), "V", "datasheet", METHOD_FIT),
        "tube_models.7027a.koren.screen_ratio_floor": (
            round(float(ratio[0]), 6),
            "1",
            "estimated",
            METHOD_KNEE,
        ),
        "tube_models.7027a.koren.screen_ratio_boost": (
            round(float(ratio[1]), 6),
            "1",
            "estimated",
            METHOD_KNEE,
        ),
        "tube_models.7027a.koren.screen_ratio_volts": (
            round(float(ratio[2]), 6),
            "V",
            "estimated",
            METHOD_KNEE,
        ),
    }
    fragment = {
        key: {
            "value": value,
            "unit": unit,
            "origin": origin,
            "method": method,
            "date": DATE,
            "source": SOURCE,
        }
        for key, (value, unit, origin, method) in entries.items()
    }
    print("\nFragmento para el overlay de calibracion:")
    print(json.dumps(fragment, indent=2, ensure_ascii=False))


if __name__ == "__main__":
    main()
