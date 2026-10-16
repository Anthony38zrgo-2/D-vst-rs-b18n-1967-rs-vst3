# Instrucciones para continuar el proyecto (handoff)

> **Actualización prioritaria 2026-10-16 — B18-009 continúa abierto.** Se
> construyó un candidato de solve conjunto con capacitancias RCA y referencia
> SPICE corregida. La medición de retorno y los transitorios muestran margen
> HF negativo/oscilación; el caso Rust reactivo cerrado también falla. El
> candidato está aislado en `stages/output_network.rs`, no en el plugin.
> Se mantiene el camino anterior y su fallback. Leer primero
> [`docs/B18_009_STATUS.md`](docs/B18_009_STATUS.md); sustituye las conclusiones
> de estabilidad de sprint §22.3 y el supuesto de que basta añadir parásitos.

Documento de traspaso para retomar el trabajo en el port VST3/Rust del Ampeg
B-18N/B-15ND (revisión de esquema `1967-05`). Está escrito para que un agente o
persona nueva pueda construir, verificar y continuar sin contexto previo.

**Regla de oro:** antes de tocar algo relacionado con la 7199, el tone stack, la
NFB, la etapa de potencia, la fuente, el transformador de salida o **la carga
electrical del altavoz**, leer `docs/B18N_1967_AGENT_BLOCKERS_RESOLUTION.md` y
`docs/sprint.md` (§19–§21). Ese archivo contiene la resolución de los bloqueos
con fuentes primarias y su estado de ejecución; este handoff resume y enlaza.

Documentos de referencia, en orden de lectura:

1. `docs/B18N_1967_AGENT_BLOCKERS_RESOLUTION.md` — resoluciones y prioridades P0/P1/P2.
2. `docs/sprint.md` — bitácora completa de hitos e impedimentos (§19 B18-012,
   §20 B18-013, §21 B18-015 con el archivo de problemas de esta ronda).
3. `docs/IMPLEMENTATION_GUIDE.md` — guía maestra (orden de hitos y criterios).
4. `docs/CIRCUIT_TRACEABILITY.md` — qué está trazado del esquema y con qué evidencia.
5. `docs/MODEL_ASSUMPTIONS.md` — supuestos y parámetros `estimated`/`datasheet`
   (véase la sección nueva *Carga eléctrica del altavoz (B18-015)*).
6. `docs/MEASUREMENT_PROTOCOL.md` — cómo se contrasta y se reporta.
7. `docs/DSP_ARCHITECTURE.md` — arquitectura y reglas del callback de audio.
8. `b18-fixes.md` — backlog de trabajo sin versionar (copia de trabajo del
   usuario): estado, archivos y criterios de cada B18 pendiente.

---

## 1. Objetivo

Emular por etapas el circuito del B-18N/B-15ND con un núcleo DSP en Rust
independiente del host y exportarlo como VST3 (NIH-plug). El JSON histórico es
la única fuente de parámetros; todo dato no documentado va al overlay de
calibración o al archivo de errata, con unidad, método, origen y fecha.

## 2. Estado actual (2026-10-15, tras B18-015 física)

> El estado vigente y **lo que falta** están en la sección **2bis**. Los
> números de cada ola se conservan resumidos aquí como historia; el detalle
> completo vive en `docs/sprint.md` y `CHANGELOG.md`.

| Hito | Estado |
|---|---|
| A — repositorio y modelo de datos | Completado |
| B — camino lineal verificable (tone stack MNA/TPT) | Completado |
| C1 — triodos 6SL7 no lineales, Newton, reposo, sobremuestreo 4× | Completado |
| C2 — etapa de potencia 2×7027A push-pull con OT y bias fija | Completado |
| D — fuente dinámica (B18-012) y OT-471 lineal (B18-013) | **Completado** |
| P0.1 — errata 270 kΩ/470 kΩ de la 7199 | **Cerrado** (archivo de errata + huella efectiva) |
| P0.2 — 150 kΩ de pin 3 | **Cerrado** (SPICE: R(pin3, masa) = 150 kΩ) |
| P0.3 — reposo 7199 + carga del medidor | **Cerrado** (la lectura de 65 V es carga del instrumento) |
| P1.4 — inversor 7199 real en el motor | **Cerrado** (.op/.ac/.tran contra SPICE) |
| P1.5 — tone stack contra ngspice 47 | **Cerrado** (SPICE–numpy ≤1e-9 dB / 0°) |
| P1.6 — NFB + memoria de capacitores | **Cerrado** (`z^-1` + compensación declarada; estabilidad medida) |
| P1.7 — plugin VST3 | **Cerrado en código** (`crates/b18n-plugin`, bundle generado); pendientes manuales: Steinberg Validator y DAW (B18-019) |
| Fase 1/Ola 8 — presupuesto de CPU del plugin | **Cerrado** (Newton 3D con jacobiano analítico; guardia de CPU y niveles DAW) |
| B18-001…008, 010/011 | Cerrados (manifiesto, polo NFB a tasa efectiva, tono en MNA, suavizado 15 ms, volumen físico, mezclador, jacks, inversor acoplado, 7027A desde el datasheet RCA) |
| B18-009 | **Abierto; candidato aislado y bloqueado** por márgenes/transitorios físicos, caso Rust reactivo y CPU. Ver `docs/B18_009_STATUS.md` (2026-10-16) |
| B18-012 — fuente dinámica 5AR4 + sag | **Cerrado** (red H2/H1/H3/H4 + bias, Euler implícito, contraste SPICE; `power_supply_transient.rs`) |
| B18-013 — OT-471 lineal con Lm/fuga/Cp | **Cerrado** (red diferencial en el Newton de placas, bloqueo DC físico; golden SPICE + `output_transformer_ac.rs`, ≤0,091 dB/0,97°) |
| **B18-015 — carga reactiva del altavoz** | **Física cerrada** (RLC estimado en el solve del OT, SPICE, corriente de secundario). **Lazo reactivo+NFB: bloqueado y documentado** (inestabilidad del lazo provisional; cae a la resistiva). Reabre B18-009 |
| P2 — NFB real (B18-009, **desbloqueado**), saturación del OT (B18-014), modo experimental reactivo con NFB (después de 009), gabinete/IR (016), calibración I/O (017), matriz E2E (018), distribución (019), ruido/aging (020), cierre documental (021) | Pendiente |

