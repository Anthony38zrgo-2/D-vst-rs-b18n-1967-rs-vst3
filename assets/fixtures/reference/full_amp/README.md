# Fixtures de amplificador completo (B18-001)

Directorio reservado para referencias de **cadena completa** (entrada → 6SL7 → tono →
7199 → 7027A → OT-471 → carga), a diferencia de los goldens por etapa que viven en
`tone_stack/` y `phase_7199/`.

## Estado actual

**Vacío de fixtures**: todavía no existe ninguna medición de hardware ni simulación SPICE
de la cadena completa versionada. Los únicos goldens disponibles son por etapa. Cualquier
archivo que se añada aquí debe cumplir las reglas siguientes.

## Reglas de procedencia (B18-021)

Cada fixture registra su clase de procedencia en
`assets/fixtures/reference/manifest.json` (verificado por
`scripts/check_reference_manifest.py`):

| Clase | Significado | Ejemplo |
|---|---|---|
| `historical-manual` | Lectura publicada en manual/boletín histórico | Reposo pin 9 cargado por el voltímetro |
| `manufacturer-datasheet` | Curva o valor literal de hoja de fabricante | Curva RCA 7027-A |
| `independent-SPICE` | Netlist versionado ejecutado con ngspice, independiente del DSP | `tone_stack_spice.csv` |
| `real-hardware` | Medición sobre una unidad física, con condiciones | Impedancia `Z(f)` del 8 Ω |
| `model-only` | Salida del propio modelo/herramienta del proyecto | `tone_stack_numpy.csv` |

Reglas adicionales:

- Un fixture del propio DSP **no** valida fidelidad física (regla de
  `assets/fixtures/reference/README.md`); se etiqueta `model-only`.
- Un ajuste a curvas de datasheet se registra como derivado (`estimated` en calibración),
  no como `manufacturer-datasheet` literal.
- Cada archivo indica condiciones: sample rate, amplitudes en **voltios**, carga,
  posiciones de controles, instrumento de medida simulado si aplica y fecha.
- Los PDFs de terceros no se versionan; solo el golden derivado y su descripción.

## Fixtures planificados (aún no disponibles)

| Ensayo | Categoría | Estado | Ticket que lo consumirá |
|---|---|---|---|
| Respuesta en frecuencia completa (20 Hz–20 kHz, magnitud y fase) | AC | pendiente de netlist SPICE completo | B18-003, B18-008, B18-018 |
| Reposo DC de la cadena completa con instrumento modelado | DC | pendiente | B18-018 |
| THD vs frecuencia y amplitud en la carga | THD | pendiente | B18-010, B18-018 |
| Transitorios de graves (40/60/100 Hz) para B+/bias | Transitorio | pendiente | B18-012 |
| Impedancia `Z(f)` del altavoz de 8 Ω | Carga | pendiente de medición | B18-015 |
| IR del gabinete con metadatos (micrófono, posición, distancia) | Acústica | pendiente de adquisición | B18-016 |

Hasta que existan, el baseline (`docs/FIDELITY_BASELINE.md`) declara la cadena completa
como `topology_unverified` y las aproximaciones de OT/gabinete como hipótesis.
