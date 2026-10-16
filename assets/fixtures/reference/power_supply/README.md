# Referencia de la fuente dinámica (B18-012)

Netlist `scripts/spice/b18n_power_supply_1967.cir` (ngspice 47) con la red RC
trazada del esquema 5/67 (zona V6/PT-12XA): rectificador como fuente Thevenin
`Vsrc = 518.935 V + Rrect = 150 Ω` (aproximación RC + media, `estimated`),
`1k/10k/22k`, `30/40/40/40 µF`, bias `-55 V/15k/100 µF`, y cargas de reposo
del modelo (`Ict = 83 mA, Iscr = 3 mA, Itri = 2.3 mA, Ipre = 4.6 mA`).

## Archivos

- `b18n_power_supply_dc_1967_golden.txt`: punto DC (`.op`) con las cargas de
  reposo. Generado por `scripts/spice/power_supply_reference.py`.
- `b18n_power_supply_tran_1967_golden.csv`: transitorio `0.25 ms/200 ms` con
  escalón de `+20 mA` en H2 a `t = 50 ms` (sag ≈ 2.9 V en B+, recuperación
  exponencial con `τ ≈ Rrect·Ctotal ≈ 22 ms`). Mismo generador.

## Condiciones y tolerancias

- Topología Hipótesis A (10k → triodo, 22k → previo), `estimated`.
- Contraste Rust en `crates/b18n-dsp/tests/supply_dynamics.rs`: DC ±2 V
  (misma red resistiva, distinto integrador), sag ±1 V y recuperación
  cualitativa (Backward Euler vs trapezoidal de ngspice).
- No incluye curva 5AR4 ni DCR del PT (unknowns del JSON); no es una
  caracterización del transformador real.
