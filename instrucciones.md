# Instrucciones para continuar el proyecto (handoff)

Documento de traspaso para retomar el trabajo en el port VST3/Rust del Ampeg
B-18N/B-15ND (revisión de esquema `1967-05`). Está escrito para que un agente o
persona nueva pueda construir, verificar y continuar sin contexto previo.

**Regla de oro:** antes de tocar algo relacionado con la 7199, el tone stack, la
NFB o la etapa de potencia, leer `docs/B18N_1967_AGENT_BLOCKERS_RESOLUTION.md`.
Ese archivo contiene la resolución de los bloqueos con fuentes primarias y su
estado de ejecución; este handoff resume y enlaza.

Documentos de referencia, en orden de lectura:

1. `docs/B18N_1967_AGENT_BLOCKERS_RESOLUTION.md` — resoluciones y prioridades P0/P1/P2.
2. `docs/sprint.md` — bitácora completa de hitos e impedimentos (Olas 1–8 al día).
3. `docs/IMPLEMENTATION_GUIDE.md` — guía maestra (orden de hitos y criterios).
4. `docs/CIRCUIT_TRACEABILITY.md` — qué está trazado del esquema y con qué evidencia.
5. `docs/MODEL_ASSUMPTIONS.md` — supuestos y parámetros `estimated`/`datasheet`.
6. `docs/MEASUREMENT_PROTOCOL.md` — cómo se contrasta y se reporta.
7. `docs/DSP_ARCHITECTURE.md` — arquitectura y reglas del callback de audio.

---

## 1. Objetivo

Emular por etapas el circuito del B-18N/B-15ND con un núcleo DSP en Rust
independiente del host y exportarlo como VST3 (NIH-plug). El JSON histórico es
la única fuente de parámetros; todo dato no documentado va al overlay de
calibración o al archivo de errata, con unidad, método, origen y fecha.

## 2. Estado actual (2026-10-11, tras Olas 1–8)

| Hito | Estado |
|---|---|
| A — repositorio y modelo de datos | Completado |
| B — camino lineal verificable (tone stack MNA/TPT) | Completado |
| C1 — triodos 6SL7 no lineales, Newton, reposo, sobremuestreo 4× | Completado |
| C2 — etapa de potencia 2×7027A push-pull con OT y bias fija | Completado |
| P0.1 — errata 270 kΩ/470 kΩ de la 7199 | **Cerrado** (archivo de errata + huella efectiva) |
| P0.2 — 150 kΩ de pin 3 | **Cerrado** (SPICE: R(pin3, masa) = 150 kΩ) |
| P0.3 — reposo 7199 + carga del medidor | **Cerrado** (la lectura de 65 V es carga del instrumento) |
| P1.4 — inversor 7199 real en el motor | **Cerrado** (.op/.ac/.tran contra SPICE) |
| P1.5 — tone stack contra ngspice 47 | **Cerrado** (SPICE–numpy ≤1e-9 dB / 0°) |
| P1.6 — NFB + memoria de capacitores | **Cerrado** (`z^-1` + compensación declarada; estabilidad medida) |
| P1.7 — plugin VST3 | **Cerrado en código** (`crates/b18n-plugin`, bundle generado); pendientes manuales: Steinberg Validator y DAW |
| Fase 1/Ola 8 — presupuesto de CPU del plugin | **Cerrado** (inversor Newton 3D con jacobiano analítico ≈2-6 Koren/muestra, robustez a drive extremo 4 V con 0 fallos, guardia de CPU: ≈23,5 µs/muestra frente a 25 µs, niveles DAW y arranque sin rampa) |
| P2 — fuente dinámica, OT no ideal, gabinete reactivo/IR | Pendiente |

Verificación al cierre del último entregable (Fase 1/Ola 8, CPU del plugin):

- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`, `cargo build --workspace --release`: limpios.
- **140 tests** en verde (config 28, dsp 92, plugin 14 + 1 bench ignorado,
  render 6): paridad sample-exact plugin↔motor, invariancia de bloque,
  latencia con impulso, no-alloc en `process`/`reset`, presupuesto de CPU,
  niveles DAW e invariancia de alias a 4 V.
- Bundle: `cargo xtask bundle b18n-plugin --release` genera
  `target/bundled/B18N 67.vst3`.
- Bench de CPU (ignorado, `cargo test -p b18n-plugin --release -- --ignored
  --nocapture`): **23,5–23,6 µs/muestra** (estéreo 4×, 48 kHz, drive fuerte con
  input +6 dB), bajo la guardia declarada de 25 µs.
- Render E2E: `algorithm_version: 0.1.0-nonlinear-triodes-power-7199-inverter-nfb-25k-comp-4x`,
  latencia de **47 muestras** a 4× (FIR de media banda de 63 taps; sin cambio
  con la tasa del host 44,1–192 kHz), pico 3,084, offset +0,0015; reporte en
  `target/reports/b18n_1967.json`.

Correcciones de la Ola 8 (cambios audibles documentados en `sprint.md` y
`CHANGELOG.md`):

- **Inversor 7199: bisección anidada (~5.500 evaluaciones Koren/muestra) →
  Newton 3D acoplado con jacobiano analítico** (`PentodeCurrents`): ~2-6
  evaluaciones en el camino rápido. Respuesta de robustez declarada: con
  rejilla positiva extrema (la placa sale de rango por la zona `Vp ≤ 0` del
  modelo) el Newton estanca; respaldo con bisección monótona de la corriente
  en el bracket físico de la red de placa `[0, A]` y Newton de reinicio
  (objetivo: 0 fallos de congelación a 4 V de drive, medido).
- **Triodos 6SL7: Newton 24 → 64 iteraciones máximas.** El arrastre
  amortiguado cerca del corte necesitaba ~26-40 iteraciones antes de la fase
  cuadrática y congelaba samples con drive extremo. El camino rápido sigue
  saliendo en 2-6 (criterio de paso <1e-6).
- **NFB compensación: 20 kHz → 2,5 kHz de la tasa base.** Con el solver
  exacto del inversor el lazo idealizado era inestable a Nyquist (barrera
  estable/inestable 4–5 kHz efectivos a 44,1/48 kHz); además el corte
  anterior se construía con la tasa de proceso, de modo que variaba con el
  sobremuestreo. Cambio audible: `ALGORITHM_VERSION` actualizada.
- **Nivel de instancia por defecto:** con senos a escala completa la salida
  stock producía pico 5,57 (rebasa la escala del host). El trim de salida por
  defecto pasa a **−16,5 dB** (no histórico; la instancia nueva queda ≈0,83,
  −1,6 dBFS) y la primera configuración de mandos hace snap directo (sin
  rampa de 15 ms a ganancia previa al arranque; `reset` mantiene los mandos
  del usuario sin rampa).
- **Latencia 4× sin cambios: 47 muestras** (impulso stock pica en 46, dentro
  de ±1). Con drive moderno (+6 dB) el ápice del transitorio resuelto se
  desplaza a 49: no es cambio de latencia FIR, es forma del transitorio no
  lineal; test re-anclado (adaptador ≡ motor directo sample-exact).
- **Sonda temporal eliminada** (`probe_failures.rs`, se usó para diagnosticar
  el estancamiento del Newton del inversor).

Correcciones de la Ola 7 que siguen vigentes (detalle en `docs/sprint.md`
§16.2): la latencia 4× se corrigió de 93 a **47 muestras** midiendo el impulso
(pico 47, correlación 47, centro de masas 46,28 con el sobremuestreador puro;
el cálculo anterior duplicaba el retardo por etapa) y el solver Newton junto
con `reset()` quedaron sin asignaciones dinámicas (regla del callback).

Residuales documentados (no bloquean, no retocar sin evidencia):

- El pentodo 7199 simulado queda ~19 % por debajo del manual en pin 2 (el ajuste
  del boletín sobreestima `Ip` en pantalla baja; véase `MODEL_ASSUMPTIONS`).
- El alias de 4× con la cadena real está dominado por intermodulación con las
  imágenes del interpolador; el FIR se subió a 63 taps y la mejora queda
  documentada como pendiente de un interpolador mayor.
- No hay conducción de rejilla del pentodo 7199 en el solve del inversor
  (la 270 kΩ del mezclador no carga la rejilla con corriente de rejilla
  modelada; queda documentado como simplificación y solo importa a drive
  extrema de entrada).

## 3. Cómo construir, testear y ejecutar

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --release
cargo xtask bundle b18n-plugin --release   # target/bundled/B18N 67.vst3
```