Verificación al cierre de esta ronda (B18-015 física; ver estado exacto en §2bis):

- `cargo fmt --all -- --check` (limpio), `cargo clippy --workspace
  --all-targets` (sin avisos) y `cargo test --workspace`: **215 tests en
  verde** (211 pasando + 1 `#[ignore]` documentado de B18-015 + 3 del bench
  de CPU del plugin, preexistentes). Incluye la batería completa de
  `tests/reactive_load.rs` (ver §2bis). **Pendientes de re-ejecutar (lista en
  §2bis):** `cargo test --workspace --release`, `cargo build --workspace
  --release`, bench de CPU y bundle: la sesión terminó antes de completarlos.
- Test nuevo `crates/b18n-dsp/tests/reactive_load.rs` (8 tests + 1 con
  `#[ignore]` justificado): contraste golden SPICE < 0,5 dB / < 10°, anclas de
  Z(f), transimpedancia `|v/i| = |Z(f)|`, corriente·resonancia, motor
  reactivo sin NFB estable, fallback con NFB activa, selección+reset, batería
  de tasas con impulso moderado (< 5.000 fallos) y el caso extremo
  (impulso 4 Vp a 44,1 kHz) `#[ignore]` por residual documentado.
- Bundle: `cargo xtask bundle b18n-plugin --release` genera
  `target/bundled/B18N 67.vst3` (no regenerado en esta ronda).
- Render E2E **invariante** (el modo por defecto no cambia), re-verificado al
  cierre: `algorithm_version` idéntico a B18-013, latencia 47 muestras a 4×,
  pico 4,916955, RMS 1,4760989, offset +0,0015545 — igual al baseline de
  B18-013 a la precisión del reporte; diferencia máxima por muestra del WAV
  1,19e-7 (reorden aritmético). Trim de salida por defecto del plugin:
  **−20,0 dB**. Por eso **no se subió `ALGORITHM_VERSION`**.

## 2bis. Handoff: qué falta y en qué orden (2026-10-15, tras B18-015 física)

Fuente detallada de cada ticket: `b18-fixes.md`. Resumen ejecutivo:

### Completado (ramas y commits, de base a punta)

| Ticket | Rama / commit | Qué quedó hecho |
|---|---|---|
| B18-001 | `fix/nfb-rate` `abcb0fd` | Línea base reproducible (`docs/FIDELITY_BASELINE.md`), manifiesto de fixtures con procedencia y SHA-256 y verificador `scripts/check_reference_manifest.py`. |
| B18-002 | `fix/nfb-rate` `abcb0fd` | Polo NFB con la **tasa de proceso efectiva** y TPT/prewarp. Test `nfb_rate_invariance.rs`. `ALGORITHM_VERSION`. |
| B18-004 | `fix/trim-smoothing` `d1d1511` | Suavizado de trims a **15 ms físicos**. Test `control_smoothing.rs`. |
| B18-003 | `fix/tone-coupling-mna` `088dcc8` | Acoplo de 0,022 µF del 6SL7#2 **dentro del MNA del tono**. Golden `tone_stack_coupled_spice.csv`. Trim por defecto −20,0 dB. |
| B18-005/006/007 | `fix/preamp-loading` `c431338` | Red de volumen física, mezclador trazado, jacks diferenciados. Goldens `volume_network_spice.csv` e `input_jacks_spice.csv`. |
| B18-008 | `feat/7199-dynamic` `1c09287` | Acoplo pentodo↔triodo del 7199 acoplado por muestra (Newton 6D). |
| B18-010/011 | `feat/7199-dynamic` (publicación autorizada 2026-10-16) | 7027A desde el datasheet RCA + división de pantalla. `power_7027a_curves.rs`. |
| B18-012 | `feat/7199-dynamic` (publicación autorizada 2026-10-16) | Fuente dinámica 5AR4 + sag (Hito D). `power_supply_transient.rs`. |
| B18-013 | `feat/7199-dynamic` (publicación autorizada 2026-10-16) | OT-471 lineal: `OutputTransformerModel` en el Newton de placas, bloqueo DC físico, golden + `output_transformer_ac.rs`. |
| **B18-015 (física)** | `feat/7199-dynamic` (publicación autorizada 2026-10-16) | Carga RLC estimada dentro del solve del OT (`stages/speaker_impedance.rs`), selección por mando, `--load` de render, golden SPICE nuevo (`speaker_load/`) + 8 tests. **Con la NFB activa la selección cae a la nominal** (bloqueo documentado); ver §Archivo. |

