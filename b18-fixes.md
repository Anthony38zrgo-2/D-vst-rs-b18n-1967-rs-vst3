# B-18N 1967 — Backlog de correcciones y fidelidad física del VST3

> **Cierre B18-009, 2026-10-16:** causa raíz aislada en SPICE (resonancia
> estimada del OT a 145 kHz en el cruce del lazo + amortiguamiento HF ausente +
> carga RLC estimada, no la topología NFB), `Cp` del OT corregido a 1,5 nF y
> etapa de salida física acoplada (`OutputNetwork`) integrada en el motor,
> retirando el `z^-1` y el polo de compensación de 2,5 kHz. `ALGORITHM_VERSION`
> → `…-supply-ot471-linear-nfb-coupled-v2`. Informes:
> [`docs/B18_009_STATUS.md`](docs/B18_009_STATUS.md) y
> [`docs/B18_009_SENSITIVITY.md`](docs/B18_009_SENSITIVITY.md). El cuerpo
> siguiente conserva la propuesta histórica original; no es una tabla de avance
> actual.

**Proyecto:** [`Anthony38zrgo-2/D-vst-rs-b18n-1967-rs-vst3`](https://github.com/Anthony38zrgo-2/D-vst-rs-b18n-1967-rs-vst3)\
**Rama auditada:** `main` · **commit de referencia:** `fecf203bab9e48a899206c4400190332e5eba859` (`fixes`, 2026-10-14)\
**Fecha:** 2026-10-14 · **Estado:** backlog propuesto, **no implementado**\
**Objetivo:** reproducir por circuito, y con incertidumbres trazadas, la revisión **Ampeg B-18N/B-15ND de mayo de 1967 (5/67)**. No confundir con las revisiones de 1965, B-15N posteriores ni con el gabinete B-15ND.

> **Límite de esta auditoría:** inspección de los archivos de GitHub, referencias de circuito y documentación disponible. No se ha ejecutado `cargo test`, realizado una escucha A/B ni medido un B-18N físico desde este entorno. Cuando un punto se etiqueta **defecto constatado**, se trata de una discrepancia deducible del código; las consecuencias cuantitativas necesitan tests. Cuando se etiqueta **mejora de modelo**, no implica que exista un bug funcional.

## Índice de prioridades

| ID | Prioridad | Área | Diagnóstico | Dependencias |
|---|---|---|---|---|
| B18-001 | P0 | Referencia y regresiones | Garantizar una línea base reproducible | — |
| B18-002 | P0 | NFB | Frecuencia de filtro incorrecta bajo 2×/4× (**constatado**) | 001 |
| B18-003 | P0 | Tone stack/acoplo | Atenuación de fuente duplicada y acoplo RC separado (**constatado**) | 001 |
| B18-004 | P0 | Parámetros | Tiempo de suavizado × oversampling (**constatado**) | 001 |
| B18-005 | P1 | Previo/volumen | Fuente, potenciómetro y rejilla de 6SL7 parcialmente aproximados | 003 |
| B18-006 | P1 | Mezclador | Resistencia a rejilla de 1 MΩ asumida y carga desacoplada | 003,005 |
| B18-007 | P1 | Entrada | Jacks Bass/Guitar procesados igual (**constatado**) | 001 |
| B18-008 | P1 | 7199 | Red 10 MΩ/3,3 MΩ y acoplo AC simplificados | 002,006 |
| B18-009 | P1 | NFB física | Retardo `z^-1` y polo 2,5 kHz no derivados del esquema | 002,008,012,013,015 |
| B18-010 | P1 | 7027A | Parámetros tomados de 6L6GC; falta calibración directa | 001 |
| B18-011 | P1 | 7027A | Corriente de pantalla sin dependencia de Vplaca | 010 |
| B18-012 | P1 | Alimentación | 5AR4, filtrado, caída B+ y bias dinámicos ausentes | 010,011 |
| B18-013 | P1 | OT-471 | Modelo de transformador idealizado: Lm, Lfuga, Cp ausentes | 010,012 |
| B18-014 | P2 | OT-471 | Saturación/histéresis del núcleo por caracterizar | 013 |
| B18-015 | P1 | Altavoz/carga | Carga fija 8 Ω, sin impedancia reactiva | 013 |
| B18-016 | P1 | Gabinete | Perfiles IIR arbitrarios, sin IR/medición de 18" | 015 |
| B18-017 | P1 | Calibración I/O | Adaptación voltios↔dBFS y potencia sin protocolo completo | 003,010,015 |
| B18-018 | P1 | Pruebas y rendimiento | Métricas objetivas/DAW tras modificar DSP | Todas las P0/P1 |
| B18-019 | P2 | VST3/distribución | Validación externa, presets, latencia y empaquetado | 018 |
| B18-020 | P2 | Ruido/variabilidad | Solo si existen medidas: hum, ruido, tolerancias | 012,016 |
| B18-021 | P1 | Proveniencia y documentación | Dejar explícito qué parte está medida/estimada | Todos |

**Orden recomendado:** `001 → 002+004 → 003 → 005+006+007 → 008 → 010+011 → 012 → 013 → 015 → 009 → 016+017 → 018 → 019`; `014` y `020` después de medir. No abordar la saturación magnética o el cabinet antes de garantizar una NFB estable con la carga reactiva.

## 0. Fuente de verdad y reglas generales

### Material de referencia

1. **Referencia de conexiones del circuito:** [B-18N/B-15ND, esquema 5/67](https://el34world.com/charts/Schematics/Files/Ampeg/Ampeg_b18n_b15nd_schematic_5_67.pdf), reproducción del plano histórico, que el proyecto identifica como `SCH-567` (`4010036`).
2. **Servicio y medidas históricas:** [B18N 1967 service manual, cuarta edición/segunda impresión 1968](https://www.vintage-blue.com/assets/b18n_1967_service_manual.pdf), identificado como `SM-1967`. Sus lecturas de alta impedancia están afectadas por el voltímetro de 20.000 Ω/V.
3. **Catálogo del fabricante:** [Ampeg Technical Schematics](https://ampeg.com/support/schematics.html). El enlace general es oficial; **no sustituir** al archivo 5/67 sin confirmar que el PDF específico sea exactamente la misma revisión.
4. **Datasheets de fabricantes:** [RCA 7199 (1963)](https://www.drtube.com/datasheets/7199-rca1963.pdf), [RCA 7027-A](https://frank.pocnet.net/sheets/049/7/7027A.pdf), [Mullard GZ34/5AR4](https://frank.pocnet.net/sheets/129/g/GZ34_Mullard.pdf). El último es una hoja de características de rectificadora, no una caracterización del rectificador individual del B-18N.
5. **Referencias versionadas del proyecto:** `docs/CIRCUIT_TRACEABILITY.md`, `docs/MODEL_ASSUMPTIONS.md`, `assets/models/B18N_1967_simulation_parameters.json`, `assets/models/b18n_1967_historical_errata.json`, `assets/models/b18n_1967_calibration.json`, `scripts/spice/` y `assets/fixtures/reference/`.

**No sobreescribir** el JSON histórico por ajustes DSP. Los errores del material histórico se documentan en `b18n_1967_historical_errata.json` con evidencia y SHA; los parámetros de ajuste viven en `b18n_1967_calibration.json` con `origin = estimated | datasheet | measured`. Los valores desconocidos permanecen `null`/`None`. Los modelos obtenidos por estimación **no** se presentan como medidos. Los PDFs de terceros permanecen fuera del repositorio según `.gitignore`.

### Invariantes técnicos (para todo PR)

- La instancia digital representa **un** amplificador mono con dos preamplificadores internos y una etapa de potencia/OT/fuente compartidos. En configuración host estéreo, dos instancias eléctricas independientes, salvo que se documente otro diseño.
- Todo componente que afecte a la corriente de un nodo se estampa **una vez** en el sistema de ecuaciones. No aplicar pérdidas RC de manera externa si el mismo resistor/capacitor está dentro del MNA.
- `sample_rate_hz` debe indicar claramente si es **tasa del host** o **tasa efectiva**; no avanzar un estado más veces que las contempladas al diseñar sus coeficientes.
- Evitar asignaciones, bloqueos, lectura de disco, creación de `Vec` y operaciones de I/O dentro de `process_sample`/callback del DAW. Preparar configuraciones y matrices al inicializar o cuando sea imprescindible fuera de audio.
- Parametrización en voltios, ohmios, faradios, amperios y henrios dentro del DSP. Convertir a `f32`/dBFS **solo** en la frontera con DAW; conservar `f64` en solves de circuito.
- No introducir filtro/compresor/limitador como *corrección acústica* sin identificarlo como adaptación moderna. Preservar modo `Direct` que no aplique cabinet.
- No usar las curvas de una reedición B-15 como criterio absoluto de aceptación de un B-18N 5/67.
- Cada ticket debe terminar con `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings` y `cargo test --workspace` (además de sus pruebas específicas). Registrar CPU y número de fallos de convergencia.

---

## B18-001 — Congelar línea base reproducible y referencias

**Prioridad:** P0 · **Clase:** pruebas/higiene · **Riesgo:** bajo.\
**Archivos existentes:** `README.md`, `docs/MEASUREMENT_PROTOCOL.md`, `docs/CIRCUIT_TRACEABILITY.md`, `scripts/spice/README.md`, `.github/workflows/ci.yml`.\
**Crear:** `docs/FIDELITY_BASELINE.md`, `assets/fixtures/reference/full_amp/README.md` y, si se justifica, `scripts/check_reference_manifest.py`.

**Implementación:**

1. Guardar `commit SHA`, Rust toolchain, sistema operativo, ngspice, tasas 44.1/48/96/192 kHz, oversampling 1×/2×/4×, latencias y hashes de JSON/errata/calibración. Generar un reporte de estado **antes** de cambiar algoritmos.
2. Ejecutar (en máquina de desarrollo) todas las pruebas existentes. Registrar `nfb_stability`, `tone_stack_response`, `phase_inverter_ac`, `alias_suppression`, `latency`, `parity`, `no_alloc`, `cpu_budget` y render offline. Si alguna falla, conservar su log: no marcar el baseline como aprobado.
3. Añadir entradas de fixture para medición de frecuencia, THD, DC, transitorios y carga. Cada fixture debe registrar **procedencia**: `historical-manual`, `manufacturer-datasheet`, `independent-SPICE`, `real-hardware` o `model-only`.
4. Generar señal sintética por amplitud en voltios (no solo fracciones arbitrarias de dBFS), y CSV/JSON con metadatos. Mantener el audio de referencia A/B separado del audio generado por el mismo motor, para evitar tests autocorrelacionados.
5. Registrar como hipótesis —no validación— los parámetros sin medida del OT y del gabinete.

**Aceptación:** baseline versionado; hashes verificables; scripts ejecutables en Windows; `cargo test --workspace` con resultado registrado. CI no necesita redistribuir PDFs ajenos.

---

## B18-002 — Corregir la frecuencia efectiva de compensación NFB

**Prioridad:** P0 · **Clase:** defecto de código constatado.\
**Archivo principal:** `crates/b18n-dsp/src/engine.rs`, constantes y `LinearChain::new` (alrededor de líneas 36–50, 327–330, 407–415 y 478–490).\
**Archivos relacionados:** `crates/b18n-dsp/src/utilities/filters.rs` (`OnePoleLowPass`), `crates/b18n-dsp/src/numerics/oversampling.rs` (`Oversampler::process`), `crates/b18n-dsp/tests/nfb_stability.rs`, `docs/MODEL_ASSUMPTIONS.md`, `docs/CIRCUIT_TRACEABILITY.md`, `crates/b18n-plugin/src/lib.rs` (`ALGORITHM_VERSION`).

**Problema confirmado:** `feedback_lowpass` se construye con `spec.sample_rate_hz`, pero `LinearChain::process_sample()` corre 2 o 4 veces por muestra host dentro de `Oversampler::process()`. El filtro integra demasiadas veces y su polo efectivo sube con el factor de OS. El comentario de código que afirma que el feedback avanza una vez por muestra del host contradice el flujo real. No cambiar solo el comentario.

**Parche mínimo:**

```diff
 let processing_rate = spec.sample_rate_hz * f64::from(spec.oversampling);
 ...
 feedback_lowpass: OnePoleLowPass::new(
     FEEDBACK_COMPENSATION_CUTOFF_HZ,
-    spec.sample_rate_hz,
+    processing_rate,
 ),
```

**Trabajo adicional obligatorio:**

- Decidir si el filtro sigue siendo `OnePoleLowPass` por Euler implícito o se usa TPT/bilineal con prewarp. Esta elección cambia el corte real a frecuencias altas; no cambiar el método a la vez que el polo sin registrar su impacto.
- Sustituir en `engine.rs`/docs comentarios incorrectos por “NFB calculada **por muestra sobremuestreada**”. El `z^-1` actual es **un sample de `processing_rate`** (no uno de host).
- Recalcular curvas de ganancia en lazo abierto/cerrado y márgenes de estabilidad a 1×/2×/4×. El filtro de 2,5 kHz no es una característica verificada del B-18N: conservarlo solo provisionalmente.
- Actualizar identificador de algoritmo y golden files afectados, **después** de medir el cambio. No ocultar fallos ampliando umbrales.

**Crear test:** `crates/b18n-dsp/tests/nfb_rate_invariance.rs`. Estimular seno pequeño y barrido hasta zona alta audible; desfasar/compensar latencia antes de comparar respuestas 1×/2×/4×. Los objetivos de dispersión son **criterios de ingeniería provisionales**, no medidas de hardware (por ejemplo, buscar ≤0,5 dB en banda útil excluyendo zonas próximas a Nyquist, luego ajustar conforme al modelo).

**Aceptación:** polo de NFB comparable entre factores; `nfb_stability` sin NaN ni pérdidas de convergencia; diferencia audible documentada; tests a las cuatro sample rates; no valores hardcodeados por modo para aparentar paridad.

---

## B18-003 — Integrar el acoplo 0,022 µF del segundo 6SL7 en el MNA del tono

**Prioridad:** P0 · **Clase:** defecto de topología/atenuación constatado.\
**Archivos:** `crates/b18n-dsp/src/engine.rs` (`ChannelChain`, `new`, `process`, alrededor de líneas 502–513, 580–620, 680–712), `crates/b18n-dsp/src/stages/tone_stack.rs` (`ToneStackDesign::frequency_response`, `ToneStackProcessor`, nodos y stamp MNA), `crates/b18n-dsp/src/numerics/discretization.rs`, `crates/b18n-dsp/tests/tone_stack_response.rs`, `scripts/spice/b18n_tone_1967.cir`, `scripts/spice/tone_stack_sweep.py`.

**Problema:** `engine.rs` aplica al camino previo al tono:

```rust
let coupling_2_total = output_impedance_2 + tone_series_ohms;
let coupling_2_gain = tone_series_ohms / coupling_2_total;
// Y luego ToneStackProcessor::new(..., output_impedance_2, ...)
```

`ToneStackProcessor` **ya** incluye `output_impedance_2` como resistencia de fuente de su MNA, y el resistor de 220 kΩ está estampado dentro de la red. El divisor externo provoca una segunda pérdida; el capacitor externo, con corte fijo, ignora la carga dependiente de las posiciones Bass/Treble.

**Diseño propuesto (no aplicar un mero “quitar gain”):**

1. Ampliar el MNA de tono con un nodo fuente `NODE_SOURCE` y conservar `NODE_IN` al lado de la red pasiva. **Una sola** resistencia de Thevenin `Ro_6SL7` entre `NODE_SOURCE` y el origen de referencia de tensión. **Un solo** capacitor de acoplo de `22e-9 F` **entre** `NODE_SOURCE` y `NODE_IN`.
2. En tiempo real, estampar la fuente como Norton `I=Vsource/Ro` y `G=1/Ro` en `NODE_SOURCE`; estampar el capacitor con `TptCapacitor::conductance()` y `companion_current()`; conservar el resto del netlist James/Baxandall en sus nodos físicos. **No** colocar simultáneamente la fuente Norton en `NODE_IN`.
3. En el análisis de pequeña señal (`frequency_response`), introducir el mismo nodo y capacitor con admitancia `jωC`; compartir un constructor de topología/constantes entre el cálculo AC y el temporal cuando sea posible.
4. En `ChannelChain`, eliminar `coupling_2: OnePoleHighPass` y `coupling_2_gain`; transmitir el voltaje **AC** de placa al nuevo MNA. El capacitor debe inicializar su estado coherentemente con la tensión DC del segundo triodo y el voltaje DC nulo de la entrada del tone stack. Asegurar que reset, clone y `restore_from()` preserven la capacidad de buffers sin asignación.
5. Asegurar que el test SPICE incluya **la fuente de placa + Ro + C de 0,022 µF + tone stack + carga**; conservar por separado el fixture del tone stack aislado existente (su comparación no debe romperse porque se haya cambiado la definición de la prueba).
6. Para el anterior `ToneStackDesign::frequency_response(...)`, mantener una API `tone-only` compatible o crear `frequency_response_with_input_coupling(...)` explícita, sin cambiar en silencio el significado de fixtures antiguos.

**Pseudocódigo de estampado orientativo** (adaptar a las APIs reales de `RealMna`/`ComplexMna`):

```rust
// Una sola fuente Thevenin, Norton equivalente:
mna.add_conductance(NODE_SOURCE, GND, 1.0 / output_impedance_2);
mna.add_current_injection(NODE_SOURCE, plate_ac / output_impedance_2);
// Cac de 22 nF une SOURCE con IN (TPT en tiempo real):
stamp_capacitor_tpt(NODE_SOURCE, NODE_IN, &mut coupling_cap, &mut mna);
// El resistor 220k ya existe: NODE_IN -> NODE_B.
```

**Aceptación:** igualdad con ngspice para magnitud **y fase** en múltiples combinaciones de mandos y frecuencias; continua bloqueada; cambio de potencia de graves coherente al mover controles; no doble divisor; invariancia respecto a tamaño de bloque y OS.

---

## B18-004 — Reparar tiempo de suavizado de Input/Output Trim

**Prioridad:** P0 · **Clase:** defecto de código constatado.\
**Archivo:** `crates/b18n-dsp/src/engine.rs`, `LinearChain::new()` aproximadamente líneas 329–330 y 396–397.\
**Tests:** crear `crates/b18n-dsp/tests/control_smoothing.rs` o extender pruebas de `engine.rs`.

**Problema:**

```rust
let smoothing_ms = CONTROL_SMOOTHING_MS * f32::from(spec.oversampling);
// SmoothedValue::new(..., processing_rate as f32, smoothing_ms)
```

`processing_rate` ya incluye OS. Por ello el smoothing nominal de 15 ms termina en 15/30/60 ms a 1×/2×/4×. En cambio, los potenciómetros de `ChannelChain` utilizan el tiempo constante, lo que agrava la inconsistencia.

**Cambio:**

```rust
let smoothing_ms = CONTROL_SMOOTHING_MS;
// o pasar directamente CONTROL_SMOOTHING_MS en los dos constructores.
```

**Test:** automatizar escalón 0→1 y medir el instante de 63,2 % del recorrido con `SmoothedValue::new(..., rate_effective, 15.0)` para todos los OS. Si se prueba `LinearChain` completo, separar el efecto del smoothing del tiempo de establecimiento de válvulas y filtros. Verificar también que `set_controls()` inicial sigue haciendo `snap` como está documentado.

**Aceptación:** mismos 15 ms físicos (tolerancia de una muestra de tasa efectiva) a 1×/2×/4×, sin romper presets/ID de parámetros.

---

## B18-005 — Resolver la impedancia dinámica del control de volumen y la rejilla del segundo 6SL7

**Prioridad:** P1 · **Clase:** mejora física / aproximación registrada.\
**Archivos:** `crates/b18n-dsp/src/engine.rs` (`ChannelChain::new`, `process`), `crates/b18n-dsp/src/stages/triode_6sl7.rs` (`TriodeStageConfig`, `NonlinearTriodeStage`), `crates/b18n-dsp/src/controls.rs` (`volume_fraction`), `crates/b18n-dsp/src/numerics/discretization.rs` y pruebas nuevas.

**Problema:** `triode_2_config.series_resistance_ohms = volume_pot_ohms * 0.25` es una **impedancia de Thevenin máxima fija de 250 kΩ**, independientemente de la posición del potenciómetro. `coupling_1_gain` y `volume_fraction` son ganancias independientes del nodo de rejilla y pueden ocultar interacciones de carga, capacitancias y conducción. La ley actual del volumen es una aproximación 10 % a media carrera, no una medida del potenciometro de 1967.

**Pasos:**

1. Representar el pot de 1 MΩ mediante `Rtop(p)` y `Rbottom(p)` con ley logarítmica configurable/calibrable. Para una fuente idealizada, la impedancia de cursor en AC es `Rtop || Rbottom`, **pero** debe incluirse la impedancia de salida del primer triodo y la de rejilla; evitar usar solamente `Rpot*0.25`.
2. Acoplar la red fuente–C1–pot–rejilla al solve no lineal del segundo triodo cuando la rejilla conduce. Puede hacerse mediante nodo(s) MNA + ecuación no lineal de `Ig(Vgk)`; no recrear matrices por sample si su patrón es constante.
3. Inicializar condensadores en reposo DC y conservar el comportamiento de *blocking* al saturar. Modelar capacitancias parásitas solo con procedencia de datasheet o estimación explícita.
4. Mantener transitoriamente un modo de referencia `legacy` en tests para comparar antes/después; no añadir dos algoritmos permanentes sin justificación.

**Tests:** respuesta de salida/impedancia vista en el cursor para volumen 0/0.1/0.5/0.9/1, distintos niveles y condiciones de conducción; curvas ngspice equivalentes; monotonicidad de volumen en pequeña señal; no pops después de reset.

**Criterio:** los resistores del pot y de la rejilla aparecen una sola vez; curva de referencia reproducible; las desviaciones respecto del código antiguo se registran como cambio de algoritmo.

---

## B18-006 — Integrar mezcla de canales con impedancias reales

**Prioridad:** P1 · **Clase:** aproximación declarada.\
**Archivos:** `crates/b18n-dsp/src/stages/channel_mixer.rs`, `crates/b18n-dsp/src/engine.rs`, `crates/b18n-dsp/src/stages/phase_splitter.rs`, `crates/b18n-dsp/src/circuit.rs`, `crates/b18n-config/src/mapping.rs`, `scripts/spice/`.

**Estado:** `ChannelMixer::DEFAULT_GRID_LEAK_OHMS = 1_000_000.0` está **asumido**; el nodo suma `(v1+v2)` mediante un divisor fijo y luego un HP de 0,05 µF. Aunque ambos canales se ejecutan, las impedancias de sus salidas no están acopladas a la red de suma.

**Cambios:**

- Trazar en el esquema 5/67 resistor de mezcla 270 kΩ de cada canal, capacitor de 0,05 µF, entrada a 7199 y resistencias efectivas hacia tierra/bias. No trasladar los 10 MΩ/3,3 MΩ del triodo 7199 al pentodo por error.
- Sustituir `DEFAULT_GRID_LEAK_OHMS` por un parámetro trazado o un circuito explícito: `ChannelMixerConfig` con `Rmix1`, `Rmix2`, impedancias del tone stack de cada canal, `Ccoupling` y carga de rejilla no lineal si corresponde.
- Resolver un nodo físico compartido, no multiplicar por `node_gain` e inyectar de nuevo la carga equivalente en el pentodo. Coordinar interfaz con B18-008.
- Validar canal seleccionado y canal inactivo (en el amplificador real ambos permanecen eléctricamente presentes), potes extremos y respuesta en baja frecuencia.

**Aceptación:** ganancia y fase de suma frente a SPICE, incluyendo carga del canal inactivo. Valores de fuga trazados; no afirmar que 1 MΩ es el valor histórico si el esquema no lo demuestra.

---

## B18-007 — Diferenciar físicamente las entradas Bass/Guitar/Instrument

**Prioridad:** P1 · **Clase:** función expuesta que no está modelada.\
**Archivos:** `crates/b18n-dsp/src/engine.rs` (`InputChannel::Bass | InputChannel::Guitar` agrupados, aprox. línea 470), `crates/b18n-dsp/src/stages/input.rs`, `crates/b18n-dsp/src/stages/triode_6sl7.rs`, `crates/b18n-dsp/src/circuit.rs` (`InputNetworkValues`), `crates/b18n-config/src/mapping.rs`, `crates/b18n-plugin/src/params.rs`.

**Estado:** el enum ya identifica tres jacks pero Bass y Guitar siguen exactamente la misma ruta y `INPUT_SERIES_OHMS` es fijo a 120 kΩ. El propio `CIRCUIT_TRACEABILITY.md` marca la asignación jack↔resistencia como pendiente.

**Cambios:** digitalizar **contactos conmutados** de los jacks del plano 5/67 (resistencias 120 kΩ, 100 kΩ, fugas 5,6 MΩ y 2,2 MΩ en sus nodos correctos). Crear un `InputJackNetwork` por posición, preferiblemente solucionado como fuente de Thevenin (o MNA pequeño) y entregar `(v_grid, R_source)` a la primera 6SL7. No inventar una curva “bass boost” o “guitar boost” porque el nombre del jack sugiera una ecualización. Mantener IDs `bass`, `guitar`, `instrument` sin renombrarlos.

**Tests:** impedancia de entrada/fuente y transferencia por jack frente a SPICE en banda 20 Hz–20 kHz; límites de circuito abierto/corto; distintas impedancias de instrumento (p. ej. pasivo/activo como condiciones de ensayo, etiquetadas), 0/1/2 conectores si el modelo admite entrada simultánea.

**Aceptación:** jacks conectados a sus redes correctas, respaldadas con recorte/tabla de nodos; salida igual **solo si el circuito realmente lo determina**.

---

## B18-008 — Acoplar dinámicamente el inversor 7199 completo

**Prioridad:** P1 · **Clase:** aproximación del circuito.\
**Archivos:** `crates/b18n-dsp/src/stages/phase_splitter.rs` (config, `new`, `process`, `solve_pentode`, `solve_triode`), `crates/b18n-dsp/src/numerics/mna.rs`, `crates/b18n-dsp/src/numerics/discretization.rs`, `crates/b18n-dsp/tests/phase_inverter_ac.rs`, `scripts/spice/b18n_7199_{dc,ac,tran}_1967.cir`, `scripts/spice/b18n_7199_meter_loading.cir`.

**Topología confirmada por el trazado del repo:** pentodo 7199 con placa 270 kΩ y alimentación ~380 V, pantalla por 470 kΩ + 150 kΩ a masa, cátodo 1,5 kΩ con retorno de NFB 22 kΩ desde toma 16 Ω; acoplo placa de pentodo→rejilla triodo de 0,02 µF; triodo split-load con 47 kΩ/47 kΩ y red 10 MΩ entre placa y rejilla y 3,3 MΩ rejilla–masa. La salida de placa/cátodo del triodo va por dos C de 0,05 µF a 7027A.

**Simplificación detectada:** `PhaseInverter7199::new()` reduce la red de 10 MΩ/3,3 MΩ/capacitor a un HP con conductancia calculada con `alpha` **constante**. `process()` calcula `grid_v = operating_point.triode_grid_v + grid_ac` y resuelve el triodo sin realimentar dinámicamente el potencial de la placa a su rejilla mediante la red completa. Las cargas AC de rejilla también son aproximadas.

**Implementación recomendada:**

1. Incluir nodo real de rejilla del triodo, capacitor de 0,02 µF TPT y resistencias 10 MΩ/3,3 MΩ en el solve acoplado; placa del triodo debe realimentar al nodo de rejilla en **la misma muestra**, no mediante un `alpha` congelado en reposo.
2. Resolver conjuntamente las ecuaciones de tensión/corriente de rejilla, placa y cátodo del triodo. Inicializar en DC **con** la carga de medida del voltímetro de 20.000 Ω/V solo cuando el protocolo simule esa lectura, no en la simulación sonora.
3. Mantener 7199 pentodo y triodo como modelos de electrodos diferentes; no reusar indistintamente el modelo 6SL7. Conservar `Newton` con jacobiano analítico y ruta segura para no convergencia, sin asignación RT.
4. Agregar capacitancias de interelectrodo documentadas de la 7199 si el propósito es capturar polos HF y margen de NFB; no inventar compensación extra al mismo tiempo.

**Tests:** tensión DC nodos V3 y lectura cargada de pin 9; AC/phase en ambos brazos y su balance; `.tran` con drive pequeño/grande; `nfb_stability` con el nuevo acoplo.

**Aceptación:** coincidencia contra SPICE del **circuito ampliado**; no usar un golden de la simplificación vieja como única referencia.

---

## B18-009 — Cerrar la NFB sobre red eléctrica real, sin retardo artificial

**Prioridad:** P1 · **Clase:** fidelidad / estabilidad numérica; **no** arreglar apresuradamente.\
**Archivos:** `crates/b18n-dsp/src/engine.rs` (`feedback_state`, `feedback_polarity`, `feedback_lowpass`), `crates/b18n-dsp/src/stages/phase_splitter.rs` (`feedback_v` y corriente en cátodo), `crates/b18n-dsp/src/stages/power_7027a.rs`, `crates/b18n-dsp/src/stages/output_transformer.rs`, nuevos `stages/output_network.rs`/`numerics/coupled_solver.rs` si conviene.

**Estado:** el modelo actual implementa un lazo causal con realimentación de la salida secundaria **anterior** (`feedback_state`) y un LP de 2,5 kHz para estabilizar el cálculo. El circuito físico retroalimenta la tensión de la toma **16 Ω GREEN** al cátodo del pentodo por **22 kΩ**. El delay y el LP no constan como componentes en el plano 5/67.

**Objetivo:** modelar la tensión de secundario y el retorno en una ecuación eléctrica acoplada y estable; conservar la red física de 22 kΩ. La fase HF debe surgir principalmente de capacitancias de válvulas, transformador y carga, y no exclusivamente de un filtro de compensación escogido por conveniencia numérica.

**Método:** empezar con `B18-002` y conservar implementación anterior como referencia. Tras implementar B18-008, 013 y 015, calcular un equilibrio implícito por muestra de las variables críticas del lazo, o usar un esquema WDF/MNA con estados causales físicos. Comparar AC del lazo abierto, margen de ganancia/fase y respuesta al escalón en SPICE. Solo retirar `z^-1`/LP cuando el sistema nuevo conserve estabilidad demostrada en todas las tasas y extremos.

**Aceptación:** lazo estable para carga nominal, 16 Ω y modelos reactivos; sin oscilación subsonora ni alias HF; respuesta de lazo documentada y coherente con datos disponibles. No declarar “idéntico al hardware” sin medir ganancia/margen del hardware.

---

## B18-010 — Ajustar válvula 7027A desde datos RCA, no solo 6L6GC

**Prioridad:** P1 · **Clase:** calibración de no linealidad.\
**Archivos:** `assets/models/b18n_1967_calibration.json`, `scripts/fit_triode_model.py` (no reutilizar sin adaptar), **crear** `scripts/fit_7027a_model.py`, `crates/b18n-dsp/src/stages/tube_models.rs`, `crates/b18n-dsp/src/stages/power_7027a.rs`, pruebas nuevas `crates/b18n-dsp/tests/power_7027a_curves.rs`.

**Estado:** el overlay define `tube_models.7027a.koren.*` con parámetros de **6L6GC**, documentados explícitamente. Esto no es un bug del parser pero no valida la curva de un 7027A.

**Trabajo:**

1. Digitalizar curvas y puntos RCA 7027-A (según sean disponibles), registrar condiciones `Va`, `Vg2`, `Vg1`, temperatura/operación y tolerancias de lectura. No derivar valores absolutos solo de una curva escaneada sin registrar la incertidumbre.
2. Ajustar parámetros Koren con optimización robusta en múltiples puntos, poniendo mayor peso a reposo y zona usada en el B18N (B+ nominal 505 V, pantallas nominales 500 V, bias −55 V). Incluir medidas separadas para corriente de pantalla si están publicadas.
3. Ajustar **familias** de curvas, transconductancia y dinámica de corte; limitar parámetros físicamente y evaluar generalización fuera de los puntos de fit.
4. Guardar `origin: datasheet-derived-fit` **como etiqueta en método/source**, pero conservar `origin: estimated` para parámetros inferidos; reservar `datasheet` para valores explícitos de la hoja. No llamar “medido” a un fit a curvas publicadas.
5. Registrar corriente de reposo modelada como salida del solver, compararla con límites de placa/pantalla; la corriente real por válvula del espécimen queda desconocida hasta medición.

**Aceptación:** curvas Ip–Va y Ig2–Va comparadas con RCA y con error visible; los modos de placa/corte no se desestabilizan; no se incrementan fallos Newton; test de bias y salida 8 Ω.

---

## B18-011 — Modelo 7027A: dependencia de corriente de pantalla respecto a Vplaca

**Prioridad:** P1 · **Clase:** limitación física de ecuación.\
**Archivos:** `crates/b18n-dsp/src/stages/tube_models.rs` (aprox. `screen_current`, `currents_with_slopes`, `PentodeCurrents`), `crates/b18n-dsp/src/stages/power_7027a.rs` (`solve_plates`, `last_screen_current_a`), `crates/b18n-dsp/src/stages/phase_splitter.rs` si se comparte interfaz de pentodo 7199, `assets/models/b18n_1967_calibration.json`.

**Estado:** `KorenPentodeModel::screen_current(screen_v, grid_v)` depende de pantalla y rejilla, **no de placa**. En la región de rodilla y saturación de potencia real puede cambiar la distribución de corriente placa↔pantalla. El `solve_plates()` de `power_7027a.rs` calcula `last_screen_current_a` usando solo pantalla y rejilla. Es una aproximación reconocible en el código.

**Refactor propuesta:**

```rust
// API propuesta; requiere derivadas congruentes:
fn electrode_currents(&self, vp_k: f64, vs_k: f64, vg_k: f64) -> ElectrodeCurrents;
// ElectrodeCurrents: Ip, Ig2, Ig1, y jacobianos dI/dVp, dI/dVs, dI/dVg.
```

- Utilizar una extensión Koren/curva complementaria de pantalla **calibrada**, no una regla arbitraria `Ig2 += k*Ip`.
- Cambiar las firmas en todos los consumidores, incluidos preamplificador 7199 y tests, solo si comparten adecuadamente la nueva semántica.
- En `PushPullStage`, resolver placa/pantalla en el mismo sistema si aparece dependencia mutua o si las pantallas se alimentan de fuente dinámica B18-012. Comprobar jacobiano analítico contra diferencias finitas fuera de discontinuidades.
- Asegurar corrientes no negativas y potencia de disipación finita en condiciones normales; manejar con cuidado límites de corte/rodilla sin clamping que falsee audio.

**Aceptación:** validación en zonas AB1, inicio de conducción de rejilla y rodilla; corriente de pantalla compatible con referencia RCA; convergencia para entradas extremas que ya cubre `power_7027a.rs`.

---

## B18-012 — Modelar fuente 5AR4/GZ34 + PT-12XA + condensadores, resistencias y bias

**Prioridad:** P1 · **Clase:** subsistema ausente.\
**Archivos:** **crear** `crates/b18n-dsp/src/stages/power_supply.rs`, opcional `crates/b18n-dsp/src/stages/rectifier_5ar4.rs`, tests `crates/b18n-dsp/tests/supply_dynamics.rs`; modificar `crates/b18n-dsp/src/stages/mod.rs`, `engine.rs`, `power_7027a.rs`, `phase_splitter.rs`, `triode_6sl7.rs`, `circuit.rs`, `crates/b18n-config/src/{schema,mapping,validation}.rs`, `scripts/spice/`.

**Estado:** `PowerStageConfig` mantiene `supply_v=505`, `screen_supply_v=500` y `bias_v=-55` constantes. No hay simulación de carga-descarga de electrolíticos ni de pérdidas de rectificador/transformador. Los valores del JSON (30/40/40/40 µF; 1k, 10k, 22k, etc.) son una **lista**, no un netlist: reconstruir los nodos desde el esquema antes de codificar.

**Arquitectura orientativa:**

```rust
struct SupplyState {
    bplus_ct_v: f64,
    screen_rail_v: f64,
    preamp_rail_v: f64,
    inverter_rail_v: f64,
    bias_rail_v: f64,
    // estados de cada condensador y, si se modela, del rectificador
}
struct SupplyLoad {
    plate_current_a: [f64; 2],
    screen_current_a: [f64; 2],
    preamp_current_a: f64,
    inverter_current_a: f64,
}
fn advance_supply(state: &mut SupplyState, load: SupplyLoad, dt: f64);
```

**Tareas:**

1. Dibujar netlist topológico con nodos B+ y las **distintas** alimentaciones del pentodo 7199 (~380 V) y triodo inversor (~460 V), no reducir todas a un carril común. Conectar rectificador de onda completa, secundario PT-12XA de 800 Vac **extremo a extremo con toma central**, resistencias de cobre/serie donde estén documentadas, 5AR4, resistencias y C de filtro (40/40/40/30 µF en topología real), línea de bias −55 V y ruta de corriente de pantalla.
2. Empezar con una fuente RC de voltaje medio y rectificación aproximada **señalada como aproximación**; luego implementar conducción de rectificador con la curva Mullard GZ34 y pérdidas del PT **solo** al disponer de parámetros/cotas.
3. Avanzar estados de filtro a tasa efectiva (o con integración multirate justificada) y cerrar demanda de corriente de todas las etapas. Evitar usar corrientes de muestras futuras sin resolver el acoplamiento; si se usa estado atrasado, cuantificar error respecto a un solve implícito.
4. Iniciar en reposo mediante solve DC en vez de cargar condensadores con 0 V al hacer `reset()`; modelar *warmup* únicamente si se necesita y puede calibrarse.
5. Calcular B+ y corriente dinámica ante ráfagas graves; medir caída, tiempo de recuperación y cambio THD. **No** ajustar sag para que “suene vintage” sin relacionarlo con el circuito.
6. El bias DC nominal −55 V no debe perderse al pasar a una fuente dinámica. Distinguir voltaje de bias del ripple y de modulación por carga.

**Tests:** corriente cero, corriente nominal, ráfaga 40/60/100 Hz, escalón de corriente, distintos niveles, estabilidad f64, consistencia energía en RC, comparación con ngspice y límites de hoja GZ34. Sin valores arbitrarios de sag en el criterio de fidelidad histórica.

**Aceptación:** B+ se modifica coherentemente con carga y vuelve a reposo; potencias y railes no son imposibles; las corrientes de 7027A modifican fuente y viceversa; no se producen resets con golpes; CPU/latencia documentados.

---

## B18-013 — Transformador OT-471 lineal con pérdidas y parásitos

**Prioridad:** P1 · **Clase:** subsistema idealizado.\
**Archivos:** `crates/b18n-dsp/src/stages/output_transformer.rs` (actualmente modelo ideal alternativo, no asumir que la etapa principal lo invoque), `crates/b18n-dsp/src/stages/power_7027a.rs` (donde **realmente** se calcula la tensión secundaria), `engine.rs`, `circuit.rs`, `crates/b18n-config/src/{mapping,schema,validation}.rs`, **crear** `crates/b18n-dsp/tests/output_transformer_ac.rs` y `scripts/spice/b18n_ot471_linear.cir`.

**Valores con trazabilidad actual:** OT-471, 4300 Ω placa–placa, 8/16 Ω, primario DC 300 Ω, DCR secundarios en JSON y relación N8≈23,18; `Lm`, inductancia de fuga, capacitancias, histéresis y saturación siguen `null`.

**Riesgo de diseño:** `output_transformer.rs` implementa un convertidor ideal con DCR, **pero el motor actual instancia `PushPullStage` y calcula `secondary` en `power_7027a.rs`**; cambiar únicamente `output_transformer.rs` no modifica el camino de audio principal. Consolidar primero para que haya **una sola** implementación activa.

**Cambios:**

1. Crear `OutputTransformerModel` con componentes configurables: primario CT, resistencias en cada media bobina, `Lm` en paralelo en magnetización, `Lleak` serie, capacitancias parásitas, secundarios de 8/16 Ω y acoplamiento. Mantener modo `ideal` como prueba de regresión y fallback explícito.
2. Reemplazar las ecuaciones algebraicas idealizadas en `PushPullStage::solve_plates()` por un acoplamiento MNA/WDF con estados inductivos/capacitivos; las corrientes por media bobina deben incluir el efecto de magnetización para no atribuirlo falsamente a la corriente de placa.
3. Recalcular la tensión de la toma 16 Ω a partir del mismo modelo (para B18-009) y entregar la tensión a la carga desde el secundario 8 Ω sin un factor de ganancia duplicado.
4. Identificar `Lm`, `Lleak`, `Cp` por medidas de transformador fuera de circuito, por datasheet de modelo equivalente **etiquetado como estimado** o por ajuste AC de equipo real. No introducir cifras supuestamente históricas por intuición.
5. Sustituir gradualmente `OT_DC_BLOCK_CUTOFF_HZ = 1.0` en `power_7027a.rs` por el bloqueo DC emergente de la red. Retirar el HP artificial **solo** cuando la etapa nueva lo haga físicamente y no introduzca offset.

**Aceptación:** transferencia con N real y DCR correcta; atenuación baja frecuencia gobernada por Lm, agudos por fuga/C, polaridad/tomas verificadas, respuesta a carga resistiva y reactiva contrastada con SPICE. El transformador sin datos medidos queda rotulado `estimated`.

---

## B18-014 — Saturación e histéresis OT-471 (opcional hasta caracterizar)

**Prioridad:** P2 · **Clase:** mejora no lineal sin datos únicos.\
**Archivos:** `crates/b18n-dsp/src/stages/output_transformer.rs` o **crear** `crates/b18n-dsp/src/stages/magnetic_core.rs`; tests `ot_core_saturation.rs`; overlay de calibración, scripts de identificación.

**Implementación:** usar variable de flujo integrada a partir de voltaje de bobina e introducir ley `H(B)`/histéresis con memoria, preferiblemente modelo energético estable (e.g. Jiles–Atherton o Preisach reducido) **solo si** existen datos suficientes para calibrarlo. Mantener una alternativa sin histéresis con saturación suave calibrada, rotulada `estimated`, como experimento. Considerar pérdidas y asimetría DC del push-pull. Separar no linealidad de núcleo de la compresión de 7027A y de la fuente.

**Tests:** barridos 30–200 Hz a múltiples niveles, THD vs potencia, limitación de flujo, convergencia y balance de energía, comparación con un transformador real. **Bloqueo de ticket:** no fijar parámetros del OT-471 sin evidencia, salvo preset experimental marcado como tal.

---

## B18-015 — Carga eléctrica reactiva de altavoz de 18"

**Prioridad:** P1 · **Clase:** física ausente.\
**Archivos:** `crates/b18n-dsp/src/stages/speaker.rs`, `power_7027a.rs`, `output_transformer.rs`, `engine.rs`, `circuit.rs`, config `schema/mapping`; **crear** `stages/speaker_impedance.rs`, `tests/reactive_load.rs`, `scripts/spice/b18n_speaker_load.cir`.

**Estado:** `speaker.nominal_load_ohms=8` determina la carga constante de `PushPullStage`. Los perfiles `SpeakerCabinet` son filtros **post-amplificador**, por lo que no afectan a la corriente, transformador, NFB ni potencia.

**Cambios:**

1. Crear interfaz `ElectricalLoad` con `Resistive8Ohm`, `Resistive16Ohm`, `MeasuredImpedance` y, opcionalmente, `EquivalentLumpedSpeaker`. Carga dinámica representada por red RLC e inductancia de bobina/mecánica equivalente.
2. Conectar dicha carga al secundario del **mismo** OT en el solve (no filtrar el WAV de salida para simular la carga). Evaluar cómo cambia tensión/corriente de secundario y fase de NFB.
3. No asumir impedancia de 8 Ω constante a todas las frecuencias. Las curvas medidas o parámetros Thiele–Small del Cleveland concreto no están aún disponibles: usar carga resistiva como fallback y declarar cualquier aproximación RLC.
4. Separar carga física de render acústico (B18-016). La elección de IR/micrófono no tiene que modificar automáticamente la impedancia sin una asignación explícita de altavoz asociado.

**Tests:** carga 8/16 Ω, barrido complejo `Z(f)`, resonancia del equivalente RLC, fase corriente/tensión, estabilidad NFB y energía. El modo Direct debe poder elegir si representa salida de potencia con carga resistiva o reactiva, sin aplicar IR.

**Aceptación:** corriente de placas y secundarios responde a `Z(f)`; sin inestabilidad; respuesta eléctrica validada contra SPICE y, al disponer de ella, medición.

---

## B18-016 — Gabinete B-18N y reproducción acústica calibrada

**Prioridad:** P1 · **Clase:** sustitución de aproximación acústica.\
**Archivos:** `crates/b18n-dsp/src/stages/speaker.rs`, `crates/b18n-dsp/src/engine.rs`, `crates/b18n-dsp/src/controls.rs`, `crates/b18n-plugin/src/params.rs`, `assets/ir/README.md`; **crear** `stages/convolution.rs`, `assets/ir/metadata.schema.json`, `tests/speaker_ir.rs`.

**Estado:** perfil 18" = HP 40 Hz, peak +2 dB 80 Hz, LP 3,2 kHz; perfil 15" = HP 50 Hz, peak +2 dB 110 Hz, LP 4,2 kHz. **Son elecciones DSP modernas, no medidas** del Portaflex ni de un Cleveland. La cabina acústica no puede deducirse íntegramente del esquema eléctrico.

**Tareas:**

1. Adquirir una IR original con información de **altavoz concreto**, gabinete, micrófono, posición, distancia, tasa y condiciones de grabación. Verificar derechos de distribución. Si no existe, mantener `18 inch (approx.)` como fallback, no etiquetarlo como IR auténtica.
2. Crear convolución particionada FFT **sin allocs en audio**, buffers precreados, interpolación entre IRs y alineación de latencia. La IR es render acústico; **no** reemplaza modelo eléctrico de B18-015.
3. Mantener `Direct`, `18 inch (approx.)`, `15 inch (approx.)` y añadir nuevo modo con ID estable como `18inch_measured`, sin reordenar IDs serializados ni romper presets. Actualizar `Latency` si la convolución añade muestras.
4. Corregir ganancia post IR usando calibración DAW, no modificar el nivel eléctrico interno para forzar loudness; ofrecer nivel de salida y compensación transparente.
5. Si se modela altavoz mecánico con no linealidad, debe ser otro módulo calibrable, no confundirlo con una IR lineal.

**Tests:** delta→IR idéntica, convolución offline vs streaming, paso y cancelación con Direct, cambios de perfil sin clicks, IR vacía/incorrecta, latencia reportada, mediciones 20–10.000 Hz y presupuesto CPU.

**Aceptación:** perfiles estimados identificados claramente; IR trazada si existe; el mismo preset produce audio reproducible y no reconfigura accidentalmente la carga reactiva.

---

## B18-017 — Calibración de niveles, headroom y potencia nominal

**Prioridad:** P1 · **Clase:** precisión y UX audio.\
**Archivos:** `crates/b18n-dsp/src/stages/input.rs` (`VPK_PER_FULL_SCALE = 1.0`), `engine.rs` (`input_gain`/`output_gain`), `crates/b18n-plugin/src/{processor,params}.rs`, `crates/b18n-render/src/{main,report,sweep}.rs`, `docs/MODEL_ASSUMPTIONS.md`; **crear** `tests/level_calibration.rs`.

**Estado:** el software asume que muestra digital `1.0` equivale a `1,0 Vp` pre-trim y un trim de salida por defecto −16,5 dB por protección del host. Son **convenciones modernas**, no datos Ampeg. La potencia nominal indicada en la documentación histórica es 50 W, pero no constituye una garantía de que el modelo actual entregue 50 W con un valor concreto de entrada.

**Tareas:**

1. Documentar tres dominios: `input_dBFS → input_volts`, circuito eléctrico en voltios/corrientes, `speaker_voltage_or_mic_pressure → output_dBFS`. La conversión del último depende de si se usa salida Direct o acústica; evitar usar la misma escala implícita para W eléctricos y audio de micrófono.
2. Para carga resistiva nominal, medir `P=Vrms²/R` **antes** del trim digital de salida; 50 W en 8 Ω serían 20 Vrms si se cumplieran exactamente las condiciones (valor derivado, no resultado medido). Anotar criterio de distorsión y frecuencia que determina la potencia.
3. Definir rutina de calibración con señales pequeñas y grandes (40/80/100/400/1.000 Hz) y controles normalizados. Marcar clipping interno de válvula distinto de clipping numérico a 0 dBFS.
4. No insertar limiter automático oculto. La adaptación al host se documenta y se prueba sin modificar los voltajes eléctricos.

**Aceptación:** misma potencia eléctrica simulada al variar `output_trim_db`; saturación de host separada de saturación de válvulas; no se pierden detalles de dinámica por normalizaciones internas.

---

## B18-018 — Matriz de pruebas E2E y rendimiento en DAW

**Prioridad:** P1 · **Clase:** calidad de release.\
**Archivos:** tests existentes en `crates/b18n-dsp/tests/`, `crates/b18n-plugin/tests/`, `crates/b18n-render/`, `.github/workflows/ci.yml`, `docs/MEASUREMENT_PROTOCOL.md`, `docs/DSP_OPTIMIZATION_RESULTS.md`; **crear** `docs/VALIDATION_MATRIX.md`.

**Batería obligatoria:**

| Categoría | Ensayo | Resultado esperado |
|---|---|---|
| DC | nodos 6SL7/7199/7027A | Dentro de tolerancias históricas y con instrumento modelado cuando corresponde |
| AC | barridos de 20 Hz a 20 kHz | Magnitud y fase frente a SPICE y hardware cuando exista |
| THD | nivel bajo/medio/alto, varias frecuencias | Espectro medido, no solo picos/RMS; sin alias inaceptable |
| NFB | lazo abierto/cerrado, 1×/2×/4× | Estabilidad, polos y respuesta consistentes |
| Potencia | 8 Ω/16 Ω y reactivo | V, I, W, disipaciones compatibles |
| Fuente | ráfagas y transitorios | Sag, rizado y recuperación coherentes |
| RT safety | procesar/reset/cambiar controles | Sin alloc, bloqueos, panic ni I/O en hilo audio |
| Latencia | impulso/decimación/bypass/IR | Latencia reportada = observada |
| CPU | 44.1/48/96/192 kHz mono/estéreo | Coste por bloque, margen vs deadline, máximo y p99 |
| DAW | varios hosts VST3 y Windows | Escaneo, state restore, cambio rate, automatización, bypass |

**Normas:**

- Conservar CSV históricos (`tone_stack_spice.csv`, fase 7199, etc.) y **versionar nuevos** fixtures cuando cambia el circuito. No sobreescribir evidencia independiente para que pase un test.
- Probar espectro con seno/graves pulsados y filtro de alias, evitando confundir armónicos verdaderos con alias plegado; tolerancias numéricas se especifican tras observar precisión SPICE/Rust y ruido de medición.
- Crear pruebas de estabilidad del solve no lineal y registrar `convergence_failures()` / `failure_breakdown()` por etapa en cada barrido. Un sample retenido por fallo Newton no se acepta como “estabilidad” si los fallos son frecuentes.
- Ejecutar `cargo test --workspace --release` además de debug; comparar resultados. Presupuesto CPU se evalúa **también en DAW**; los ns/frame del repositorio son observaciones de benchmark y no garantizan funcionamiento en otro PC.
- CI no puede probar sonido histórico sin referencias; usar tests automatizados como *regresión*, no certificación auditiva.

**Aceptación:** matriz con resultado `pass/fail/not-measured`, reportes por versión, pruebas de hot switching y bypass, ausencia de excepciones DAW.

---

## B18-019 — VST3, presets, latencia y publicación

**Prioridad:** P2 · **Clase:** integración de producto.\
**Archivos:** `crates/b18n-plugin/src/{lib,params,processor}.rs`, `crates/b18n-plugin/tests/`, `bundler.toml`, `README.md`, `CHANGELOG.md`, `docs/RELEASE_CHECKLIST.md`, `.github/workflows/ci.yml`.

**Trabajo:**

1. Ejecutar `cargo xtask bundle b18n-plugin --release`; probar paquete final con Steinberg VST3 Validator, y al menos un host objetivo real. No considerar aprobada la compatibilidad DAW solo porque `cargo test` pase.
2. Validar opciones OS 1×/2×/4× en tiempo de ejecución: el adaptador `B18nProcessor` mantiene motores separados y el plugin notifica latencias. Confirmar salto de latencia, crossfade/reinicio, compensación de bypass y paridad estado/sonido en DAW.
3. `ALGORITHM_VERSION` debe cambiar con modificaciones audibles, no con documentación. Preservar `#[id]` de parámetros y enums existentes; nuevos controles **solo** con IDs nuevos. Si varía significado físico de preset, agregar migración/versionado explícito y registro de hashes `model_sha256`, `errata_sha256`, `effective_sha256`.
4. Verificar licencias de NIH-plug/VST3, GPL-3.0-only y derechos de IR; el plugin es no oficial, no afiliado a Ampeg. Actualizar instrucciones Windows de instalación/ubicación de `.vst3`.

**Aceptación:** escaneo y ejecución sin incidencias conocidas; presets antiguos abren sin error (con aviso de cambio de modelo cuando corresponda), latencia y estado coherentes; checklist release firmado.

---

## B18-020 — Ruido, hum, dispersión y envejecimiento (no bloquear fidelidad básica)

**Prioridad:** P2 · **Clase:** accesorio/experimental.\
**Archivos:** posible `crates/b18n-dsp/src/stages/noise.rs`, `stages/power_supply.rs`, overlay de calibración, `docs/MODEL_ASSUMPTIONS.md`, `tests/noise_statistics.rs`.

**Recomendación:** no añadir zumbido de 50/60/100/120 Hz por estética. Si se incorpora, medir espectro, armónicos de fuente, ruido de preamp, ruido microfónico y dispersión de componentes en una unidad B-18N. Seed reproducible y control para desactivar todo ruido en modo medición. Evitar que la generación de ruido modifique la resolución DC/Newton de manera no controlada.

**Aceptación:** ruido calibrado y trazado, desactivable; cero determinista en tests; sin CPU notable ni alias, y sin atribuir a fábrica las desviaciones propias de un aparato envejecido.

---

## B18-021 — Trazabilidad, incertidumbre y documentación por etapa

**Prioridad:** P1 · **Clase:** gobernanza técnica.\
**Archivos:** `docs/CIRCUIT_TRACEABILITY.md`, `docs/MODEL_ASSUMPTIONS.md`, `docs/DSP_ARCHITECTURE.md`, `docs/MEASUREMENT_PROTOCOL.md`, `docs/sprint.md`, `assets/models/b18n_1967_calibration.json`, `assets/models/b18n_1967_historical_errata.json`, `crates/b18n-config/src/{validation,errata,calibration}.rs`, `README.md`.

**Reglas:**

- Todo resistor/capacitor reproducido del 5/67 debe llevar referencia a nodo y evidencia de esquema. Todo valor empírico debe indicar método, incertidumbre, fecha y fuente.
- Mantener `datasheet` para valores explícitos de hoja del fabricante, `estimated` para parámetros derivados/ajustados y `measured` solo para laboratorio/medición verificable. Cuando se ajuste una curva de datasheet, documentar **ambas** cosas en `method/source` y etiquetar como inferido lo que no sea literal.
- Exponer niveles separados: `topology-traced`, `DC-SPICE-validated`, `AC-SPICE-validated`, `nonlinear-datasheet-fit`, `measured-hardware`, `cabinet-measured`. Que la existencia de SPICE no eleve automáticamente la categoría a hardware-calibrated.
- Para valores históricos corregidos, preservar base canónica y overlay de errata; nunca editar el JSON solo para satisfacer Newton. Validar SHA y conflictos de nodo en CI.
- Documentar claramente `Direct` como salida electrónica digital de un modelo con carga eléctrica, no como micrófono de un gabinete. Las selecciones 15"/18" actuales son **aproximadas** hasta medición.

**Aceptación:** cualquier nuevo parámetro se puede rastrear a documento, medición o suposición; README declara qué se reproduce y qué no; sin referencias rotas ni tests que dependan de PDFs no redistribuibles.

---

## Plan de PRs / sprints sugerido

| PR | Tickets | Entregable único | No mezclar con |
|---|---|---|---|
| `fix/nfb-rate` | 001,002 | NFB invariante respecto al oversampling, tests + docs | Nuevo OT |
| `fix/trim-smoothing` | 004 | Constante física 15 ms | Cambios de controles históricos |
| `fix/tone-coupling-mna` | 003 | C2+tonestack MNA frente a SPICE | Refit 7027A |
| `feat/preamp-loading` | 005,006,007 | Entradas y carga entre stages trazadas | Gabinete |
| `feat/7199-dynamic-network` | 008 | 7199 con red AC completa, comparación SPICE | Nueva NFB implícita |
| `feat/7027a-datasheet-fit` | 010,011 | Calibración y corriente de pantalla | Sag/OT |
| `feat/power-supply` | 012 | Fuente RC + rectificador con parámetros trazados | Histéresis de OT |
| `feat/ot471-linear` | 013 | OT único, equivalente lineal validable | IR |
| `feat/reactive-speaker` | 015 | Carga dinámica + pruebas de estabilidad | Acústica posfiltro |
| `feat/nfb-closed-loop` | 009 | Sustituir NFB artificial por red cerrada | Nuevas válvulas |
| `feat/cabinet-and-calibration` | 016,017 | IR medida o fallback honesto, niveles | Noise |
| `test/release-qa` | 018,019,021 | Validator, reportes, CPU, docs y distribución | Cambios de carácter |
| `research/ot-hysteresis-noise` | 014,020 | Investigación parametrizada, **no obligatorio** | Releases sin calibrar |

### Criterio de bloqueo de cada PR

- **Bloquear** si falla el reposo DC relevante, aumenta el número de fallos Newton a nivel audible, se duplica algún componente o aparece asignación dentro de audio.
- **Bloquear** si las comparaciones con ngspice emplean parámetros distintos sin justificación o si se aprueban por mover tolerancias arbitrariamente.
- **Bloquear release** si la latencia reportada no corresponde a la observada, si no funcionan presets anteriores o si hay picos/clipping numérico por error de escala.
- **No bloquear desarrollo** por falta de curvas reales de gabinete/OT: conservar `null`/`estimated`, documentar incertidumbre y usar fallback transparente. **Sí bloquear la etiqueta de fidelidad “measured/hardware-validated”** si no existen mediciones.

## Comandos recomendados para el agente

```powershell
# Ejecutar en raíz del repositorio
rustc --version
cargo --version
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --workspace --release
cargo build --workspace --release
cargo xtask bundle b18n-plugin --release

# Baseline offline reproducible
cargo run -p b18n-render --release -- `
  --input assets/fixtures/signals/bass_test.wav `
  --output target/renders/b18n_baseline.wav `
  --sample-rate 48000 `
  --oversampling 4 `
  --channel bass `
  --cabinet off `
  --speaker direct `
  --measure tone `
  --report target/reports/b18n_baseline.json
```

**Nota:** la última orden sigue la sintaxis publicada en README del commit auditado. Si el CLI ha cambiado, comprobar `--help` en la versión local antes de ejecutar.

## Instrucciones listas para Codex u otro agente

> Trabaja en la rama que yo especifique del repositorio `D-vst-rs-b18n-1967-rs-vst3`. Lee primero `b18-fixes.md`, `docs/CIRCUIT_TRACEABILITY.md`, `docs/MODEL_ASSUMPTIONS.md` y las referencias citadas. Ejecuta el baseline antes de tocar el DSP. Aborda un ticket o un PR del plan por iteración; no combines cambios eléctricos independientes. Conserva los valores históricos en el JSON canónico y utiliza la errata/calibración para correcciones y supuestos trazados. Implementa las topologías con componentes estampados una sola vez; usa `f64` y asegura real-time safety. Añade golden de SPICE verdaderamente independiente, pruebas unitarias/regresión, reporte de CPU, fallos Newton y latencia. Si falta un parámetro histórico, deja `None`/`estimated` y solicita datos, nunca lo inventes como medido. Actualiza `ALGORITHM_VERSION` cuando cambie el sonido y mantén IDs de presets. Tras cada ticket, entrega: archivos cambiados, fórmula/netlist aplicado, test ejecutado, resultado antes/después, incertidumbres y puntos todavía bloqueados. No declares equivalencia acústica con el B-18N real hasta medir un espécimen.

## Matriz de nodos históricos que deben conservarse como anclas

| Sección | Ancla de la documentación del proyecto | Tipo de verificación |
|---|---|---|
| Primer triodo 6SL7 | placa 175 V; cátodo 2,4 V | DC sin señal |
| Segundo triodo 6SL7 | placa 185 V; cátodo 2,0 V | DC sin señal |
| 7199 pentodo | placa 105 V; pantalla 60 V; cátodo 1,8 V | DC/medidor |
| 7199 split-load | placa 360 V; cátodo 100 V | DC/medidor |
| 7199 rejilla triodo | manual 65 V, susceptible de carga de voltímetro | DC con **modelo de instrumento** |
| 7027A | B+ 505 V; pantallas ~500 V; bias nominal −55 V | nodos de alimentación; no confundir con corriente de reposo |
| OT-471 | 4,3 kΩ p-p; secundario 8/16 Ω | AC/topología, parámetros no caracterizados separados |
| Salida nominal | 50 W documentados para la unidad/serie | contrastar condiciones antes de convertir a criterio de test |

**Advertencia de precisión:** estas tensiones son lecturas de servicio/documentación utilizadas por el proyecto, no necesariamente las tensiones exactas que tendrá cualquier aparato de 1967. El voltímetro histórico tenía carga finita. Registrar rango, tolerancia y condiciones de red eléctrica para comparaciones serias.

## Resultado que debe perseguirse

La meta **no** es “un amplificador a válvulas con sonido aproximado a Ampeg”, sino una representación eléctrica cuya cadena de entrada, 6SL7, tone stack, 7199, 7027A, alimentación 5AR4, OT-471, NFB y carga sea internamente coherente, reproducible frente a SPICE y trazable al B-18N de mayo de 1967. La acústica del Portaflex exige evidencia adicional: mediciones del altavoz/gabinete e IR con metadatos. Hasta entonces, etiquetar con precisión qué es físico, qué es derivado y qué es estimado.
