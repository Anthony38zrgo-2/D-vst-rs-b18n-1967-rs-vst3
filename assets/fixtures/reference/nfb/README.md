# B18-009 — referencias del candidato, integración bloqueada

Generadores: `scripts/spice/nfb_coupled_reference.py` (AC/DC/netlists) y
`scripts/spice/nfb_loop_margins.py` (retorno/transitorio), ngspice 47.

- `coupled_ac.csv`: 24 puntos, 93,75–18.000 Hz, nominal/RLC × lazo abierto/cerrado.
- `coupled_dc.csv`: once tensiones de nodo en cuatro combinaciones.
- `loop_margins.json`: cruces de retorno y respuesta a pulso de 10 µV.
- `*.cir`: netlists generados, conservarlos reproducibles, no editar a mano.

Rieles fijos 380/460/505/500/−55 V; Rth de mezcla 135 kΩ; capacitancias RCA;
parámetros estimados del OT y RLC; toma de NFB ideal sin carga de secundario.
La referencia **falla estabilidad**: margen HF −55,38° nominal / −9,54° RLC.
Las amplitudes grandes del transitorio son diagnósticas (sin conducción de
rejilla en los subcircuitos SPICE), no mediciones de hardware.

La fase del CSV usa grados; la tasa de muestreo no interviene en este AC
continuo. El test Rust declara por separado su tasa efectiva y los límites
de discretización. Manifiesto/SHA-256 en `../manifest.json`.

Estado, pruebas fallidas y siguiente trabajo: `docs/B18_009_STATUS.md`.