Render offline con medición de tono (25 curvas Bass/Treble):

```bash
cargo run -p b18n-render --release -- \
  --input assets/fixtures/signals/bass_test.wav \
  --output target/renders/b18n_output.wav \
  --sample-rate 48000 --channel bass --cabinet off \
  --measure tone --report target/reports/b18n_1967.json
```

SPICE (ngspice 47 ya instalado en `tools/ngspice/`, ignorado por git):

```bash
python scripts/spice/run_spice.py scripts/spice/b18n_7199_nfb_dc_1967.cir
python scripts/spice/phase_inverter_reference.py     # regenera el golden AC
python scripts/spice/tone_stack_sweep.py             # regenera el golden del tone stack
```

Scripts auxiliares (Python 3.14; requieren `pypdf`, `pypdfium2`, `Pillow`,
`numpy` y `scipy`):

- `scripts/extract_pdfs.py` — extrae imágenes/texto de los PDF históricos.
- `scripts/trace_schematic.py` / `scripts/ascii_schematic.py` — segmentos y mapas
  ASCII del esquema raster para trazado de nodos.
- `scripts/fit_triode_model.py` / `scripts/fit_7199_model.py` — ajustes Koren
  del 6SL7 y de la 7199 (pentodo y triodo) contra anclas documentadas.
- `scripts/tone_stack_reference.py` — referencia numpy del tone stack (CSV versionado).
- `scripts/spice/run_spice.py`, `tone_stack_sweep.py`, `phase_inverter_reference.py`
  — netlists y goldens de referencia independiente.
- `scripts/generate_bass_test.py` — señal de prueba del fixture.

## 4. Mapa del repositorio

```text
crates/b18n-config/   JSON histórico -> CircuitDefinition; errata + overlay de calibración
crates/b18n-dsp/      núcleo DSP: numerics/ (MNA, Newton const-genérico sin alloc,
                      TPT, oversampling 4×), stages/ (input, 6SL7, tone stack,
                      mezcla, inversor 7199, potencia 7027A con acoplo TPT),
                      engine.rs (cadena por sample, reposo, NFB explícita z^-1
                      con compensación, reset sin alloc)
crates/b18n-plugin/   adaptador VST3 (NIH-plug): params con IDs estables,
                      processor con motores 1×/4× preconstruidos y bypass
                      compensado en latencia
crates/b18n-render/   CLI WAV offline + reportes + barridos
xtask/                bundler NIH-plug (pin de421011...)
assets/models/        JSON canónico + errata + calibración (incluye la 7199)
assets/fixtures/      señales y goldens (tone_stack, phase_7199)
assets/reference/     PDFs históricos y datasheets RCA (gitignored)
docs/                 guía, trazabilidad, supuestos, sprint y resolución
scripts/              extracción, trazado, ajustes, referencias y netlists SPICE
tools/                ngspice 47 y 7zr (gitignored)
```

## 5. Reglas no negociables

1. **No inventar.** Ningún valor eléctrico, curva o medición se inventa. Lo no
   documentado vive en `assets/models/b18n_1967_calibration.json` como
   `estimated`/`measured`/`datasheet`, con método, fecha y fuente.