> **Los commits los controla el usuario (regla 6):** el 2026-10-16 autorizó
> commit y push de los cambios pendientes, incluido el diagnóstico. La
> publicación en `feat/7199-dynamic` incluye B18-010/011/012/013/015, el
> candidato aislado B18-009, `b18-fixes.md` y este handoff. B18-009 sigue abierto.

### Archivo de problemas encontrados al hacer B18-015 (2026-10-15)

Todo lo de esta lista fue **medido** durante la sesión. El detalle paso a paso
está en `docs/sprint.md` §21; aquí queda el índice completo para no repetir
errores:

1. **Euler atrás vs TPT en la red de carga.** La primera versión discretizaba
   los inductores (Le, Les) con Euler atrás, igual que el OT. El contraste con
   el golden SPICE daba **+1,77 dB de desviación a 20 kHz** (y fase ~7,9°): a
   4× la carga es inductiva (|Z(20 kHz)| ≈ 100 Ω) y `ωT` grande rompe la
   aproximación BE. Se mudó la red de carga a **TPT/trapezoidal
   integral** (inductores y condensador), declarado en
   `stages/speaker_impedance.rs` y en los comentarios del test: la carga tiene
   camino resistivo (`Re`, `Res`), no es el integrador sin amortiguación del
   flujo del OT (ahí Euler atrás sigue elegante y necesario, B18-013). Con
   TPT: < 0,5 dB / < 10° en banda audible.
2. **Dos errores de diseño del propio test** (lección de método): (a) el test
   de transimpedancia esperaba `|v_load/id| = N·|Z(f)|`, que **solo vale si
   toda la corriente va a la carga**: la rama de magnetización divide la
   corriente a baja frecuencia y aplasta el pico; el test correcto es el
   **fasor `|v_load/i_sec|`**, que es `|Z(f)|` por definición del puerto
   (así quedó). (b) el pico de corriente se medía sobre todo el render,
   incluyendo el transitorio de arranque; ahora se mide en la cola
   (`index >= frames/2`).
3. **El problema principal — NFB provisional × carga reactiva (ABIERTO,
   reabre B18-009).** Manifestación medida: con la NFB del circuito activa y
   el equivalente RLC estimado seleccionado, cualquier entrada mínima engancha
   un **ciclo límite sostenido** (silencio exacto = reposo estable; input
   0,005 V a cualquier tono → salida saturada ~6 V constante; drives mayores →
   44–106 V). Alternancia de signo cada 1–2 muestras de host → el modo
   dominante está cerca de 24 kHz tras el pliegue, con sincronización de
   fallos de Newton en cascada (inversor + potencia, ~50 % de llamadas
   fallando; el solver se congela y arrastra). La etapa de potencia **sola**
   es estable en todo el rango (0 fallos a drives 0,5–40 V, incluso con
   cambio en caliente de carga tras resolver el reposo), y el motor completo
   con la carga reactiva **y NFB desactivada** también (todos los drives y
   tasas). El conflicto es solo del **lazo**.
   - **Caracterización (para B18-009):** con el lazo en escala de ganancia
     (escalando `turns_ratio_primary_to_16ohm` ×{0,33…10}), el ratio
     cerrado/abierta va 44,8 → 31,4 → 0,72 → 0,83: el lazo reactivo solo es
     estable con ≈**1/5 de la ganancia nominal de lazo**; con ganancia ≥2
     sobre su límite oscila. La protrusión estructural medida: la carga
     inductiva **cancela el roll-off de fuga del OT**
     (`vsec/v_F ≈ N·Le/(Lleak+N²·Le)` ≈ constante en HF), subiendo la
     ganancia de lazo ~+24 dB hacia 24 kHz que el polo único de 2,5 kHz no
     cubre.
   - **Intentos de compensación provisional (todos medidos)**: polo único a
     500 Hz (trivial, −10 dB en NFB, habilitado solo limitando drive);
     1,0 kHz y 1,5 kHz (estables solo a drives ≤0,05); 2,0 kHz (fallos);
     **cascada de 2 polos 2,5 kHz (PEOR: ciclo de 44 V — típico de ciclo
     relajativo: más memoria amplifica el enganche)**; cascada 2×900 Hz
     (senos estables a todo drive y a 44,1/96/192 kHz, **pero un impulso de
     2 Vp engancha**: ventanas 86 → 102 → 11 → 4,5 V con ~14.000 fallos);
     cascada 2×600 Hz (senos estables a todo drive; el impulso queda
     **enganchado** 86 → 84 V sostenido con ~65.000 fallos).
   - **Decisión adoptada (regla 3: fallo explícito sobre relleno
     silencioso):** con la NFB activa, la selección `EstimatedLumped` **cae a
     la carga resistiva nominal** (`engine.rs::set_targets`, comentado in
     situ; `effective_speaker_load()` expone el modo efectivo; test
     `reactive_load_falls_back_with_active_nfb` lo fija en ambos casos: NFB
     con overlay y sin overlay). El modo reactivo queda **habilitado solo con
     la NFB abierta** del circuito (`negative_feedback.present = false`), que
     es la puerta con la que trabaja B18-009. **La cascada provisional se
     retiró del árbol**: el camino NFB volvió a su estado de
     B18-002/010–013 (un solo polo a 2,5 kHz, bit-exacto para la carga
     nominal).
   - **Residual con la NFB abierta (resuelto en batería, dos capas):** (a)
     tras un **salto violento** de nivel (impulso 0,2–1,0 Vp), el solve de
     potencia deja una ráfqaga corta de fallos que se recupera: medidos
     155–1.347 llamadas de 65.536 según la tasa; el **seno equivalente a
     todo drive es limpio** (0 fallos hasta el recorte), luego es el salto
     discreto y no el nivel lo que lanza el solve. La batería de tasas corre
     con criterio `< 5.000` (≈7,6 %) para la reactiva y `< 500` para la
     nominal (como `nfb_stability`). (b) **Impulso extremo 4 Vp a
     44,1 kHz con carga reactiva sin NFB: engancha el solve aislado**
     (33.855/65.536 ≈ 52 %, ventana sostenida) — test
     `reactive_impulse_extreme_pending_b18_009` con `#[ignore] = "residual
     B18-015 conocido…"`. La misma batería con carga nominal es limpia. Se
     revisa en B18-009 al cerrar el lazo y el solve acoplado.
