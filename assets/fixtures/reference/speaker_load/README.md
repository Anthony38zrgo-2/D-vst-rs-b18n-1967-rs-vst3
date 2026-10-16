# OT-471 + carga RLC del altavoz (B18-015): referencia SPICE

`speaker_load_spice.csv` es la respuesta compleja `vload/vs` del **terminal del
altavoz**: la red diferencial del OT-471 (parámetros `estimated` de B18-013)
con la carga RLC estimada del altavoz de 18" del overlay `speaker.reactive_load.*`
(B18-015):

| Parámetro | Valor | Criterio (véase `docs/MODEL_ASSUMPTIONS.md`) |
|---|---|---|
| `Re` (resistencia DC de bobina) | 6.5 Ω | ≈0,8× la carga nominal de 8 Ω (JSON) |
| `Le` (inductancia de bobina) | 0.8 mH | típica declarada de un 18"; esquina ≈1,3 kHz |
| `Res` (rama de movimiento) | 23.5 Ω | pico declarado \|Z(fs)\| = 30 Ω |
| `Les` (rama de movimiento) | 69.3 mH | fs = 45 Hz con Q = 1,2 |
| `Ces` (rama de movimiento) | 180.6 µF | coherente con fs = 45 Hz |
| Fuente Thevenin `Rs` | 45 kΩ | 2·rp de la 7027A (datasheet RCA, clase A1) |
| OT (B18-013) | Lm 28.7 H, Lleak 12 mH, Cp 100 pF, R_core 500 Ω, DCR 300/0.4 Ω, N8 23.18 | overlay `estimated` |

Condiciones: ngspice 47 sobre `scripts/spice/b18n_speaker_load.cir` (AC, `dec 20`,
5 Hz–200 kHz). El generador `scripts/spice/speaker_load_reference.py` comprueba
el netlist contra una implementación analítica independiente (desviación
relativa < 1e-3) antes de escribir el CSV. Fase en grados, referida a la fuente
(coseno).

**Espera:** los parámetros son `estimated` (no hay medición del Cleveland
concreto; `cabinet.speaker_impedance_curve` sigue `null`). Al disponer de una
curva Z(f) medida del altavoz, regenerar este golden con ella y actualizar
`LumpedSpeakerParameters`.

Contraste con el motor: `crates/b18n-dsp/tests/reactive_load.rs` mide el
modelo (TPT en toda la red de carga; Euler atrás declarado solo en el flujo
del OT) a la tasa de proceso 4× de 48 kHz. Contraste en banda audible:
< 0,5 dB / < 10° (imprimir la desviación exacta con
`cargo test -p b18n-dsp --test reactive_load -- --nocapture
magnitude_and_phase_match_spice_in_the_audio_band`).
