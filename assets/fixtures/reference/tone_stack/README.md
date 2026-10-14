# Referencia de la red de tono

- **Archivos:**
  - `tone_stack_numpy.csv` — 775 filas (25 combinaciones × 31 frecuencias),
    solver numpy independiente (`scripts/tone_stack_reference.py`).
  - `tone_stack_spice.csv` — mismas 775 filas, **ngspice 47** sobre el mismo
    netlist (`scripts/spice/tone_stack_sweep.py`; netlist legible en
    `scripts/spice/b18n_tone_1967.cir`).
  - `tone_stack_coupled_spice.csv` — 775 filas del **circuito acoplado**
    (B18-003): fuente AC de 1 V detrás de 100 kΩ, capacitor de acoplo de
    0.022 µF y la misma red con carga de 270 kΩ; `H = v(out)/v(src)`
    (`scripts/spice/tone_stack_coupled_sweep.py`, netlist legible en
    `scripts/spice/b18n_tone_coupled_1967.cir`).
- **Condiciones (aislado):** 25 combinaciones Bass/Treble en 0/25/50/75/100 %,
  rejilla ISO de tercios de octava de 20 Hz a 20 kHz, fuente Norton de 1 A con
  100 kΩ a masa, carga 270 kΩ (resistor de mezcla).
- **Columnas:** `bass,treble,freq_hz,mag_db,phase_deg`.
- **Uso:** los tests `crates/b18n-dsp/tests/tone_stack_response.rs` exigen
  coincidencia de 1e-6 dB y 1e-4° contra numpy y contra SPICE (aislado), y
  1e-6 dB / 1e-4° contra el golden acoplado usando
  `ToneStackDesign::frequency_response_with_input_coupling`.
- **Regenerar:** `python scripts/tone_stack_reference.py`,
  `python scripts/spice/tone_stack_sweep.py` y
  `python scripts/spice/tone_stack_coupled_sweep.py`.

## Alcance de esta referencia

Verifica que el solver MNA de Rust y el netlist implementado coinciden con dos
implementaciones independientes (numpy y ngspice). El contraste SPICE–numpy dio
un error máximo de 1e-9 dB y 0° (precisión de impresión), lo que confirma que
ambas resuelven el mismo sistema. Aun así, **no** valida por sí sola que la
topología sea la del hardware: eso exige el cotejo del raster ya documentado en
`docs/CIRCUIT_TRACEABILITY.md` y, en última instancia, medición de una unidad.

## Comparación con especificaciones publicadas (revisión 1966 de la reedición)

Rangos medidos en esta implementación (relativos a mandos al 50 %):

| Mando | Boost | Cut |
|---|---|---|
| Bass @ 40 Hz | +8.3 dB | −16.1 dB |
| Treble @ 5 kHz | +10.8 dB | −31.6 dB |

El recorte de agudos coincide con el valor publicado (−30 dB @ 5 kHz). El boost
de agudos y el rango de graves difieren de los valores de la reedición
(+18/−30 dB y ±11 dB); la revisión 5/67 y las condiciones de medida pueden
explicar la diferencia. Queda **pendiente de contraste con SPICE/medición**.