4. **Comportamiento esperado (no problema) que conviene conocer:** el cambio
   de carga en caliente re-instancia la red del altavoz (arranca descargada,
   transitorio corto declarado); es una selección de configuración, no un
   mando de ejecución, y no se suaviza a propósito.
5. **Estado histórico al cierre de B18-015:** la ronda estaba sin commits;
   la publicación fue autorizada el 2026-10-16 (ver tabla anterior). No confundir los archivos modificados
   previos (B18-012/013) con los de esta ronda: los nuevos son
   `stages/speaker_impedance.rs`, `tests/reactive_load.rs`,
   `scripts/spice/b18n_speaker_load.cir`, `scripts/spice/speaker_load_reference.py`,
   `assets/fixtures/reference/speaker_load/` (golden + README) y las
   modificaciones de `output_transformer.rs`, `power_7027a.rs`, `engine.rs`,
   `controls.rs`, `circuit.rs`, `calibration.rs`, `mapping.rs`, `params.rs`,
   `render/main.rs`, `render/report.rs` y los albores de documentación.

### Siguiente trabajo, en orden recomendado

**0. Cierre de la ronda (primero, ~20 min):** re-ejecutar
`cargo test --workspace --release` (la guardia de CPU incluida),
`cargo build --workspace --release`, el bench de impresión de CPU
(`cargo test -p b18n-plugin --release -- --ignored --nocapture`) y
`cargo xtask bundle b18n-plugin --release` (conservar IDs y `VST3_CLASS_ID`).
`fmt`, `clippy` y la batería completa debug **ya están** verificados al cierre
(§2); con el render E2E confirmar que el fingerprint sigue siendo el mismo
(pico 4,916955 / RMS 1,4760989; re-verificado tras el gate).

**1. B18-009 (P1, ahora sin bloqueo previo) — NFB sobre red real sin
`z^-1` ni LP de compensación, y re-apertura del modo reactivo con NFB.**

> **Avance 2026-10-15 (paso 0 cerrado; paso 1 caracterizado; cierre implícito
> intentado, medido y revertido):** `scripts/spice/nfb_loop_reference.py`
> simula el lazo completo (7199 + 2×7027A con capacitancias de datasheet RCA +
> OT-471 + NFB 22 kΩ) sin `z^-1` ni LP. Resultado (sprint §22): con carga
> reactiva el lazo cerrado no tiene pico HF (0,97 dB a 399 Hz); sin las
> capacitancias de válvula sube a +16 dB a 2 MHz. El roll-off HF lo dan las
> capacitancias y el LP de 2,5 kHz solo enmascaraba el retardo `z^-1`.
> Se implementó el cierre **implícito** por muestra en `engine.rs`: **no
> converge** en el modelo discreto sin los parásitos físicos (9.000–15.000
> fallos de solver a 44,1 kHz / 16 Ω / 1 V; sprint §22.5) y se **revirtió**
> (árbol verde, render E2E invariante, `ALGORITHM_VERSION` sin cambio).
> **Pendiente (ronda propia):** añadir las capacitancias de datasheet al
> modelo del motor, cerrar el lazo con un solver con búsqueda de línea y medir
> CPU antes de retirar el polo.

