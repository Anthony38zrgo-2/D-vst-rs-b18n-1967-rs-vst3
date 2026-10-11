# B-18N Portaflex 1967 — modelo de circuito DSP (Rust)

Emulación por etapas del circuito del **Ampeg B-18N/B-15ND, revisión 5/67**, en Rust,
con un núcleo DSP independiente del host y exportación posterior a VST3.

> Proyecto no oficial. No está afiliado ni respaldado por Ampeg/Yamaha.
> «Ampeg», «Portaflex» y «B-18N» se mencionan solo con fines descriptivos e históricos.

## Estado

- **Hito A — repositorio y modelo de datos:** completado.
- **Hito B — camino lineal verificable:** completado (entrada, volumen, red de tono con
  MNA/TPT, mezcla, inversor ideal, OT-471 ideal con DCR y 8 Ω).
- **Hito C — no linealidad a válvulas:** implementado para el 6SL7 (Koren calibrado,
  Newton amortiguado, conducción de rejilla, reposo resuelto al inicializar) y para la
  **etapa de potencia 2 × 7027A** (placa + pantalla, bias fija con acoplo que bloquea
  continua, OT ideal con toma central y DCR, 8 Ω) con sobremuestreo 4× y latencia
  reportada. El pentodo 7199 queda pendiente de resolver sus medidas DC (documentado).
- **P1.4–P1.6:** inversor 7199 real, tone stack contra ngspice y NFB con memoria de
  capacitores; latencia del sobremuestreo 4× corregida a **47 muestras** midiendo el
  impulso (el valor anterior, 93, duplicaba el retardo por etapa).
- **P1.7 — plugin VST3:** `crates/b18n-plugin` (NIH-plug, GPL-3.0-only) con layouts
  2→2 y 1→1 (estéreo = dos motores), parámetros con IDs estables, presets con huellas
  del modelo, bypass compensado en latencia y oversampling 1×/4× en panel avanzado.
  Bundle: `cargo xtask bundle b18n-plugin --release` → `target/bundled/B18N 67.vst3`.
  Quedan como pasos manuales el Steinberg Validator y la prueba en DAW.
- **P2 (pendiente):** fuente dinámica (5AR4/sag), OT no ideal, gabinete reactivo/IR.
- Guía de hitos en [`docs/IMPLEMENTATION_GUIDE.md`](docs/IMPLEMENTATION_GUIDE.md).

El motor ya satura previo y etapa de potencia; la fuente todavía es rígida (Hito D).
Los parámetros de modelo viven en `assets/models/b18n_1967_calibration.json`,
etiquetados como estimados/datasheet.
Cualquier dato no respaldado por la documentación histórica debe ir a
`assets/models/b18n_1967_calibration.example.json` y quedar etiquetado.

## Estructura

```text
crates/b18n-config   JSON histórico -> CircuitDefinition tipada (serde)
crates/b18n-dsp      núcleo DSP sin NIH-plug, sin GUI, sin VST3
crates/b18n-plugin   adaptador VST3 (NIH-plug) con parámetros y estado
crates/b18n-render   procesado WAV offline, mediciones y reportes (CLI)
xtask                bundler NIH-plug (Hito E)
assets/models        JSON canónico + calibración separada
assets/fixtures      señales y referencias verificadas (no inventadas)
assets/reference     PDFs históricos locales (no se redistribuyen)
docs                 guía de implementación, trazabilidad, supuestos
```

Dependencias permitidas: `assets/*.json -> b18n-config -> b18n-dsp`; `b18n-render` y
`b18n-plugin` usan `b18n-config + b18n-dsp`. `b18n-dsp` no conoce archivos ni APIs de
host.

## Construcción y pruebas

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --release
cargo xtask bundle b18n-plugin --release
```

Render offline (cadena lineal + reporte JSON; `--measure tone` añade las 25 curvas
Bass/Treble en CSV):

```bash
cargo run -p b18n-render --release -- \
  --input assets/fixtures/signals/bass_test.wav \
  --output target/renders/b18n_output.wav \
  --sample-rate 48000 \
  --oversampling 4 \
  --channel bass \
  --cabinet off \
  --measure tone \
  --report target/reports/b18n_1967.json
```

## Regla de datos

El JSON histórico es la única fuente de parámetros. No se inventan topologías, valores
ni mediciones: todo supuesto va documentado y etiquetado como `estimated`, `measured` o
`datasheet` en el archivo de calibración.

## Licencia

GPL-3.0-only. La distribución de un binario VST3 construido con los bindings `vst3-sys`
de NIH-plug exige cumplir GPLv3 (véase el Hito E en la guía).
