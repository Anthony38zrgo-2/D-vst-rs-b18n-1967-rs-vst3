# Referencias DC de la 7199 (V3)

Simulador: ngspice 47 (Windows x64, `ngspice_con.exe`, 2026-08-11).
Modelos Koren: ajustados a anclas del boletin RCA 7199 DATA 1/2 (7-63) por
`scripts/fit_7199_model.py` (mismas ecuaciones que `tube_models.rs`).

## `b18n_7199_dc_1967.cir` — R(pin 3, masa)

Fuentes de DC anuladas; 1 A inyectado de masa a `pin3`; `v(pin3)` = R.

| Variante | Condición | R(pin3, masa) | Manual (V3/pin 3) |
|---|---|---:|---:|
| A | Equipo apagado / standby abierto (carril flotante) | **150 000 Ω** | 150 kΩ |
| B | Carril de previo referido a masa (contraste) | 113 710 Ω | — |

Golden: `b18n_7199_dc_1967_golden.txt`.

## `b18n_7199_meter_loading.cir` — reposo completo y carga del medidor

Topología trazada: placa 270 kΩ y pantalla 470 kΩ + 150 kΩ + 0.1 µF (cátodo),
cátodo 1.5 kΩ; acoplo 0.02 µF del pentodo; **10 MΩ de la placa del triodo a su
rejilla** y **3.3 MΩ de rejilla a masa**; triodo de carga dividida 47 kΩ/47 kΩ.
Carriles: pentodo 380 V (nodo de previo); triodo **460 V** (inferido de que la
misma corriente recorre ambas 47 kΩ, luego `Vp+Vk = Vrail`).

| Magnitud | Manual (SM-1967) | Modelo sin medidor | Desvío |
|---|---:|---:|---:|
| pin 2 (placa pentodo) | 105 V | 88.3 V | −16 % |
| pin 3 (pantalla) | 60 V | 66.1 V | +10 % |
| pin 6 (cátodo pentodo) | 1.8 V | 1.96 V | +9 % |
| pin 1 (placa triodo) | 360 V | 356.1 V | −1 % |
| pin 8 (cátodo triodo) | 100 V | 102.7 V | +3 % |
| pin 9 (rejilla triodo, sin carga) | — | 88.3 V | — |

Carga del medidor de 20 000 Ω/V en pin 9 (fuente real ~2.5 MΩ por el divisor
10M/3.3M):

| Rango | R medidor | pin 9 leído | pin 8 | Manual |
|---|---:|---:|---:|---:|
| 50 V | 1 MΩ | 29.0 V | 51.0 V | — |
| 100 V | 2 MΩ | 43.7 V | 63.7 V | — |
| 250 V | 5 MΩ | **62.7 V** | 80.3 V | **65 V** |
| 500 V | 10 MΩ | 73.4 V | 89.6 V | — |

**Conclusión (P0.3):** la lectura de 65 V en pin 9 es la lectura **cargada** por
el instrumento de un nodo de ~88 V con impedancia de fuente ~2.5 MΩ
(10 MΩ || 3.3 MΩ), coherente con un medidor de 20 000 Ω/V en el rango de 250 V.
El reposo del triodo (Vgk ≈ −20 V, Ip ≈ 2.3 mA) explica pin 1 y pin 8 dentro del
3 %. La rejilla del pentodo queda en −2 V por la caída en la 1.5 kΩ; el modelo
sobreestima Ip un 8 % en ese punto de baja pantalla (extrapolación del boletín)
y por eso pin 2 queda ~16 % bajo: se registra como residual del ajuste, sin
retocar valores para forzar la coincidencia.

Golden: `b18n_7199_meter_loading_golden.txt`.

## `b18n_7199_nfb_dc_1967.cir` — reposo con NFB

22 kΩ desde la toma de 16 Ω (secundario a masa en DC) al cátodo del pentodo
(topología del AN-183 y del flag «F» del raster). Reposo simulado: pin 1 =
356,06 V, pin 2 = 85,23 V, pin 3 = 64,63 V, pin 6 = 1,87 V, pin 8 = 102,68 V,
pin 9 = 88,34 V. Es el punto de contraste del modelo con la NFB conectada
(la 22 kΩ sube la corriente catódica del pentodo y pin 6 baja a ≈1,87 V).

## `b18n_7199_ac_1967.cir` — respuesta AC del inversor (con NFB 22 kΩ)

Excitación AC unitaria en la rejilla del pentodo; salidas medidas en la placa
(pin 1) y el cátodo (pin 8) del triodo (desviaciones crudas: el acoplo de
0.05 µF y la memoria de bloqueo viven en la etapa de potencia). Golden:
`b18n_7199_ac_1967_golden.csv` (10 frecuencias).

| Frecuencia | mag placa | fase placa | mag cátodo | fase cátodo |
|---:|---:|---:|---:|---:|
| 20 Hz | 46.701 | +48.1° | 47.173 | −131.9° |
| 100 Hz | 75.554 | +14.3° | 76.317 | −165.7° |
| 1 kHz | 78.357 | +1.47° | 79.149 | −178.5° |
| 20 kHz | 78.387 | +0.07° | 79.179 | −179.9° |

**Contraste Rust–SPICE** (test `phase_inverter_ac.rs`, tras B18-008): magnitud
**≤0,0002 %** y fase **≤0,05°** en 20 Hz–20 kHz (medido; el test exige 0,1 % y
0,2°). El golden incluye el circuito ampliado completo (0.02 µF + 10 MΩ +
3.3 MΩ) y el solve del motor resuelve esa misma red de forma acoplada con el
pentodo por muestra.

## `b18n_7199_tran_1967.cir` — transitorio (1 kHz, 0.2 V, con NFB)

Ventana estable 15–20 ms; golden `b18n_7199_tran_1967_golden.txt` (desviaciones
respecto del reposo con NFB). Extremos SPICE: placa +15.04/−16.17 V, cátodo
+16.34/−15.19 V (polaridades opuestas, asimetría de válvula). El test compara
los cuatro extremos con un 0,5 % de margen (Rust–SPICE; medido 0,03 % tras
B18-008).

## Regenerar

```powershell
python scripts/spice/run_spice.py scripts/spice/b18n_7199_dc_1967.cir
python scripts/spice/run_spice.py scripts/spice/b18n_7199_meter_loading.cir
python scripts/spice/run_spice.py scripts/spice/b18n_7199_nfb_dc_1967.cir
python scripts/spice/phase_inverter_reference.py   # AC (golden CSV)
python scripts/spice/run_spice.py scripts/spice/b18n_7199_tran_1967.cir
```