- **Dependencias:** 002/008/012/013/015-física hechas. **El ticket ya puede
  empezarse**: la puerta del modo reactivo es `negative_feedback.present =
  false` (tests `reactive_load.rs`; el bloqueo documentado en
  `engine.rs::set_targets` se retira cuando el lazo físico sea estable).
- **Estado:** el lazo realimenta la salida secundaria **anterior** con
  `z^-1` + LP provisional de 2,5 kHz; ninguno figura en el esquema. Con
  carga reactiva el lazo provisional es inestable (ver Archivo, punto 3).
- **Objetivo:** equilibrio implícito por muestra o WDF/MNA con estados
  causales físicos, conservando la red de 22 kΩ desde la toma de 16 Ω al
  cátodo del pentodo; la fase HF debe surgir de **capacitancias de válvulas,
  transformador y carga**. Candidato físico (documentado): capacitancias
  interelectrodinales de la 7199 y la 7027A y capacitancias de arrollamiento
  del OT-471, desde los datasheets RCA (no inventar valores, `estimated` con
  método).
- **Método:** comparar AC de lazo abierto, margen de ganancia/fase y escalón
  en SPICE; **no retirar** `z^-1`/LP hasta demostrar estabilidad en todas
  las tasas (44,1–192 kHz) y extremos (8/16 Ω, **carga reactiva estimada
  incluida**, drive máximo, impulso fuerte). El archivo de mediciones de
  margen del Archivo (punto 3) es el punto de partida.
- **Archivos:** `engine.rs` (`feedback_state/polarity/lowpass`, la puerta del
  bloqueo en `set_targets`), `phase_splitter.rs`, `power_7027a.rs`,
  `output_transformer.rs`; posibles nuevos
  `stages/output_network.rs`/`numerics/coupled_solver.rs`; tests realterados
  (abrir el modo reactivo con NFB en `reactive_load.rs` y `nfb_stability.rs`).

**2. B18-016 (P1) — gabinete/IR con metadatos.** Perfiles IIR modernos
declarados; requiere IR/micrófono/posición medidos. Convolución particionada
FFT **sin allocs en audio**, buffers precreados, alineación de latencia y
asignación explícita altavoz↔IR. **La IR no sustituye la carga eléctrica**
(B18-015 ya la separa; `speaker_impedance.rs` frente a `speaker.rs`).

**3. B18-017 (P1) — calibración I/O.** Protocolo de niveles (1,0 = 1 Vp),
headroom y potencia nominal (50 W en 8 Ω); formalizar el −20,0 dB actual
contra medición; ahora la potencia puede medirse además con carga reactiva
(estimada) si se quiere margen de diseño. Test `level_calibration.rs`.

**4. B18-018 (P1) — matriz E2E y presupuestos.** `docs/VALIDATION_MATRIX.md`
(DC, AC, THD, NFB, potencia, fuente, OT, RT-safety, CPU); incluye el margen
de alias 4× y la mejora de interpolador. Puede adelantarse parcialmente con
lo ya medido en sprint §19–§21.

**5. B18-019 (P2) — distribución VST3.** Cierre manual de P1.7 (Steinberg
Validator, DAW, automatización sample-accurate, presets), CI. El parámetro
nuevo `load_model` ya está con IDs congelados verificados (13 IDs).

**6. B18-020 (P2) — ruido/hum/vibración/aging.** Solo con mediciones de
hardware y desactivable para tests deterministas (depende de 016).

**7. B18-021 (P1) — trazabilidad y etiquetas finales.** Niveles de
validación por etapa; actualizar `docs/RELEASE_CHECKLIST.md` (desfasado).

> **B18-014 (P2) — saturación e histéresis del núcleo OT-471.** Bloqueado
> hasta tener datos/medidas; el modelo lineal lleva una cota provisional de
> flujo (0,12 A) que solo acota transitorios. No modelar la curva magnética
> sin evidencia.

### Limitaciones declaradas pendientes (no bloquean; no retocar sin evidencia)

- **Impedancia de salida del tono sobre el mezclador:** el mezclador usa
  fuentes idealizadas (`(v1+v2)/2`); la `Z_out` de cada red de tono en serie
  con su 270 kΩ queda pendiente; puede cubrirse con B18-015 (parcial) o al
  cerrar la red de carga. (Estado igual que antes.)
- **Conducción de rejilla del pentodo 7199** no modelada (solo drive
  extremo).
- **Robustez del inversor a drive extremo:** respaldo declarado, re-verificado
  tras B18-012/013/015 (0 fallos a niveles DAW).
- **Curva real 5AR4/GZ34 y DCR del PT sin documentar** (declarado en
  B18-012).
- `Lm`/`Lleak`/`Cp`/`R_core` del OT (B18-013) y **`Re`/`Le`/`Res`/`Les`/`Ces`
  del altavoz (B18-015)** son estimaciones de ingeniería (método y fecha en
  la calibración); la cota de flujo 0,12 A es un limitador numérico;
  Euler atrás desvía la fase HF del OT de forma declarada y medida.
- **El `z^-1` de una muestra en la fuente** (B18-012) y el **`z^-1` + LP de la
  NFB** (hasta B18-009).
