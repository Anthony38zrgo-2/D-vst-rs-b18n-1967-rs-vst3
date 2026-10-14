# Redes de jack de entrada (B18-007)

Referencia SPICE de las redes trazadas del raster `SCH-567`:

| Jack | Serie | Carga de rejilla con el jack en uso | Evidencia |
|---|---|---|---|
| `bass` | 120 kΩ | 5.6 MΩ ∥ (120 kΩ + 100 kΩ) = 211.8 kΩ | llave del jack `guitar` libre corta su punta a masa por 100 kΩ; manual V1/pin 4 = 220 kΩ (±10 %) |
| `guitar` | 120 kΩ | 5.6 MΩ | al insertar el plug la llave abre el shunt |
| `instrument` | 0 Ω | 2.2 MΩ | CH2: punta directa a la rejilla de V2/pin 4; llave a masa sin plug |

- **Archivo:** `input_jacks_spice.csv` — 3 jacks × 2 impedancias de fuente
  (0 Ω ideal y 10 kΩ de instrumento pasivo, condición de ensayo) × 31
  frecuencias ISO (20 Hz–20 kHz). Columnas:
  `jack,source_ohms,freq_hz,mag_db,phase_deg`, con `H = v(grid)/v(src)`.
- **Simulador:** ngspice 47 (`scripts/spice/input_jacks_sweep.py`; netlist
  legible en `scripts/spice/b18n_input_jacks_1967.cir`).
- **Uso:** `crates/b18n-dsp/tests/input_jacks.rs` compara el divisor trazado
  (`InputJackNetwork::grid_division`) con el golden a 1e-6 dB y verifica la
  ganancia relativa entre canales en el motor.
- **Discrepancia registrada:** el manual lista 220 kΩ en V2/pin 4 igual que en
  V1/pin 4; el raster muestra un jack conmutado sin resistencia serie en CH2
  (0 Ω con el jack libre). Manda la conexión trazada; la fila del manual se
  conserva como observación.
- **Regenerar:** `python scripts/spice/input_jacks_sweep.py`.
