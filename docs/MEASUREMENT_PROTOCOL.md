# Protocolo de medición y comparación

## Principio

Una referencia generada por el propio DSP no valida fidelidad física. Toda comparación
clave proviene de SPICE independiente, datasheets trazables o mediciones propias.

## Puntos de control (de `validation.validate_at` y guía §8)

1. **Reposo DC:** nodos de `preamp`, `phase_inverter`, `power_stage` contra el JSON. Simular
   la carga del voltímetro (20 000 Ω/V en el rango usado) en nodos de alta impedancia.
2. **Respuesta en frecuencia:** barrido 20 Hz–20 kHz a nivel bajo; Bass/Treble en
   0/25/50/75/100 % (25 combinaciones) contra SPICE lineal del mismo netlist.
3. **Distorsión:** THD y espectros H2–H10 vs. frecuencia y amplitud; ambos semiciclos.
4. **Potencia:** 50 W en 8 Ω equivalen a **20 V RMS** en la carga; no usar como umbral
   digital sin mapa voltios↔escala.
5. **Transitorios graves:** trenes de 40–120 Hz para B+, bias y recuperación.
6. **NFB/estabilidad:** impulsos y cargas 8 Ω/16 Ω (resisitivas y reactivas); detectar
   oscilación o ganancia insuficiente de margen.
7. **Alias:** tonos altos y oversampling 1×/4× (8× no implementado); medir componentes en banda base.
8. **Cabinet:** IR off ⇒ solo etapa eléctrica; IR on ⇒ reproducir un ensayo conocido.
9. **Determinismo:** misma entrada/configuración ⇒ misma salida (± tolerancia FP) con
   bloques de 1, 7, 64, 257 y 1024 muestras.
10. **Robustez:** silencio, DC, amplitudes altas, controles extremos, cambios de sample rate,
    procesos largos, detección de valores no finitos.

## SPICE de referencia

- El netlist SPICE se versiona en `scripts/spice/` con sus golden en
  `assets/fixtures/reference/`; debe ser **independiente** del código Rust.
- **ngspice 47** está instalado localmente (`tools/ngspice/`, ignorado por git)
  y se ejecuta con `python scripts/spice/run_spice.py <netlist>`.
- Cada resultado golden indica: netlist, versión de SPICE, tolerancias y fecha.
- **Tone stack:** `scripts/spice/b18n_tone_1967.cir` (referencia legible) y
  `scripts/spice/tone_stack_sweep.py` (barrido 25×31) generan
  `assets/fixtures/reference/tone_stack/tone_stack_spice.csv`; el test
  `crates/b18n-dsp/tests/tone_stack_response.rs` compara Rust contra numpy y
  contra SPICE con tolerancia de 1e-6 dB / 1e-4°.
- **Reposo de la 7199 (pantalla):** `scripts/spice/b18n_7199_dc_1967.cir`
  con golden en `assets/fixtures/reference/phase_7199/`; `R(pin3, masa)`
  = 150 kΩ (equipo apagado) y 113.7 kΩ con el carril referido a masa.

## Reportes

Cada reporte incluye: `sha256` del JSON, versión de Rust, versión del algoritmo,
configuración completa y fecha. Los resultados con datos faltantes se marcan explícitamente;
no se completa con supuestos silenciosos.