- **Impulso fuerte + carga reactiva sin NFB:** ~0,9 % de fallos de solve
  transitorios a 44,1 kHz (acotado `< 500`; Archivo, punto 3-final).

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

Carga eléctrica del render (B18-015): **`--load resistive`** (defecto) o
`--load lumped`. Con el modelo enviado (NFB activa) `--load lumped` cae
a la resistiva (bloqueo documentado); el modo reactivo se ejercita vía tests
o circuito con NFB abierta. La elección queda en `SettingsReport.load`.

SPICE (ngspice 47 ya instalado en `tools/ngspice/`, ignorado por git):

```bash
python scripts/spice/run_spice.py scripts/spice/b18n_7199_nfb_dc_1967.cir
python scripts/spice/phase_inverter_reference.py         # golden AC del inversor
python scripts/spice/tone_stack_sweep.py                 # golden del tone stack aislado
python scripts/spice/tone_stack_coupled_sweep.py         # golden del acoplo + tono (B18-003)
python scripts/spice/input_jacks_sweep.py                # golden de jacks de entrada (B18-007)
python scripts/spice/volume_network_sweep.py             # golden de la red de volumen (B18-005)
python scripts/spice/power_supply_reference.py           # golden DC + tran de la fuente (B18-012)
python scripts/spice/output_transformer_reference.py     # golden AC del OT-471 (B18-013)
python scripts/spice/speaker_load_reference.py           # golden OT + carga RLC (B18-015)
python scripts/check_reference_manifest.py               # procedencia y SHA-256 (14 fixtures OK)
```

Bench de CPU (release; la guardia corre en `cargo test --release`):

```bash
cargo test -p b18n-plugin --release -- --ignored --nocapture   # imprime µs/muestra
```

Tests de la carga reactiva (B18-015):

```bash
cargo test -p b18n-dsp --test reactive_load -- --nocapture   # imprime la desviación vs golden
```

Scripts auxiliares (Python 3.14; requieren `pypdf`, `pypdfium2`, `Pillow`,
`numpy` y `scipy`):

- `scripts/extract_pdfs.py` — extrae imágenes/texto de los PDF históricos.
- `scripts/trace_schematic.py` / `scripts/ascii_schematic.py` — segmentos y mapas
  ASCII del esquema raster para trazado de nodos.
- `scripts/fit_triode_model.py` / `scripts/fit_7199_model.py` — ajustes Koren
  del 6SL7 y de la 7199 (pentodo y triodo) contra anclas documentadas.
- `scripts/digitize_7027a_curves.py` / `scripts/fit_7027a_model.py` — curvas
  digitalizadas y ajuste Koren + división de pantalla de la 7027A (B18-010/011).
- `scripts/tone_stack_reference.py` — referencia numpy del tone stack.
- `scripts/spice/*` — netlists, sweeps y generadores de goldens de referencia
  independiente (ahora incluye `b18n_speaker_load.cir` +
  `speaker_load_reference.py`).
- `scripts/generate_bass_test.py` — señal de prueba del fixture.

## 4. Mapa del repositorio