2. **El JSON canónico es inmutable.** Las correcciones documentadas van a
   `assets/models/b18n_1967_historical_errata.json` (aplicado antes de construir
   `CircuitDefinition`, enlazado por SHA-256 y registrado en la huella efectiva).
   El overlay solo aporta parámetros de modelo; nunca componentes históricos.
3. **Fallo explícito sobre relleno silencioso:** sin modelo de válvula o sin
   reposo convergente el motor devuelve error; no se degrada con valores inventados.
4. **Callback de audio limpio:** nada de heap, locks, filesystem, logging,
   `panic`/`unwrap` ni NaN/Inf en `process()` (y `reset()` sin alloc); latencia
   reportada (hoy **47 muestras** a 4×, verificada con impulso).
5. **PDFs históricos fuera de git**; artefactos de análisis en `extracted/` y
   herramientas en `tools/` tampoco se versionan.
6. **Sin commits automáticos**: el historial git lo controla el usuario.
7. Comparaciones de calibración contra SPICE o referencia independiente; nunca
   contra la propia implementación, y registrar archivo, versión, condiciones y tolerancias.
8. **Al cambiar el modelo DSP, regenerar los goldens SPICE afectados** y subir
   `ALGORITHM_VERSION` si el cambio es audible. No editar un golden a mano.

## 6. Bloqueos resueltos (detalle en la resolución y la bitácora)

- **7199 — pinout** (RCA 1963): pin 1 placa del triodo, pin 2 placa del pentodo,
  pin 3 pantalla G2, pin 6 cátodo del pentodo, pin 7 rejilla de control, pin 8
  cátodo del triodo, pin 9 rejilla del triodo.
- **Errata 270 kΩ/470 kΩ:** pin 2 (placa) = **270 kΩ**, pin 3 (pantalla) =
  **470 kΩ**, corregido por archivo de errata con evidencia del raster y nodos.
- **150 kΩ:** resistencia de **pantalla a masa** (no NFB); SPICE da
  `R(pin3, masa)` = 150 kΩ con el carril flotante, coherente con el manual.
- **Reposo real de la 7199:** placa del pentodo 270 kΩ y pantalla 470 kΩ desde
  el carril de previo (~380 V); **10 MΩ de la placa del triodo a su rejilla** y
  **3.3 MΩ a masa**; carril del triodo **~460 V** (nodo distinto, inferido de
  `Vp+Vk=Vrail`). La lectura de **65 V en pin 9 es la lectura cargada** por un
  medidor de 20 kΩ/V (rango 250 V) de un nodo de ~88 V con fuente ~2.5 MΩ.
  Goldens en `assets/fixtures/reference/phase_7199/`.
- **Tone stack:** contraste SPICE con condiciones idénticas (SPICE–numpy
  ≤1e-9 dB / 0°). No retocar valores para igualar reediciones.
- **NFB:** 22 kΩ desde la toma de **16 Ω** al **cátodo del pentodo** (flag «F»
  del raster y texto del RCA AN-183). Lazo **explícito `z^-1`** (aproximación
  declarada) con **polo de compensación a 2,5 kHz de la tasa base** (Ola 8,
  antes 20 kHz y construido con la tasa de proceso; barrera estable/inestable
  medida 4–5 kHz efectivos a 44,1/48 kHz) en el camino de realimentación (sin
  él el lazo era inestable a la 22 kΩ real; los parásitos de HF no están
  modelados). Bloqueo de continua del OT modelado como paso alto a
  1 Hz. La polaridad del arrollamiento se asume negativa y se verifica por
  reducción de ganancia (≈0,14×) y estabilidad en `nfb_stability.rs`.
- **Memoria de acoplo PI→7027A:** los 0.05 µF son capacitores TPT por rejilla
  con retorno de 100 kΩ y conducción de rejilla (blocking distortion recuperable).
- **7027A:** el modelo sigue siendo un proxy **6L6GC**; las curvas originales y
  el ajuste fino quedan para P2.
