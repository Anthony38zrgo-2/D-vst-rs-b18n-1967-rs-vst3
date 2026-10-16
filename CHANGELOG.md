# Changelog

El formato sigue [Keep a Changelog](https://keepachangelog.com/es-ES/1.1.0/).
El versionado sigue SemVer una vez exista el primer bundle distribuible.

## [Unreleased]

### Investigación — B18-009 (2026-10-16, integración bloqueada)

- Candidato aislado `OutputNetwork`: solve conjunto 7199/7027A/OT/NFB,
  capacitancias RCA en el overlay, Jacobiano analítico y búsqueda de línea.
- Nueva referencia SPICE coherente en polaridad y puerto diferencial,
  comparación DC/AC y medición explícita de retorno/transitorio.
- **Bloqueo confirmado:** márgenes HF negativos (−55,38° nominal;
  −9,54° RLC), oscilación SPICE tras pulso de 10 µV, prueba Rust reactiva
  cerrada fallida y CPU candidata 32,487 µs frente a guardia de 32 µs.
- El candidato no está conectado al motor/plugin. Se conservan el lazo
  provisional, el fallback resistivo y la versión de algoritmo. Detalle y
  reproducción: `docs/B18_009_STATUS.md`. B18-009 **no se declara cerrado**.

### Añadido — B18-015 (carga reactiva del altavoz, física en el solve; NFB Reactiva bloqueada hasta B18-009)

- **Carga eléctrica real en el secundario del OT (no un filtro post).**
  `stages/speaker_impedance.rs` define `ElectricalLoadConfig`
  (`Resistive { ohms }` = fallback declarado con la 8 Ω del JSON, y
  `LumpedEquivalent(LumpedSpeakerParameters)`) y el estado discretizado
  `ElectricalLoad` (puerto Norton `i = G·v + Ieq`, TPT en toda la red).
  `OutputTransformerModel` resuelve la rama reflejada con la carga dinámica
  (DCR de secundario en serie, reflejo N², fuente Norton reflejada) y publica
  la tensión de terminal y la **corriente de secundario** del mismo avance.
- Parámetros `estimated` en el overlay con método y fecha
  (`speaker.reactive_load.*`, los cinco juntos o error explícito): red
  `Re=6.5 Ω + Le=0.8 mH + (Res=23.5 Ω ∥ Les=69.3 mH ∥ Ces=180.6 µF)` →
  fs=45 Hz, Q=1,2, pico |Z|≈30 Ω, |Z|(200 Hz)≈8,2 Ω. **No medido** del
  Cleveland; prioriza la regla «sin inventar Z(f)».
- Contraste independiente SPICE: golden
  `assets/fixtures/reference/speaker_load/speaker_load_spice.csv`
  (ngspice 47 + verificación analítica numérica; manifiesto con SHA-256).
  Motor vs golden: < 0,5 dB / < 10° en 20 Hz–20 kHz a 4×; transimpedancia
  `|v_sec/i_sec| = |Z(f)|` verificada en 45/200/1000 Hz.
- Selección por control moderno (`RuntimeControls::speaker_load`), con
  fallback documentado a la resistiva: si el overlay no trae parámetros **o
  la NFB provisional está activa** (la combinación con carga reactiva es
  inestable: ciclo límite ~24 kHz medido; cierre en B18-009),
  `effective_speaker_load()` informa de la activa. Exposición: parámetro del
  plugin `load_model` (Advanced; ID `nominal`/`rlc_estimated`) y
  `b18n-render --load resistive|lumped`.
- **El sonido por defecto no cambia**: con carga resistiva el camino reduce
  exactamente a las fórmulas de B18-013; render E2E igual al baseline
  (diferencia máxima 1,2e-7 por reorden aritmético) → sin `ALGORITHM_VERSION`.

### Corregido — B18-013 (OT-471 lineal con pérdidas y parásitos, cambio audible)

- **Una sola implementación del transformador.** `stages/output_transformer.rs`
  pasa a `OutputTransformerModel` (el `Ideal` del Hito C queda como
  regresión/fallback explícito) y `PushPullStage::solve_plates()` resuelve la
  **red diferencial** en el mismo Newton: `2·Rh` en serie con
  `R_core + (Lm ∥ (Lleak + N²·(Rcarga+DCR_sec))) ∥ Cp`. El secundario se acopla
  a la tensión del nodo interno F (EMF del flujo): la continua no se
  transfiere y el paso alto artificial de 1 Hz desaparece del modo lineal.
- Parámetros `estimated` (overlay, con método/fecha): `Lm = 28,7 H` (esquina
  baja a 25 Hz con la carga reflejada), `Lleak = 12 mH` (agudos ≈64 kHz),
  `Cp = 100 pF`, `R_core = 500 Ω` (pérdidas del camino de flujo, τ ≈ 57 ms) y
  cota provisional de flujo de 0,12 A (placeholder de saturación hasta
  B18-014). Integración: Euler atrás en los inductores (memoria acotada,
  robustez del solve; desviación HF declarada) y TPT en Cp.
- Solver de placas: rango `[−2·B, 3·B]` (flyback), paso ≤ 80 V, 200
  iteraciones y criterio de residuo 1 mV (≈ −100 dB). El test de estrés de
  alias (2 V/7,3 kHz, 4×) pasa de 121.364 fallos del primer prototipo a
  **0 fallos** 1×/2×/4×; cota absoluta de 1× re-basada a ≤ −12 dBc (medido
  −13,5; 4× −33,5).
- **Contraste SPICE independiente** (`scripts/spice/b18n_ot471_linear.cir`,
  `output_transformer_reference.py`, validado contra analítica < 1e-3):
  golden `assets/fixtures/reference/output_transformer/ot471_linear_spice.csv`
  (93 frecuencias) con README y manifiesto. Test nuevo
  `crates/b18n-dsp/tests/output_transformer_ac.rs`: peor desviación medida en
  20 Hz–20 kHz **0,091 dB / 0,97°** a la tasa 4×, esquina LF y polaridad.
- `ALGORITHM_VERSION` → `…-supply-ot471-linear-v1`; render E2E: pico 4,7902
  → 4,9170, RMS 1,4370 → 1,4761, offset +0,0029 → +0,0016, latencia 47 (sin
  cambio); CPU ≈27,2 µs/muestra (guardia 32 µs sin cambio).

### Corregido — B18-012 (fuente dinámica 5AR4 + sag, Hito D, cambio audible)

- **Los rieles dejan de ser rígidos.** Nuevo `stages/power_supply.rs`: red
  trazada SCH-567 (Hipótesis A, `estimated`) H2 [30 µF, CT/rectificador] → 1k →
  H1 [40 µF, pantallas] → 10k → H3 [40 µF, triodo] y 22k → H4 [40 µF, previo],
  más bias 10D6 + 100 µF → −55 V. Rectificador como Thevenin promedio
  (`Rrect = 150 Ω`, `estimated`; `Vsrc` calibrada a B+ 505 V con el reposo) y
  bias con `Rb = 15 kΩ` (`estimated`); sin curva 5AR4 ni DCR documentadas.
  Solve **implícito** (Euler backward 4×4 + bias 1D) por muestra a la tasa
  efectiva, sin asignaciones; reposo DC resuelto al inicializar (punto fijo
  rieles↔reposos en 20 iteradas, silencio sin transitorio).
- **Lazo de corriente cerrado para todas las etapas:** CT y pantallas 7027A,
  triodo inversor (47 kΩ), previo 6SL7 (resistencias de placa) y pentodo 7199
  (270 kΩ + 470 kΩ), más retornos de rejilla al bias. Los rieles de una muestra
  alimentan a la siguiente (`z^-1`); el error frente al solve simultáneo se
  cuantifica en el test (<0,05 V a 100 Hz con τ ≈ 22 ms >> dt).
- **Contraste SPICE independiente** (`scripts/spice/b18n_power_supply_1967.cir`,
  `power_supply_reference.py`): DC 505,0/495,1/472,1/393,9/−55,0 V (Rust ±2 V);
  escalón +20 mA en H2 con sag ≈ 2,9 V (Rust 1,5–4,5 V) y recuperación.
  Goldens en `assets/fixtures/reference/power_supply/` con README y manifiesto.
- Test nuevo `crates/b18n-dsp/tests/power_supply_transient.rs` (9 tests: cero/nominal,
  DC vs ngspice, escalón, ráfagas 40/60/100 Hz, energía RC, lag, reset,
  integración en motor con sag y recuperación). Reposo convergido del modelo:
  B+ 505, H1 495, H3 472, H4 394 V; CT 83 mA, pantallas 3 mA, triodo 2,3 mA,
  previo 4,6 mA.
- `ALGORITHM_VERSION` → `…-7027a-datasheet-supply-v1`; render E2E: pico 4,8224
  → 4,7902, RMS 1,4463 → 1,4370, latencia 47 (sin cambio); CPU ≈27–29,5
  µs/muestra, guardia 28 → **32 µs** (sobrecoste fuente ≈2–4 µs + varianza).

### Corregido — B18-010/011 (7027A desde el datasheet RCA, cambio audible)

- **La 7027A deja de ser un proxy 6L6GC.** Se ajustó el modelo Koren a las
  curvas digitalizadas del datasheet RCA 7027-A (8-59) y a sus tablas (clase A1
  y bias fija). Digitalización reproducible con `scripts/digitize_7027a_curves.py`
  (`assets/fixtures/reference/power_7027a/7027a_rca_digitized.csv`, 315 puntos
  con procedencia en el manifiesto) y ajuste con `scripts/fit_7027a_model.py`:
  `mu` 9,0 (cota: conducción en el reposo B-18N de 505/500/−55 V), `ex` 1,2983,
  `kg1` 1087,9, `kp` 42,41, `kvb` 38,35 V. Errores contra RCA: familia de placa
  p05 mediana 2,9 % (peor 23 %), transferencia p07 mediana 9 %; clase A1 Ip +15 %,
  gm +14 %, rp +27 %. Reposo del modelo: ≈44 mA/tubo (Pd ≈22 W).
- **Corriente de pantalla dependiente de la placa (B18-011).** La forma Koren
  `Ig2(Vs, Vg)` sin `Vp` no reproduce el datasheet (la tabla exige `ex`≈0,8 y la
  curva `ex`≈1,9); se calibró la **división de corriente**
  `Ig2 = Ip·(F + G·e^(−Vp/V0))` con `F`=0,0160, `G`=0,1425, `V0`=255 V: p07 Ig2
  mediana 7,8 % (peor 14 %), forma de p06 mediana 14 % (la joroba fina <70 V
  queda documentada). El `Max.-Signal Grid-No.2 Current` de la tabla es media de
  ciclo y no entra en el ajuste. Con `screen_ratio_floor = 0` (7199, respaldo
  6L6GC) el modelo de pantalla conserva la forma Koren publicada.
- **API del pentodo:** `screen_current(plate_v, screen_v, grid_v)` (antes sin
  `Vp`), nuevo `screen_current_with_slopes()` y `d_screen_d_plate` en
  `PentodeCurrents`; el Newton del 7199 no cambia (su floor es 0).
- Test nuevo: `crates/b18n-dsp/tests/power_7027a_curves.rs` (familia p05,
  transferencias p07, forma p06, anclas tabuladas y reposo del par con límites
  RCA). `ALGORITHM_VERSION` → `…-7199-coupled-speaker-7027a-datasheet-v1`;
  render E2E: pico 4,8155 → 4,8224, RMS 1,4433 → 1,4463, latencia 47 (sin
  cambio); CPU ≈25,2 µs/muestra (guardia 28, sin cambio).

### Corregido — B18-008 (acoplo dinámico del inversor 7199, cambio audible)

- **El acoplo pentodo→triodo se resuelve acoplado por muestra** (Newton 6D,
  `x = [Vs, Vc, Ip, Vg, It, Vk]` con jacobiano analítico): pantalla y cátodo del
  pentodo con su capacitor TPT, placa del pentodo implícita con su red real
  (270 kΩ + 0.02 µF), rejilla del triodo con la malla 10 MΩ/3.3 MΩ y la
  conducción de rejilla `Ig(Vgk)`, y triodo split-load con placa/cátodo
  implícitos en la corriente (la 10 MΩ carga la placa). La placa del triodo
  realimenta su rejilla **en la misma muestra**. La aproximación anterior (paso
  alto de un polo con `alpha` congelado y carga linealizada) queda solo como
  respaldo declarado si el solve acoplado no converge.
- **Contraste SPICE del circuito ampliado** (`b18n_7199_ac_1967.cir` ya incluía
  0.02 µF + 10 MΩ + 3.3 MΩ): `.ac` en 20 Hz–20 kHz con **≤0,0002 %** de
  magnitud y **≤0,05°** de fase (el test exige 0,1 % / 0,2°; antes 10 %/35° en
  LF); `.tran` 1 kHz/0,2 V con extremos al **0,03 %** (exige 0,5 %; antes 8 %).
  Se corrigió la convención del fasor del test (devolvía el conjugado, oculto
  por las tolerancias antiguas) y el asentamiento en LF (polo de ~3 Hz).
- Newton acoplado con residuo 1e-10 y paso 1e-8 (el capacitor de 0.02 µF
  integra el error de la rejilla/placa muestra a muestra) y refinado de la
  rejilla con la `Ig` convergida para eliminar la deriva de continua. El coste
  medido pasa de ~23,6 a ~25,2 µs/muestra (estéreo 4×) y la guardia de
  `cpu_budget` se actualiza a 28 µs con la justificación medida.
- `ALGORITHM_VERSION` → `…-preamp-loading-7199-coupled-speaker-v1`; render E2E:
  pico 4,8210 → 4,8155, RMS 1,4451 → 1,4433, offset +0,0028 → +0,0029,
  latencia 47 (sin cambio).

### Corregido — B18-005/006/007 (carga del previo: volumen, mezcla y jacks, cambio audible)

- **Red de volumen física (B18-005).** El motor aplicaba un paso alto externo, una
  ganancia de volumen y una serie fija de `1 MΩ × 0.25` al 6SL7#2. Ahora la rejilla
  del segundo triodo resuelve la red trazada: placa del 6SL7#1 detrás de su `Ro` →
  acoplo de 0.022 µF → potenciómetro 1 MΩ log **a masa** (Rtop–cursor–Rbottom) →
  rejilla, con la conducción de rejilla dentro del Newton. La carga DC es la sección
  inferior del pot (manual V1/pin 1: 0 Ω–1 MΩ). Golden ngspice en
  `assets/fixtures/reference/volume_network/` (0,02 dB / 0,2° a 100 Hz–10 kHz); el
  arranque del Newton en la solución lineal evita congelar la rejilla en pequeña señal
  y el refinado con la `Ig` convergida elimina la deriva del capacitor (residuo de
  silencio ~2e-14).
- **Mezclador físico sin fuga asumida (B18-006).** Trazado del raster + tabla del
  manual (V3/pin 7 = 220/330 kΩ según Bass): los dos 270 kΩ van **directos** al nodo
  de rejilla, sin 0.05 µF ni 1 MΩ (los 0.05 µF del esquema son los acoplos del
  inversor a las 7027A). El canal inactivo carga con su 270 kΩ (`(v1+v2)/2` con
  fuentes idealizadas; test `mixer_node_resistance_matches_manual_reading`:
  205 kΩ/302 kΩ dentro del ±10 %). La impedancia de salida del tono en serie con su
  270 kΩ queda para el nodo acoplado del inversor (B18-008).
- **Jacks de entrada diferenciados (B18-007).** CH1: 120 kΩ por jack y 5.6 MΩ de
  fuga; el jack `guitar` (conmutado) corta su punta a masa por 100 kΩ cuando está
  libre (`bass` = 211.8 kΩ, V1/pin 4 = 220 kΩ en el manual ±10 %); `guitar` sin
  shunt al insertar. CH2 (`instrument`): punta directa a la rejilla y 2.2 MΩ de fuga.
  `InputJackNetwork` por canal, conmutado en `set_controls`; golden ngspice en
  `assets/fixtures/reference/input_jacks/`. Discrepancia del manual en V2/pin 4
  registrada (fila duplicada de V1).
- `ALGORITHM_VERSION` →
  `…-tone-mna-preamp-loading-speaker-v1`; render E2E: pico 5,1420 → 4,8210,
  RMS 1,5610 → 1,4451, offset +0,0030 → +0,0028, latencia 47 (sin cambio).
  La instancia stock del plugin queda en 0,771 (−2,3 dBFS) con el trim −20,0 dB.

### Corregido — B18-003 (acoplo 0,022 µF integrado en el MNA del tono, cambio audible)

- **Doble atenuación del camino de tono eliminada.** El motor aplicaba un paso
  alto externo (0,022 µF) más un divisor `220k/(Ro+220k)` y además
  `ToneStackProcessor` estampaba `Ro` como fuente en su MNA: la fuente y la
  carga se contaban dos veces (~6 dB de pérdida espuria) y el corte del acoplo
  no seguía a las posiciones de Bass/Treble. Ahora el MNA tiene un nodo
  `SOURCE` (placa AC del 6SL7#2 detrás de `Ro`), **una sola** fuente de Thevenin
  y el capacitor de acoplo entre `SOURCE` e `IN`; el motor entrega la desviación
  AC de placa. El bloqueo de continua lo hace el propio capacitor.
- **`frequency_response_with_input_coupling`** (nueva API AC explícita) y golden
  SPICE del circuito completo (`scripts/spice/b18n_tone_coupled_1967.cir` +
  `tone_stack_coupled_sweep.py` → `tone_stack_coupled_spice.csv`): contraste de
  magnitud **y fase** a 1e-6 dB / 1e-4° en 25 combinaciones × 31 frecuencias.
  La API de tono aislado y su fixture se conservan sin cambios.
- **Tests nuevos**: bloqueo de continua del acoplo, equivalencia
  tasa×oversampling, respuesta temporal vs AC, direcciones de los mandos con
  acoplo, y el contraste ngspice acoplado.
- **Trim de salida por defecto del plugin: −16,5 → −20,0 dB.** La corrección
  sube la cadena ≈3,4 dB a 110/220 Hz; el nuevo valor devuelve la instancia
  por defecto a ≈0,82 (−1,7 dBFS) y mantiene la protección del host
  (`daw_levels`).
- **Test de alias recalibrado de 4 V a 2 V** (mismo drive interno tras quitar
  el doble atenuador): 1× −32,3 dBc y 4× −28,7 dBc, bajo la cota ≤ −18 dBc.
  Queda registrado que con ≥2,5 V/7,3 kHz el Newton del inversor 7199 congela
  samples; la robustez de esa etapa es alcance de B18-008.
- `ALGORITHM_VERSION` →
  `0.1.0-nonlinear-triodes-power-7199-inverter-nfb-25k-tpt-rate-2x-tone-mna-speaker-v1`;
  render E2E: pico 3,0776 → 5,1420, RMS 1,0571 → 1,5610, offset +0,0014 →
  +0,0030, latencia 47 (sin cambio).

### Corregido — B18-004 (tiempo de suavizado de los trims, sin cambio de sonido en régimen)

- **Trims de entrada/salida: 15 ms físicos en todos los factores.** El tiempo
  nominal se multiplicaba por el factor de sobremuestreo aunque los suavizados
  avanzan una vez por muestra de proceso, de modo que el escalón duraba
  15/30/60 ms a 1×/2×/4×; los potenciómetros de `ChannelChain` ya usaban el
  tiempo constante. No se modifica `ALGORITHM_VERSION`: la respuesta en régimen
  permanente no cambia (render E2E idéntico) y solo se corrige el tiempo de
  gesto.
- **`crates/b18n-dsp/tests/control_smoothing.rs` (nuevo):** mide el cruce del
  63,2 % del escalón de trim en la envolvente de salida en dB (15,75 ms
  medidos a 1×/2×/4×; con el código anterior la misma medida da 31,25 ms a 2×
  —≈61 ms a 4×, proporcional al factor—), verifica el contrato de
  `SmoothedValue` a las tasas efectivas y que la primera sincronización y
  `reset()` siguen haciendo `snap` sin rampa.

### Corregido — B18-002 (NFB a tasa efectiva + TPT, cambio audible)

- **Polo de compensación de la NFB: construido con la tasa base → construido
  con la tasa de proceso efectiva** (`sample_rate × oversampling`). El filtro
  integraba `oversampling` veces por muestra del host con coeficientes de la
  tasa base, de modo que el corte efectivo escalaba con el factor (≈5 kHz a 2×
  y ≈10 kHz a 4×, en lugar de 2,5 kHz). El comentario que afirmaba que la
  realimentación avanzaba una vez por muestra del host contradecía el flujo
  real.
- **Discretización del polo: Euler implícito (polo apareado) → TPT/bilineal con
  prewarp.** El corte físico de 2,5 kHz pasa a ser exacto a cualquier tasa
  (test unitario en `filters.rs`) y la fase en alta frecuencia se aproxima al
  RC analógico (a 20 kHz: −13° → −87° a 1×). Decisión registrada con su
  impacto medido: invariancia 1×/2×/4× de ≤0,64 dB/8,1° hasta 2,5 kHz y
  ≤2,85 dB/15,7° hasta 5 kHz (antes: 3,5 dB/20° y 7,5 dB/12° a 48 kHz).
- **`crates/b18n-dsp/tests/nfb_rate_invariance.rs` (nuevo):** senos pequeños a
  44,1/48/96/192 kHz, latencia del sobremuestreo compensada, dispersión máxima
  entre 1×/2×/4× con criterios provisionales documentados.
- **Test de alias re-basado** (`alias_suppression.rs`): la comparación relativa
  «4× mejora ≥5 dB» era un artefacto del polo dependiente del sobremuestreo.
  Con el polo unificado a 2,5 kHz (medido a 4 V): 1× mejora de −27,5 a
  −31,7 dBc; 4× pasa de −41,2 a −25,1 dBc (el 10 kHz efectivo anterior daba
  más realimentación de HF, inconsistente con 1×). Se conserva la cota
  absoluta ≤ −18 dBc en ambos factores y la pérdida de margen a 4× (alias
  dominado por intermodulación en banda imagen×armónico, no removible por
  decimación) queda registrada para B18-018.
- `ALGORITHM_VERSION` →
  `0.1.0-nonlinear-triodes-power-7199-inverter-nfb-25k-tpt-rate-2x-speaker-v1`;
  render E2E del baseline: pico 3,0838 → 3,0776, RMS 1,0592 → 1,0571,
  offset +0,00147 → +0,00140, latencia 47 (sin cambio).

### Añadido — B18-001 (línea base reproducible)

- **`docs/FIDELITY_BASELINE.md`**: commit, toolchain, hashes del modelo/errata/
  calibración efectiva, resultados de `fmt`/`clippy`/tests (151 en verde) y
  render offline del baseline, con las hipótesis no medidas declaradas.
- **`assets/fixtures/reference/manifest.json`** +
  **`scripts/check_reference_manifest.py`**: procedencia (`historical-manual`,
  `manufacturer-datasheet`, `independent-SPICE`, `real-hardware`, `model-only`)
  y SHA-256 de cada golden existente, verificables sin dependencias externas.
- **`assets/fixtures/reference/full_amp/README.md`**: reglas de procedencia y
  fixtures de cadena completa planificados (AC, DC, THD, transitorios, carga,
  IR).

### Añadido — Ola 8 (Fase 1: presupuesto de CPU del plugin, cambio audible)

- **`crates/b18n-plugin/tests/cpu_budget.rs`**: guardia de regresión del
  presupuesto de CPU (25 µs/muestra ≈1,2 núcleos a 48 kHz) más un bench
  ignorado con la cifra de la bitácora; mide el adaptador completo estéreo 4×
  con senos precomputados fuera del bucle medido.
- **`crates/b18n-plugin/tests/daw_levels.rs`**: la instancia nueva (mandos por
  defecto) no rebasa la escala completa del host y el solver no congela
  estado a niveles de DAW (0,2/1,0 V).
- **Derivadas analíticas del pentodo 7199** (`PentodeCurrents`,
  `KorenPentodeModel::currents_with_slopes`) para el Newton 3D del inversor
  sin jacobiano numérico; evaluaciones Koren con una sola exponencial y un
  solo `powf`.

### Corregido — Ola 8 (cambio audible)

- **Inversor 7199: bisección anidada (~5.500 evaluaciones Koren/muestra) →
  Newton 3D acoplado con jacobiano analítico** (~2-6 evaluaciones por muestra
  en el camino rápido), arranque cálido del sample anterior. Robustez
  declarada a rejilla positiva extrema: respaldo con bisección monótona de la
  corriente en el bracket físico de la red de placa `[0, A]` y Newton de
  reinicio; 0 fallos de congelación a 4 V de drive (antes fallaban ~578
  muestras del inversor y ~719 de triodos en la ventana de prueba).
- **Triodos 6SL7: Newton máx. 24 → 64 iteraciones.** El arrastre amortiguado
  cerca del corte (plegamiento del acoplo TPT) necesitaba ~26-40 iteraciones
  antes de la fase cuadrática y congelaba samples con drive extremo.
- **NFB compensación 20 kHz → 2,5 kHz de la tasa base.** Con el solver exacto
  del inversor el lazo idealizado era inestable a Nyquist (barrera
  estable/inestable medida: 4–5 kHz efectivos a 44,1/48 kHz, >5 kHz a
  96/192 kHz); además el corte anterior se construía con la tasa de proceso y
  variaba con el sobremuestreo. `ALGORITHM_VERSION` →
  `0.1.0-nonlinear-triodes-power-7199-inverter-nfb-25k-comp-4x`; render E2E
  nuevo: pico 3,084 (antes 3,858 con `nfb-4x`), offset +0,0015, latencia 47.
- **Nivel de instancia nueva:** con senos a escala completa la salida stock
  producía pico 5,57 (zumbe el host). Trim de salida por defecto **−16,5 dB**
  (no histórico; la instancia nueva queda ≈0,83, −1,6 dBFS).
- **Arranque sin rampa de mandos:** la primera `set_controls` de una
  instancia nueva salta directo a los objetivos (antes el suavizado de
  15 ms arrancaba a ganancia previa durante ~720 muestras); `reset()` mantiene
  los mandos ya sincronizados sin rampa.
- **Test de latencia re-anclado:** la latencia reportada sigue siendo 47
  muestras (impulso stock pica en 46, ±1). Con drive moderno (+6 dB) el ápice
  del transitorio no lineal resuelto desplaza el pico de la respuesta a 49:
  la latencia FIR no cambia; el criterio queda "adaptador ≡ motor directo
  sample-exact" por factor y "impulso stock dentro de ±1".
- **Sonda temporal `probe_failures.rs` eliminada** (diagnosticó el
  estancamiento del Newton del inversor: 32.768/32.768 fallos a 4 V).

### Añadido — Ola 7 (P1.7: plugin VST3, sin cambio audible)

- **`crates/b18n-plugin`**: adaptador VST3 con NIH-plug fijado a
  `de421011f41a6d10fc8c7a6084e4f4dee0143683` (GPL-3.0-only). Layouts 2→2 y
  1→1 (estéreo = dos motores independientes); sin MIDI; estado/presets con
  huellas del modelo persistidas (JSON canónico, errata, efectivo, revisión,
  versión de algoritmo).
- **Parámetros con IDs estables** (variantes de enum con `#[id]`): `channel`,
  `volume/bass/treble` 1 y 2, `input_trim`, `output_trim`, `bypass`
  (compensado en latencia) y `oversampling` 1×/4× en panel «Advanced»
  (8× no implementado: no se expone).
- **Bypass con compensación**: línea de retardo seco (47 muestras a 4×) y
  crossfade de 15 ms; `SAMPLE_ACCURATE_AUTOMATION` activo.
- **Tests nuevos (11)**: paridad sample-exact adaptador↔motor directo (1×/4×,
  mono/estéreo), invariancia de bloque 1/7/64/257/1024, latencia con impulso,
  no-alloc en `process`/`reset` y estabilidad de IDs. Total del workspace: 135.
- **Bundle**: `cargo xtask bundle b18n-plugin --release` genera
  `target/bundled/B18N 67.vst3` (clave `[b18n-plugin]` en `bundler.toml`).

### Corregido — Ola 7 (sin cambio audible)

- **Latencia del sobremuestreo 4×: 93 → 47 muestras.** El cálculo anterior
  contaba cada FIR de media banda al doble de su retardo real. Medición:
  pico del impulso 47, correlación cruzada de bursts 47, centro de masas
  46,28 (teórico 46,5). El render E2E no cambia (pico 3,858, offset −0,0207).
- **Solver Newton sin asignaciones dinámicas** (`numerics/nonlinear.rs`):
  const-genérico con memoria en pila; antes asignaba 5–6 `Vec` por muestra y
  violaba la regla del callback. `B18nEngine::reset()` ahora restaura estado
  reutilizando buffers (`restore_from` en MNA, tone stack y sobremuestreador)
  en lugar de clonar la cadena; verificado por el test no-alloc del plugin.

### Añadido — Ola 6 (NFB y memoria de capacitores, cambio audible)

- NFB: 22 kΩ desde la toma de 16 Ω al **cátodo del pentodo** (AN-183 + flag
  «F» del raster), lazo explícito `z^-1` con compensación declarada de 20 kHz
  (los parásitos de HF no están modelados; sin ella el lazo era inestable).
- Bloqueo de continua del OT (paso alto 1 Hz asumido): elimina offset y latch.
- Acoplo PI→7027A con **capacitores TPT** (blocking distortion real y
  recuperable); las salidas del inversor pasan a ser crudas.
- Robustez: bisección monótona en la placa del pentodo; bracket del cátodo
  ampliado para tensiones negativas.
- `nfb_stability.rs`: ganancia cerrada 0,14×, estabilidad 44,1–192 kHz × 8/16 Ω
  × drives hasta 4 V, decaimiento de ringing y recuperación del blocking.
- **`ALGORITHM_VERSION`**:
  `0.1.0-nonlinear-triodes-power-7199-inverter-nfb-4x`.
- Netlist SPICE `b18n_7199_nfb_dc_1967.cir` (reposo con la 22 kΩ).

### Corregido — Ola 5 (cierre `.ac`/`.tran` de P1.4, cambio audible)

- Modelo del inversor 7199: las llamadas Koren usaban tensiones referidas a
  masa; ahora usan placa-cátodo y pantalla-cátodo (reposo idéntico al golden
  SPICE). El acoplo pentodo→triodo incorpora la α del split-load y la carga
  real de la rama sobre la placa.
- `b18n_7199_ac_1967.cir` + `phase_inverter_reference.py` + golden AC y test
  `phase_inverter_ac.rs` (magnitud ≤1,5 % desde 200 Hz; fase ≤6,5° desde 1 kHz;
  LF documentado). `b18n_7199_tran_1967.cir` + golden y contraste de extremos
  al 8 %.
- FIR de media banda 31 → **63 taps** (latencia reportada **93 muestras**;
  corregida a **47** en la Ola 7 al medir el impulso); test de
  espurios de la ruta identidad; alias 4× recalibrado y limitación
  (intermodulación con imágenes del interpolador) documentada.
- **`ALGORITHM_VERSION`**:
  `0.1.0-nonlinear-triodes-power-7199-inverter-63tap-4x`.

### Añadido — Ola 4 (inversor 7199 real en el motor, cambio audible)

- `stages/phase_splitter.rs`: `PhaseInverter7199` (pentodo por bisección
  monótona con capacitor de pantalla TPT, triodo de carga dividida por
  bisección, malla 10 M/3.3 M resuelta en el reposo, acoplo AC como paso alto
  declarado, salidas 0.05 µF).
- `power_7027a::process_grids(arriba, abajo)`: la etapa de potencia recibe las
  dos salidas reales del split-load; `grid_source_ohms` sale del modelo del
  triodo.
- Errata: adición trazada `phase_inverter.triode_supply_v = 460` (carril del
  triodo inferido de `Vp+Vk=Vrail`); esquema, mapeo, validación y tests.
- `OperatingPoint.phase_inverter` con las seis tensiones; contraste `.op`
  contra el golden SPICE (triodo ≤3 %, pentodo ≤20 % documentado).
- **`ALGORITHM_VERSION`**: `0.1.0-nonlinear-triodes-power-7199-inverter-4x`.
- Test de alias recalibrado a la ganancia real (0.4 V): ≥8 dB de mejora 4× y
  alias ≤ −30 dBc; límite del FIR de 31 taps documentado.

### Añadido — Ola 3 (reposo de la 7199, carga del medidor y modelos RCA)

- Datasheets **RCA 7199 DATA 1/2 (7-63)** y **AN-183** incorporados localmente
  (ignorados por git) y `scripts/fit_7199_model.py`, que ajusta los modelos
  Koren del pentodo y del triodo a las anclas del boletín.
- **Topología DC de V3 resuelta**: 10 MΩ de la placa del triodo a su rejilla,
  3.3 MΩ de rejilla a masa; carril del triodo (~460 V) distinto del pentodo
  (~380 V). La lectura de 65 V en pin 9 queda explicada como lectura cargada
  por el medidor de 20 kΩ/V.
- `scripts/spice/b18n_7199_meter_loading.cir` con subckts Koren y variantes de
  medidor 1/2/5/10 MΩ; golden y tabla de contraste en
  `assets/fixtures/reference/phase_7199/`.
- Calibración: parámetros `tube_models.7199.pentode.koren.*` y
  `.triode.koren.*` (origen `datasheet`); `TubeModelValues` y el loader los
  exponen con fallo explícito si están incompletos.
- Residual documentado: el ajuste del pentodo sobreestima `Ip` ~8 % en pantalla
  baja (pin 2 queda ~16 % bajo el manual); no se retocan valores.

### Añadido — Ola 2 (cierre P0.2 y contraste SPICE del tone stack)

- `scripts/spice/b18n_7199_dc_1967.cir`: `R(pin3, masa)` con fuentes de DC
  anuladas. Variante apagada (carril de previo flotante) = **150 000 Ω**,
  coherente con `RESISTANCE READINGS` (V3/pin 3 = 150 kΩ); variante de
  contraste con carril referido a masa = 113.7 kΩ. Golden y README en
  `assets/fixtures/reference/phase_7199/`.
- Errata ampliada con `additions`: la **150 kΩ** de pantalla a masa entra en
  `CircuitDefinition` como `phase_inverter.additional_traced_resistors` (nodos
  y evidencia), con guarda de no-redefinición de nodos existentes y test que la
  cruza contra el golden SPICE. El JSON canónico sigue intacto.
- `scripts/spice/b18n_tone_1967.cir` (referencia legible) y
  `scripts/spice/tone_stack_sweep.py` (barrido 25×31 con ngspice 47): golden
  `assets/fixtures/reference/tone_stack/tone_stack_spice.csv`. SPICE–numpy
  difieren ≤ 1e-9 dB / 0°.
- Test Rust `matches_ngspice_reference` (1e-6 dB / 1e-4°) junto al contraste
  numpy existente.

### Añadido — Ola 1 (errata 270 kΩ/470 kΩ de la 7199 y verificación SPICE)

- `assets/models/b18n_1967_historical_errata.json`: corrección documentada del
  intercambio `pentode_plate_resistor_ohms`/`pentode_screen_feed_resistor_ohms`
  (pin 2 placa = 270 kΩ, pin 3 pantalla = 470 kΩ), con evidencia del raster
  SCH-567, pinout RCA 7199, nodos/carriles y contraste DC. El JSON canónico no
  se edita.
- `b18n-config`: módulo `errata` que aplica la corrección sobre el árbol JSON
  antes de construir `CircuitDefinition`; exige `base_sha256` y guarda
  anti-deriva (`json_value`), protege `identity`, y expone `sha256_hex`.
- Guarda de inmutabilidad: test que fija el SHA-256 canónico `d61d122d…`.
- Reporte `b18n-render`: `model.sha256` (entrada), `model.errata_sha256` y
  `model.effective_sha256` (huella efectiva del modelo).
- `scripts/spice/`: README de instalación de ngspice 47 (Windows x64) y runner
  `run_spice.py` (batch, log y detección de errores).

### Añadido — Hito C, parte 2 (etapa de potencia 7027A)

- Modelo Koren de pentodo (`KorenPentodeModel`) con corriente de placa con
  rodilla (`atan`), corriente de pantalla y conducción de rejilla suave;
  parámetros publicados de la 6L6GC (tubo padre de la 7027A) en la calibración,
  verificados contra el punto típico del datasheet.
- Etapa push-pull `stages/power_7027a.rs`: bias fija con acoplo que bloquea
  continua (reposo por el retorno de 100 kΩ a −55 V), stopper de 1 kΩ,
  impedancia de fuente del inversor documentada, OT ideal con toma central y
  DCR (`vp1±vp2`), carga de 8 Ω.
- Solver de rejilla por Newton con respaldo por bisección y solver de placas
  2×2 con jacobiano analítico; diagnóstico de corrientes de placa y pantalla.
- El reposo de la etapa se resuelve al inicializar y se registra en el estado
  (corriente de placa y pantalla del modelo; el JSON las deja `null`).
- Tests: punto típico 6L6GC, reposo simétrico y disipación, antisimetría de
  salida, transición a clase B con corte, límite por conducción de rejilla y
  robustez con excitación extrema.

### Añadido — Hito C, parte 1 (triodos no lineales + sobremuestreo)

- Modelo Koren del 6SL7 (`stages/tube_models.rs`) con parámetros del overlay de
  calibración embebido (estimados por ajuste documentado en
  `scripts/fit_triode_model.py`; anclas del JSON histórico).
- Conducción de rejilla con transición suave y resistencia equivalente
  declarada; evita rejillas positivas irreales y aporta compresión asimétrica.
- Newton-Raphson amortiguado (`numerics/nonlinear.rs`) con jacobiano analítico,
  cota de paso, retroceso de línea, tolerancia de residuo, límite de iteraciones
  y diagnóstico de fallos; el estado válido se conserva ante no convergencia.
- Etapas de triodo no lineales en la cadena con acoplos RC preseteados al
  reposo; el punto de reposo se resuelve al inicializar y se registra en el
  estado del motor.
- Sobremuestreo 4× con FIR de media banda (31 taps, Blackman) y latencia
  reportada (45 muestras); test de supresión de alias 1× vs 4×.
- `EngineError::MissingTubeModel` y `OperatingPointNotConverged`: sin modelo o
  sin reposo válido el motor falla explícitamente en vez de inventar valores.

### Añadido — Hito B (camino lineal verificable)

- `b18n-dsp/numerics`: MNA real reutilizable (sin asignaciones al resolver),
  MNA complejo para funciones de transferencia y capacitores TPT estables.
- Red de tono James/Baxandall trazada del esquema 5/67, con potenciómetros continuos,
  respuesta frecuencial y procesador temporal MNA/TPT sin asignaciones.
- Cadena lineal: entrada con trim, etapas 6SL7 de pequeña señal con acoplo RC, volumen
  con taper asumido, mezcla de canales 270 kΩ + 0.05 µF, inversor ideal, OT-471 ideal con
  DCR y carga de 8 Ω; suavizado de mandos de 15 ms.
- Referencia independiente numpy de 25 combinaciones × 31 frecuencias
  (`scripts/tone_stack_reference.py`, CSV versionado) y test de tolerancia 1e-6 dB/1e-4°.
- `b18n-render --measure tone`: escribe las curvas en CSV y resume los rangos en el reporte.
- Tests de determinismo, invariancia al tamaño de bloque (1/7/64/257/1024 en dsp y render)
  y direcciones de los mandos de tono.

### Añadido — Hito A (repositorio y modelo de datos)

- Workspace Cargo con los crates `b18n-dsp`, `b18n-config` y `b18n-render`, más `xtask`.
- Toolchain fijada en `rust-toolchain.toml` (1.95.0).
- JSON histórico copiado a `assets/models/` sin modificar claves, con SHA-256 documentado.
- Archivo de calibración separado de ejemplo.
- `b18n-config`: esquema Serde estricto, validación de revisión `1967-05`, mapeo a
  `CircuitDefinition` y loader embebido fuera del callback de audio.
- `b18n-dsp`: API pública (`B18nEngine`, `ProcessSpec`, `RuntimeControls`) con motor
  passthrough determinista; sin asignaciones en `process_mono`.
- `b18n-render`: CLI WAV -> WAV con reporte JSON (hash del modelo, métricas básicas).
- Documentación inicial: trazabilidad del circuito, supuestos, protocolo de medición,
  arquitectura DSP y checklist de release.