```text
crates/b18n-config/   JSON histórico -> CircuitDefinition; errata + overlay de calibración
                      (ahora incluye `speaker.reactive_load.*`, B18-015)
crates/b18n-dsp/      núcleo DSP: numerics/ (MNA, Newton const-genérico sin alloc,
                      TPT + Euler atrás, oversampling 4×), stages/ (input, 6SL7,
                      tone stack, mezcla, inversor 7199, potencia 7027A,
                      fuente dinámica B18-012, OT-471 lineal B18-013,
                      **carga eléctrica del altavoz B18-015: speaker_impedance.rs**,
                      gabinete acústico speaker.rs), engine.rs
                      (cadena por sample, reposo con punto fijo rieles↔etapas,
                      NFB explícita z^-1 con compensación + puerta del modo
                      reactivo en set_targets, reset sin alloc)
crates/b18n-plugin/   adaptador VST3 (NIH-plug): params con IDs estables (13
                      ids; nuevo `load_model`), processor con motores 1×/4×
                      preconstruidos y bypass compensado en latencia
crates/b18n-render/   CLI WAV offline + reportes + barridos (`--load`)
xtask/                bundler NIH-plug (pin de421011...)
assets/models/        JSON canónico + errata + calibración (7199, 7027A, fuente,
                      OT, **altavoz B18-015**)
assets/fixtures/      señales y goldens (tone_stack, tone_stack acoplado,
                      phase_7199, input_jacks, volume_network, power_7027a,
                      power_supply, output_transformer,
                      **speaker_load**) + manifiesto (14 entradas)
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
3. **Fallo explícito sobre relleno silencioso:** sin modelo de válvula, sin
   reposo convergente, con el modelo del OT incompleto o **pidiendo la carga
   reactiva en condiciones no habilitadas** el motor degrada con el
   comportamiento documentado (o falla); no se degrada en silencio con valores
   inventados.
4. **Callback de audio limpio:** nada de heap, locks, filesystem, logging,
   `panic`/`unwrap` ni NaN/Inf en `process()` (y `reset()` sin alloc); latencia
   reportada (hoy **47 muestras** a 4×, verificada con impulso).
5. **PDFs históricos fuera de git**; artefactos de análisis en `extracted/` y
   herramientas en `tools/` tampoco se versionan.
6. **Sin commits automáticos**: el historial git lo controla el usuario.
7. Comparaciones de calibración contra SPICE o referencia independiente; nunca
   contra la propia implementación, y registrar archivo, versión, condiciones y
   tolerancias.
8. **Al cambiar el modelo DSP, regenerar los goldens SPICE afectados** y subir
   `ALGORITHM_VERSION` si el cambio es audible. No editar un golden a mano.
9. **La carga reactiva estimada no se habilita con NFB activa** hasta que
   B18-009 cierre el lazo sobre la red física y vuelva a medir los márgenes
   (Archivo, punto 3). No intentar arreglarlo re-tuneando la compensación
   provisional: los intentos y sus datos están archivados.

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
  **3.3 MΩ a masa**; carril del triodo **~460 V**. La lectura de **65 V en
  pin 9 es la lectura cargada** por un medidor de 20 kΩ/V (rango 250 V).
  Goldens en `assets/fixtures/reference/phase_7199/`.
- **Tone stack:** contraste SPICE con condiciones idénticas (SPICE–numpy
  ≤1e-9 dB / 0°). No retocar valores para igualar reediciones.
- **NFB:** 22 kΩ desde la toma de **16 Ω** al **cátodo del pentodo** (flag «F»
  del raster y texto del RCA AN-183). Lazo **explícito `z^-1`** (aproximación
  declarada) con **polo de compensación provisional a 2,5 kHz** en el camino de
  realimentación. Desde **B18-002** el polo se construye con la **tasa de
  proceso efectiva** y TPT con prewarp; desde **B18-013** el bloqueo de
  continua del lazo es **físico** (red del OT con Lm y acoplo al EMF del
  flujo). **B18-015 midió que este lazo provisional es inestable con la carga
  reactiva** (Archivo) y bloquea esa combinación: la retirada del `z^-1`/polo
  con la red real es **B18-009** (ya desbloqueado por la física de 015). La
  polaridad del arrollamiento se asume negativa y se verifica por reducción de
  ganancia en `nfb_stability.rs`.
- **Memoria de acoplo PI→7027A:** los 0.05 µF son capacitores TPT por rejilla
  con retorno de 100 kΩ y conducción de rejilla (blocking distortion recuperable).
- **7027A:** ajustada al datasheet RCA 7027-A (8-59), incluida la división de
  pantalla `Ig2 = Ip·(F + G·e^(−Vp/V0))` contra Vp. El proxy 6L6GC queda solo
  como respaldo legacy (`kg2`).
- **Fuente dinámica (B18-012):** red H2 [30 µF, CT] → 1k → H1 [40 µF,
  pantallas] → 10k → H3 [40 µF, triodo] y 22k → H4 [40 µF, previo], más bias
  10D6 + 100 µF → −55 V (Hipótesis A, `estimated`). Rectificador como
  Thevenin promedio (`Rrect = 150 Ω`); Euler implícito por muestra con lazo de
  corriente de todas las etapas. Reposo: B+ 505, H1 495, H3 472, H4 394 V.
- **OT-471 lineal (B18-013):** `OutputTransformerModel` con red diferencial
  `2·Rh + R_core + (Lm ∥ (Lleak + N²·(Zcarga+DCR_sec))) ∥ Cp` dentro del Newton
  de placas; secundario acoplado al EMF del flujo (bloqueo DC físico).
  Parámetros `estimated` en el overlay (`output_transformer.*`); cota
  provisional de flujo 0,12 A. Contraste ngspice + analítica en
  `assets/fixtures/reference/output_transformer/` (≤0,091 dB/0,97° a 4×).
- **Carga eléctrica del altavoz (B18-015):** `ElectricalLoad` (puerto Norton
  TPT) dentro de la rama reflejada del OT, con corriente de secundario;
  equivalente RLC `estimated` (`speaker.reactive_load.*`) y fallback resistivo
  nominal; selección por mando con **fallback a nominal con NFB activa**
  (bloqueo documentado); golden SPICE nuevo en
  `assets/fixtures/reference/speaker_load/` y tests `reactive_load.rs`.
- **Plugin VST3 (P1.7):** `crates/b18n-plugin` con NIH-plug `de421011…`
  (GPL-3.0-only), layouts 2→2/1→1 (estéreo = dos motores),
  `SAMPLE_ACCURATE_AUTOMATION`, IDs de parámetros congelados (**13**: se
  añadió `load_model`, variante `nominal`/`rlc_estimated`), bypass con
  retardo seco de 47 muestras y crossfade de 15 ms, presets con huellas del
  modelo. Bundle en `target/bundled/B18N 67.vst3`.
- **SPICE:** ngspice 47 instalado en `tools/ngspice/`; netlists y goldens en
  `scripts/spice/` y `assets/fixtures/reference/`.

## 7. Próximos pasos priorizados (resumen; detalle en 2bis)

0. **Cierre de la ronda B18-015** (fmt/clippy/tests/bench/bundle; §2bis.0).
1. **B18-009 (P1):** NFB sin `z^-1`/LP artificiales con capacitancias físicas
   de válvula/OT y re-apertura del modo reactivo con NFB (el archivo de
   mediciones de margen está en sprint §21.2).
2. **B18-016/017/018 (P1):** gabinete/IR, calibración I/O y matriz E2E.
3. **B18-019 (P2):** cierre manual de P1.7 (Steinberg Validator y DAW).
4. **B18-020/021 (P2):** ruido/hum/aging y cierre documental.
5. **B18-014 (P2):** saturación/histéresis del OT solo con datos.
6. **Mejora opcional de CPU:** si hay que bajar de ≈27,2 µs/muestra, atacar
   las evaluaciones Koren (tablas polinómicas) sin cambiar el modelo audible.

Archivos creados/pendientes:

```text
crates/b18n-dsp/src/stages/speaker_impedance.rs   # B18-015 (hecho)
crates/b18n-dsp/tests/reactive_load.rs            # B18-015 (hecho)
scripts/spice/b18n_speaker_load.cir               # B18-015 (hecho)
scripts/spice/speaker_load_reference.py           # B18-015 (hecho)
assets/fixtures/reference/speaker_load/           # B18-015 (golden + README, hecho)
scripts/spice/b18n_power_7027a_1967.cir           # netlist de potencia completo (B18-018/P2)
docs/VALIDATION_MATRIX.md                         # B18-018 (pendiente)
crates/b18n-dsp/tests/level_calibration.rs        # B18-017 (pendiente)
stages/convolution.rs + tests/speaker_ir.rs       # B18-016 (pendiente)
```

## 8. Qué NO hacer

- Corregir tensiones DC del manual «a oído» o con valores inventados.
- Retocar el tone stack para igualar especificaciones de reediciones ajenas.
- Pasar valores de ajuste no documentados al JSON histórico.
- Calibrar la curva del OT «a oído» como si fuera original.
- Inventar la curva de conducción de la 5AR4 o la DCR del PT para «mejorar»
  el sag (declarado en B18-012).
- **Añadir una `Z(f)` del altavoz inventada**: solo el RLC estimado
  (`speaker.reactive_load.*`) existe y es opt-in;lo sustituye una medición,
  no otra estimación.
- Quitar la compensación de 2,5 kHz de la NFB o el `z^-1` sin cerrar B18-009 y
  sin medir el margen del lazo.
- **Habilitar la carga reactiva estimada con la NFB activa** (Archivo, punto
  3): el lazo provisional es inestable; la puerta se abrirá desde B18-009.
- Sustituir el **Euler atrás** de los inductores del OT por trapezoidal sin
  repetir el test de estrés de alias y el contraste AC. (Lo contrario también:
  no cambiar la red de carga a Euler atrás; el contraste con el golden empeoró
  ~1,8 dB a 20 kHz, medido.)
- Tocar la cota de flujo 0,12 A o `R_core` como si fueran datos medidos (B18-014).
- Editar un golden SPICE a mano (se regenera con los scripts y actualizando el
  manifiesto).
- Mezclar parámetros de modelo (calibración) con componentes históricos (JSON).
- Volver a calcular la latencia «sumando etapas» a mano: se mide con impulso
  (47 muestras a 4×) y se fija con test.
- Exponer 8× en el plugin: el motor no lo implementa.

## 9. Cómo verificar antes de dar algo por terminado

1. `cargo fmt --check`, `cargo clippy ... -D warnings`, `cargo test --workspace`
   (debug y release), `cargo build --workspace --release` en verde.
2. Toda cifra nueva: rastreable a JSON, esquema/manual, errata o calibración
   etiquetada.
3. Cambios de DSP audibles: subir `ALGORITHM_VERSION` en
   `crates/b18n-render/src/report.rs` (y en `crates/b18n-plugin/src/lib.rs`,
   que la duplica para los presets) y ejecutar el render E2E de la sección 3.
   (Esta ronda no lo requirió: el modo por defecto es invariante.)
4. Cambios en `crates/b18n-plugin`: `cargo xtask bundle b18n-plugin --release`
   y conservar los IDs de parámetros y el `VST3_CLASS_ID`; la guardia de CPU
   corre con `cargo test --release`.
5. Cambios en la carga reactiva u OT: regenerar los goldens con
   `scripts/spice/speaker_load_reference.py` /
   `output_transformer_reference.py`, actualizar el SHA-256 del
   `assets/fixtures/reference/manifest.json` y repetir el contraste de banda
   audible de `tests/reactive_load.rs` / `output_transformer_ac.rs`.
6. Actualizar `docs/sprint.md` (bitácora), `CHANGELOG.md` y, si cambia el
   estado de un bloqueo, `docs/B18N_1967_AGENT_BLOCKERS_RESOLUTION.md`.

---

*Handoff escrito tras la ronda B18-015 física (2026-10-15). La sección §2bis
«Archivo de problemas» es la continuación directa de `docs/sprint.md` §21;
conservadla hasta que B18-009 reescriba el lazo.*
