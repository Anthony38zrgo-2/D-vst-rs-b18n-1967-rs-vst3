# Red de volumen del canal (B18-005)

Referencia SPICE de la red trazada del raster `SCH-567` (confirmada por la
tabla de resistencias del manual, V1/pin 1: 0 Ω a 1 MΩ según la posición):

- Placa del 6SL7#1 (`Vsrc`) → `Ro = 227.7 kΩ` → acoplo de **0.022 µF** →
  potenciómetro de **1 MΩ log** (Rtop–cursor–Rbottom a masa) → rejilla del
  6SL7#2. Sin fuga de rejilla separada: el propio potenciómetro es la carga.
- Ley del mando: `f(p) = (10^(2p) − 1)/99` (10 % de resistencia a mitad de
  recorrido); `Rbottom = f(p)·1 MΩ`, `Rtop = (1−f(p))·1 MΩ`.

- **Archivo:** `volume_network_spice.csv` — posiciones 0.25/0.5/0.75/1.0 ×
  100 Hz/1 kHz/10 kHz. Columnas:
  `position,fraction,ro_ohms,coupling_farads,pot_ohms,freq_hz,mag_db,phase_deg`,
  con `H = v(cursor)/v(src)` y la rejilla en circuito abierto (la corriente de
  rejilla es despreciable en pequeña señal).
- **Simulador:** ngspice 47 (`scripts/spice/volume_network_sweep.py`; netlist
  legible en `scripts/spice/b18n_volume_network_1967.cir`). El piso de 1 mΩ en
  Rtop/Rbottom evita el estampado singular de ngspice con resistencias de 0 Ω.
- **Uso:** `crates/b18n-dsp/tests/volume_network.rs` mide la transferencia del
  solve temporal del 6SL7#2 a 192 kHz y la compara con el golden a 0.02 dB /
  0.2°; además verifica monotonía, mínimo en cursor a masa y ausencia de pops
  tras `reset`.
- **Regenerar:** `python scripts/spice/volume_network_sweep.py`.
