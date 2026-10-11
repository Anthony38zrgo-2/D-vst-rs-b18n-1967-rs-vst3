# Modelos históricos

## `B18N_1967_simulation_parameters.json` — CANÓNICO

- **Origen:** parámetros y referencias del Ampeg B-18N/B-15ND, revisión de esquema `1967-05`.
- **Fecha de incorporación:** 2026-10-08.
- **SHA-256:** `d61d122d232a896b4269dd8769d54f6178540ed3b5afd655c9b403966d2a6a0b`
- **Regla:** no editar claves ni valores de este archivo. Cualquier ajuste, inferencia o
  calibración va en `b18n_1967_calibration.example.json` (o en una copia propia del mismo
  formato) y debe indicar unidades, origen, método y fecha.
- **Limitación:** este JSON **no es una netlist**. Los arrays no codifican conexiones; las
  conexiones deben trazarse desde el esquema 5/67 y el manual de servicio
  (véase `docs/CIRCUIT_TRACEABILITY.md`).
- Los campos `null` significan «sin caracterizar», no cero, ideal ni validado.

## `b18n_1967_historical_errata.json` — errata documentada

Correcciones del JSON inicial con evidencia primaria (esquema 5/67, manual,
datasheet), enlazadas por SHA-256 al canónico. Se aplican **antes** de construir
`CircuitDefinition` y no modifican el archivo original:

- **270 kΩ/470 kΩ de la 7199 intercambiados:** `pentode_plate_resistor_ohms`
  corregido a 270 kΩ (pin 2, placa) y `pentode_screen_feed_resistor_ohms` a
  470 kΩ (pin 3, pantalla). Evidencia y nodos/carriles en el propio archivo.
- **Adición trazada:** la **150 kΩ** de pantalla (pin 3) a masa, que el JSON no
  listaba, entra como `phase_inverter.additional_traced_resistors` con nodos y
  evidencia. Su valor coincide con el golden SPICE
  (`assets/fixtures/reference/phase_7199/`).

La carga efectiva (`load_reference`) es: errata -> validación -> mapeo ->
overlay de calibración. El reporte registra `sha256` (canónico),
`errata_sha256` y `effective_sha256`.

## `b18n_1967_calibration.json` — overlay activo

Parámetros **no históricos** que el motor necesita y que el JSON canónico no
contiene, todos etiquetados (`estimated`/`measured`/`datasheet`) con unidad,
método y fecha:

- Modelo Koren del **6SL7** (`tube_models.6sl7.koren.*`), ajustado por
  `scripts/fit_triode_model.py` a las anclas del JSON (tensiones típicas de los
  dos triodos y referencia de pequeña señal a 250 V).
- Modelo Koren del **7027A** (`tube_models.7027a.koren.*`), parámetros
  publicados de la 6L6GC (tubo padre de la 7027A) con su procedencia.
- **Conducción de rejilla** aproximada de ambos tubos (`tube_models.*.grid.*`).
- **Alimentación del previo** (`tube_models.preamp_supply_v`), derivada de las
  tensiones típicas y las resistencias de placa (376–385 V → 380 V).

`load_reference()` aplica este overlay tras validar el JSON canónico. Ningún
valor de este archivo modifica nominales de componentes.

## `b18n_1967_calibration.example.json`

Plantilla vacía que documenta la estructura (`target_revision`, `assumptions`,
`measured`, `provenance`). No se carga en el motor.
