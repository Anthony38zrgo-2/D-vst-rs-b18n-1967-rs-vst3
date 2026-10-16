# OT-471 lineal (B18-013): referencia SPICE

`ot471_linear_spice.csv` es la respuesta compleja `vsec/vs` del equivalente
diferencial del transformador de salida OT-471 con los parámetros
`estimated` del overlay de calibración:

| Parámetro | Valor | Criterio (véase `docs/MODEL_ASSUMPTIONS.md`) |
|---|---|---|
| `Lm` (magnetización, placa-placa) | 28.7 H | esquina de baja frecuencia a 25 Hz con la carga reflejada |
| `Lleak` (fuga, referida al primario) | 12 mH | esquina de agudos ≈64 kHz |
| `Cp` (parásita placa-placa) | 100 pF | resonancia con `Lleak` ≈145 kHz |
| `R_core` (pérdidas de flujo) | 500 Ω | atenúa < 0,1 dB en la esquina; relaja el flujo (τ ≈ 57 ms) |
| Fuente Thevenin `Rs` | 45 kΩ | 2·rp de la 7027A (datasheet RCA, clase A1) |
| Resto | N8=23.18, DCR 300/0.4 Ω, carga 8 Ω | JSON histórico |

Condiciones: ngspice 47 sobre `scripts/spice/b18n_ot471_linear.cir` (AC,
`dec 20`, 5 Hz–200 kHz). El generador
`scripts/spice/output_transformer_reference.py` comprueba el netlist contra
una implementación analítica independiente (desviación relativa < 1e-3)
antes de escribir el CSV. Fase en grados, referida a la fuente (coseno).

Contraste con el motor: `crates/b18n-dsp/tests/output_transformer_ac.rs`
mide el modelo (Euler atrás declarado en los inductores, TPT en el
condensador) a la tasa de proceso 4× de 48 kHz. Medido: peor desviación en la
banda audible **0,09 dB / 0,97°** (≪ 0,5 dB / 10° exigidos).