- **SPICE:** ngspice 47 instalado en `tools/ngspice/`; netlists y goldens en
  `scripts/spice/` y `assets/fixtures/reference/`.
- **Plugin VST3 (P1.7):** `crates/b18n-plugin` con NIH-plug `de421011…`
  (GPL-3.0-only), layouts 2→2/1→1 (estéreo = dos motores),
  `SAMPLE_ACCURATE_AUTOMATION`, IDs de parámetros congelados (variantes de enum
  con `#[id]`), bypass con retardo seco de 47 muestras y crossfade de 15 ms,
  presets con huellas del modelo. Bundle en `target/bundled/B18N 67.vst3`
  (clave `[b18n-plugin]` en `bundler.toml`, debe coincidir con el nombre de
  paquete).

## 7. Próximos pasos priorizados

1. **Cierre manual de P1.7:** ejecutar **Steinberg VST3 Validator** (no
   instalado en el equipo) y probar en DAW: automatización, guardar/restaurar,
   suspensión/reactivación. Registrar en `docs/sprint.md` y marcar
   `RELEASE_CHECKLIST.md`.
2. **P2 — fuente dinámica (Hito D):** B+/bias con *sag*, rectificador 5AR4.
3. **P2 — OT no ideal + altavoz reactivo + gabinete/IR** (solo con evidencia).
4. **Mejoras documentadas:** interpolador de mayor atenuación; curvas originales
   de la 7027A; comparación `.ac/.tran` de la potencia contra SPICE.
5. **Fase 2 de CPU (opcional):** si el presupuesto de CPU tuviera que bajar
   más (hoy ≈23,5 µs/muestra estéreo 4×), atacar el coste de las evaluaciones
   Koren (tablas polinómicas o aproximaciones de corriente/pendiente) sin
   cambiar el modelo audible; la guardia de `cpu_budget.rs` detecta
   regresiones.

Archivos previstos:

```text
scripts/spice/b18n_power_7027a_1967.cir
crates/b18n-dsp/tests/power_supply_transient.rs   # P2
```

## 8. Qué NO hacer

- Corregir tensiones DC del manual «a oído» o con valores inventados.
- Retocar el tone stack para igualar especificaciones de reediciones ajenas.
- Pasar valores de ajuste no documentados al JSON histórico.
- Calibrar la curva del OT «a oído» como si fuera original.
- Quitar la compensación de 2,5 kHz de la NFB o volver a construirla con la
  tasa de proceso sin re-medir la estabilidad del lazo.
- Editar un golden SPICE a mano (se regenera con los scripts).
- Mezclar parámetros de modelo (calibración) con componentes históricos (JSON).
- Volver a calcular la latencia «sumando etapas» a mano: se mide con impulso
  (hoy 47 muestras a 4×) y se fija con test.
- Exponer 8× en el plugin: el motor no lo implementa.

## 9. Cómo verificar antes de dar algo por terminado

1. `cargo fmt --check`, `cargo clippy ... -D warnings`, `cargo test --workspace`,
   `cargo build --workspace --release` en verde.
2. Toda cifra nueva: rastreable a JSON, esquema/manual, errata o calibración
   etiquetada.
3. Cambios de DSP audibles: subir `ALGORITHM_VERSION` en
   `crates/b18n-render/src/report.rs` (y en `crates/b18n-plugin/src/lib.rs`,
   que la duplica para los presets) y ejecutar el render E2E de la sección 3.
4. Cambios en `crates/b18n-plugin`: `cargo xtask bundle b18n-plugin --release`
   y conservar los IDs de parámetros y el `VST3_CLASS_ID`; la guardia de CPU
   corre con `cargo test --release` y el bench de impresión con
   `cargo test -p b18n-plugin --release -- --ignored --nocapture`
   (ejecutarlo si se toca el callback o el solver).
5. Actualizar `docs/sprint.md` (bitácora) y, si cambia el estado de un bloqueo,
   `docs/B18N_1967_AGENT_BLOCKERS_RESOLUTION.md`.
