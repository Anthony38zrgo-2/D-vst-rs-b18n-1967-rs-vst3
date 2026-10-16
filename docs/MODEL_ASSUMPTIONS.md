# Supuestos del modelo

**B18-009, actualización 2026-10-16:** las capacitancias RCA de 7199 y
7027A están trazadas en el overlay, pero solo las consume el candidato
aislado `OutputNetwork`. No se conectó al motor: márgenes negativos y
transitorios SPICE, más un caso Rust reactivo fallido, bloquean su integración.
La respuesta AC sin pico de la exploración anterior no prueba estabilidad.
Estado y límites completos: [`B18_009_STATUS.md`](B18_009_STATUS.md).

Este documento separa lo que proviene de documentación histórica de lo que es una decisión
de ingeniería moderna. Nada de la lista «moderno» se presenta como característica del
hardware de 1967.

## Escala y flujo

- `Vpk_per_full_scale = 1.0` (convención inicial): una muestra de amplitud `1.0` equivale a
  `1.0 V` de pico **antes** de `InputTrimDb`. Es una calibración de software documentada,
  no una medida histórica.
- `InputTrimDb` y `OutputTrimDb` son adaptaciones modernas DAW↔voltios. El **trim de
  salida por defecto del plugin es −20,0 dB** (Ola 8; recalibrado en B18-003 tras
  corregir el doble atenuador del tono, que sube la cadena ≈3,4 dB a 110/220 Hz): con
  senos de graves a escala completa la instancia nueva queda en ≈0,82 (−1,7 dBFS) y no
  rebasa la escala digital del host. Es una decisión de adaptación de nivel, no una
  característica del amplificador.
- Los mandos de una instancia nueva **saltan directo** a sus objetivos (`snap` en la
  primera configuración y tras `reset`): sin rampa de suavizado a ganancia previa al
  arranque. El suavizado de gestos es de **15 ms físicos** en todas las tasas y factores
  de sobremuestreo (B18-004: los trims multiplicaban el tiempo nominal por el factor de
  sobremuestreo y daban 30/60 ms a 2×/4×; potenciómetros y trims comparten ya la misma
  constante).
- El plugin es un **efecto de audio**; su salida es line-level digital, no la salida de
  potencia de un amplificador conectado a un altavoz real.
- Modo estéreo: dos instancias de motor independientes (una por canal). No se suma L+R por
  defecto.

## Cadena lineal (Hito B)

- **Entrada y jacks (B18-007):** cada canal lleva su red trazada del raster: CH1 con
  **120 kΩ** serie por jack y **5.6 MΩ** de fuga; el jack `guitar` (conmutado) corta su
  punta a masa por **100 kΩ** cuando está libre (V1/pin 4 = 220 kΩ en el manual ±10 %).
  CH2 (`instrument`) va directa a la rejilla con **2.2 MΩ** de fuga. No hay ecualización
  por nombre de jack. Contraste ngspice en
  `assets/fixtures/reference/input_jacks/`.
- **Red de volumen (B18-005):** placa del 6SL7#1 detrás de su `Ro` → acoplo de
  **0.022 µF** → potenciómetro **1 MΩ log a masa** (Rtop–cursor–Rbottom) → rejilla del
  6SL7#2. La carga DC de la rejilla es la sección inferior del pot (manual V1/pin 1:
  0 Ω–1 MΩ con el mando), sin fuga separada. El Newton del 6SL7#2 resuelve la malla con
  la conducción de rejilla; ley del mando `f(p) = (10^(2p) − 1)/99` (estimada, sustituible
  por una curva medida). Contraste ngspice en
  `assets/fixtures/reference/volume_network/`. La impedancia de salida de la red de tono
  en serie con su 270 kΩ queda pendiente del nodo acoplado del inversor (B18-008).
- **Mezcla de canales (B18-006):** dos resistores de **270 kΩ** a un nodo compartido que
  va directo a la rejilla del pentodo 7199 (sin 0.05 µF ni fuga de 1 MΩ; la evidencia es
  la tabla de resistencias del manual, V3/pin 7 = 220–330 kΩ según Bass). Con fuentes
  idealizadas el nodo vale `(v1+v2)/2` y el canal inactivo carga con su 270 kΩ.
- **Carga de la red de tono:** 270 kΩ (resistor de mezcla) para las curvas estándar.
- **Acoplo del tono (B18-003):** el capacitor de **0.022 µF** de la placa del 6SL7#2
  está integrado en el MNA del tono junto con la resistencia de fuente `Ro` (una sola
  fuente de Thevenin en el nodo `SOURCE`, capacitor hacia `IN`). El motor entrega la
  desviación AC de placa; la continua queda bloqueada por el propio capacitor y la
  atenuación de carga depende de las posiciones Bass/Treble. Antes existía un paso alto
  externo de corte fijo más un divisor con la misma `Ro`, que duplicaba la pérdida
  (~6 dB) y hacía que el corte no siguiera a la red. Contraste contra ngspice del
  circuito completo (fuente + `Ro` + 0.022 µF + red + carga) a 1e-6 dB / 1e-4° en
  `assets/fixtures/reference/tone_stack/tone_stack_coupled_spice.csv`.
- **Ganancia de la cadena:** con el inversor 7199 real (P1.4) la cadena tiene
  ~45× más ganancia que con el inversor ideal del Hito B; el control de salida
  permite ajustarla. Desde B18-013 el OT es la red con parásitos (véanse los
  dos puntos siguientes) y el bloqueo de continua es físico en el modo lineal.
- **Parasitismos del OT (B18-013):** `winding_inductance_h`,
  `leakage_inductance_h` y `parasitic_capacitance_f` siguen `null` en el JSON;
  el **overlay de calibración** los aporta como `estimated` con método y fecha:
  `Lm = 28,7 H` (inductancia diferencial placa-placa por esquina de baja
  frecuencia a 25 Hz con la carga reflejada `N8²·(8+0,4) ≈ 4514 Ω`),
  `Lleak = 12 mH` (esquina de agudos ≈64 kHz) y `Cp = 100 pF` (resonancia con
  `Lleak` ≈145 kHz; con la fuente pentodo estimada, polo ≈35 kHz). No son
  medidas del OT-471: son criterios de diseño documentados y sustituibles por
  una caracterización real sin cambiar la API.

## Válvulas (Hito C, parte 1)

- **Modelo Koren del 6SL7:** ecuaciones públicas (Norman Koren) con parámetros
  **estimados** por `scripts/fit_triode_model.py` a partir de las anclas del
  propio JSON (triodo 1: 175 V/2.4 V; triodo 2: 185 V/2.0 V; referencia de
  pequeña señal a 250 V con µ/gm/rp de datasheet). Los parámetros viven en
  `assets/models/b18n_1967_calibration.json` con método y fecha; no son
  mediciones propias ni curvas del hardware de 1967.
- **Conducción de rejilla:** aproximación con transición suave (softplus,
  0.08 V) y resistencia equivalente de rejilla declarada. No sustituye una
  curva de rejilla medida; su función principal es evitar tensiones de rejilla
  positivas irreales y aportar compresión asimétrica.
- **Etapa de potencia 7027A (B18-010/011, 2026-10-14):** modelo Koren de pentodo
  con corriente de placa y de pantalla y conducción de rejilla suave. Los
  parámetros ya no son el proxy 6L6GC: se ajustaron con
  `scripts/fit_7027a_model.py` a **curvas digitalizadas del datasheet RCA
  7027-A (8-59)** (`assets/fixtures/reference/power_7027a/7027a_rca_digitized.csv`,
  manifiesto de procedencia) y a sus **tablas** (clase A1 250/250/−14 V →
  72 mA/5 mA/gm 6000 µmhos/rp 22,5 kΩ; bias fija por tubo 400/300/−25 → 51 mA,
  450/350/−30 → 47,5 mA, 540/400/−38 → 50 mA). Resultado: p05 mediana 2,9 %
  (peor 23 %), p07 Ip mediana 9 % (peor 22 %), clase A1 Ip +15 %/gm +14 %;
  la tabla 450/350/−30 (+26 %) es no monótona en el propio datasheet.
- **División de pantalla de la 7027A (B18-011):** `Ig2 = Ip·(F + G·e^(−Vp/V0))`
  con `F` = 0,016, `G` = 0,142 y `V0` = 255 V, ajustada a la curva de pantalla
  de p07 (Vp=300 V: mediana 7,8 %), a la forma `Ig2(Vp)` de una curva sin
  etiqueta de p06 (subida en la rodilla; mediana 14 %, peor 41 % por debajo de
  ~140 V, donde el trazado digitalizado muestra una joroba que la forma
  exponencial no captura) y a las tablas de reposo (Ig2/Ip = 3,6–5,9 %). El
  `Max.-Signal Grid-No.2 Current` de la tabla es un **valor medio del ciclo**,
  no un punto: solo se usa como comprobación de orden. Con los parámetros
  ausentes (`screen_ratio_floor = 0`) el modelo de pantalla conserva la forma
  Koren publicada `(Vs/µ + Vg)^Ex/Kg2` (7199 y respaldo 6L6GC).
- **Cota de `mu` de la 7027A:** el ajuste querría `mu` ≈ 10,2, pero el reposo
  documentado del B-18N (pantalla ~500 V, bias −55 V) exige `Vs/µ > 55`
  (conducción) → se acota a 9,0; el coste es la dispersión de p07/p05 arriba
  citada. Reposo resultante del modelo: ≈44 mA/tubo (Pd ≈22 W < 35 W máximos de
  la ficha); sin medición hardware que lo confirme, queda como salida del solver.
- **Alimentación de la etapa de potencia (B18-012 resuelto, 2026-10-15):**
  fuente dinámica H2 [30 µF, CT] → 1k → H1 [40 µF, pantallas] → 10k → H3
  [40 µF, triodo] y 22k → H4 [40 µF, previo], más bias 10D6 + 100 µF → −55 V
  (Hipótesis A trazada del raster, `estimated`). Rectificador 5AR4 como
  Thevenin promedio (`Rrect = 150 Ω`, `estimated`; `Vsrc` calibrada a B+ 505 V
  con el reposo) y bias con `Rb = 15 kΩ` (`estimated`); sin curva 5AR4 ni DCR
  documentadas. Solve implícito por muestra a la tasa efectiva con `z^-1`;
  reposo convergido: B+ 505, H1 495, H3 472, H4 394 V. Contraste SPICE
  (`power_supply/`) y tests en `power_supply_transient.rs`.
- **Acoplo a las rejillas de la 7027A:** se modela el bloqueo de continua del
  acoplo de 0.05 µF (reposo fijado por el retorno de 100 kΩ a −55 V) y la
  excitación AC a través del stopper de 1 kΩ más una impedancia de fuente
  estimada del inversor (47 kΩ, documentada). La carga de bias de los retornos
  (`(Vbias−Vg)/100k` por tubo) modula el bias dentro del solve de la fuente.
- **Modelo Koren de la 7199 (P0.3 resuelto, 2026-10-09):** pentodo y triodo
  ajustados por `scripts/fit_7199_model.py` (scipy, `least_squares`) a las anclas
  del boletín **RCA 7199 DATA 1/2 (7-63)**; origen `datasheet` en la
  calibración. Pentodo: anclas (100 V/50 V/−1.1 V → 1.1 mA, 0.35 mA de pantalla)
  y (220 V/130 V/−0.775 V → 12.5 mA, 3.5 mA), corte a −4 V y gm/rp del punto A1;
  el ajuste sobreestima Ip un ~8 % en el punto del B-18N (pantalla 60 V,
  extrapolación bajo el ancla más próxima) → documentado, sin retocar valores.
  Triodo: µ=17 (boletín) con `kvb` fijado a 160 (el único punto de placa del
  boletín no determina la dependencia con Vp; valores mayores disparan la
  extrapolación a Vp alto); reproduce Vp/Vk del manual dentro del 3 %.
- **Topología DC de V3 (trazada y verificada con SPICE):** placa del pentodo
  270 kΩ y pantalla 470 kΩ + 150 kΩ + 0.1 µF (cátodo), cátodo 1.5 kΩ; acoplo
  0.02 µF a la rejilla del triodo; **10 MΩ de la placa del triodo a la rejilla**
  y **3.3 MΩ a masa**; split-load 47 kΩ/47 kΩ. Carril del pentodo ~380 V
  (nodo de previo) y del triodo ~460 V (nodo distinto, inferido de
  `Vp+Vk=Vrail`). La lectura de 65 V en pin 9 es la lectura cargada por un
  medidor de 20 kΩ/V (rango 250 V) de un nodo de ~88 V con fuente ~2.5 MΩ.
  Golden: `assets/fixtures/reference/phase_7199/`.
- **Inversor 7199 en el motor (P1.4; solve acoplado desde B18-008):** el
  pentodo y la red de acoplo se resuelven **conjuntamente** por **Newton 6D**
  (`x = [Vs, Vc, Ip, Vg, It, Vk]`, jacobiano analítico) por muestra: pantalla y
  cátodo del pentodo con el capacitor TPT de 0.1 µF, placa del pentodo implícita
  en `Ip` con su red real (270 kΩ + capacitor de acoplo de 0.02 µF), **rejilla
  del triodo con la malla 10 MΩ/3.3 MΩ, la conducción de rejilla `Ig(Vgk)` y la
  placa del triodo realimentando en la misma muestra** (placa y cátodo
  split-load implícitos en `It`, con la corriente de la 10 MΩ cargando la
  placa). Respaldo declarado si no converge: rejilla lineal + Newton 3D del
  pentodo con la carga real + Newton 1D del triodo (la aproximación de un polo
  de la Ola 8), conservando el último estado válido en el peor caso. Los
  modelos reciben tensiones **referidas al cátodo**. Las salidas 0.05 µF
  alimentan `process_grids`. Contraste SPICE del **circuito ampliado**
  (`b18n_7199_ac_1967.cir` incluye 0.02 µF + 10 MΩ + 3.3 MΩ): `.op` del pentodo
  idéntico y triodo dentro del 0,3 %; `.ac` magnitud ≤0,0002 % y fase ≤0,05° en
  20 Hz–20 kHz (test exige 0,1 %/0,2°); `.tran` de extremos al 0,03 % (test
  exige 0,5 %). Simplificación declarada: el solve **no modela la conducción de
  rejilla del pentodo 7199** (la rejilla es fuente ideal para el motor); solo
  importa a drive de entrada extremo y queda documentado.
- **Alias con la cadena real:** el FIR de media banda se subió a **63 taps**
  (latencia **47 muestras** ≈1 ms; corregida en P1.7 con la respuesta al
  impulso, el cálculo anterior de 93 duplicaba el retardo por etapa) y su ruta identidad es limpia (espurios
  < −300 dB). Con la cadena del inversor, el alias 4× está dominado por
  **intermodulación entre las imágenes del interpolador y los armónicos** del
  pentodo (medido ≈−22 dBc a 1,5 V). Tras B18-002 el test exige cota absoluta
  ≤ −18 dBc en 1× y 4×, y B18-003 recalibró la excitación de 4 V a 2 V (el acoplo
  integrado elimina ~6 dB de atenuación doble): medido a 2 V, −32,3 y −28,7 dBc; la
  comparación relativa «+5 dB a favor de 4×» era un artefacto del polo de NFB
  dependiente del sobremuestreo. Un interpolador de mayor atenuación queda como
  mejora documentada (B18-018).
- **NFB (P1.6, implementada; polo corregido en B18-002):** 22 kΩ
  desde la toma de 16 Ω al **cátodo del pentodo** (topología del AN-183 y del
  flag «F» del raster; DC consistente con pin 6 = 1,8 V). Separación **explícita
  `z^-1`** (aproximación declarada): la muestra de la toma se inyecta un sample
  después, **de la tasa de proceso efectiva** (`tasa base × oversampling`). El
  modelo no incluye los parásitos de HF del tubo/transformador que limitan la
  ganancia de lazo en el amplificador real, así que el camino de realimentación
  lleva un **polo de compensación a 2,5 kHz** (declarado, **provisional**: no es
  una característica verificada del B-18N). Desde B18-002 el polo se construye
  con la **tasa de proceso efectiva** y con discretización **TPT/bilineal con
  prewarp** (magnitud exacta −3 dB en 2,5 kHz a cualquier tasa, verificada en
  `filters.rs`); antes se construía con la tasa base mientras el filtro
  integraba `oversampling` veces por muestra, de modo que el corte efectivo
  escalaba con el factor (≈5 kHz a 2×, ≈10 kHz a 4×) y su fase en HF se alejaba
  del RC analógico (Euler: −13° a 20 kHz a 1×; TPT: −87°). Invariancia medida
  entre 1×/2×/4× (`nfb_rate_invariance.rs`, senos pequeños, latencia
  compensada): dispersión ≤0,64 dB/8,1° hasta 2,5 kHz y ≤2,85 dB/15,7° hasta
  5 kHz (antes: 3,5 dB/20° y 7,5 dB/12° a 48 kHz). El residuo en 4–5 kHz está
  dominado por el `z^-1` de una muestra de proceso (mayor a 1×; se retira en
  B18-009) y por la discretización de los acoplos. Cambio audible registrado
  con `ALGORITHM_VERSION` (`…-nfb-25k-tpt-rate-2x-speaker-v1`): render E2E del
  baseline pico 3,0838 → 3,0776, RMS 1,0592 → 1,0571, offset +0,00147 →
  +0,00140. La polaridad del arrollamiento de 16 Ω no está documentada: se
  asume la que produce realimentación negativa (verificada por la reducción de
  ganancia ≈7× y la ausencia de oscilación sostenida en `nfb_stability.rs` a
  44,1–192 kHz, cargas 8/16 Ω y drives hasta 4 V). El margen formal de lazo
  abierto queda pendiente de B18-008/B18-009.
- **Bloqueo de continua del OT (B18-013):** el transformador pasa a ser una
  red con estados. En el modo lineal el bloqueo es físico: el secundario se
  acopla a la tensión del nodo interno F (el EMF del flujo) y la corriente
  continua diferencial circula por `Lm`/`R_core`, no por la carga; el paso
  alto de 1 Hz queda solo en el modo ideal (fallback). El integrador de flujo
  se acota con la resistencia de pérdidas `R_core = 500 Ω` (τ = Lm/R_core ≈
  57 ms; < 0,1 dB en la esquina de 25 Hz) y con una **cota provisional de
  corriente de magnetización de 0,12 A** (1,3× el pico a plena potencia a
  40 Hz) declarada como *limitador numérico*, no como curva de saturación —
  la saturación/histéresis del núcleo es B18-014. Los inductores se integran
  con **Euler atrás** (memoria acotada; la desviación de fase HF es declarada
  y medida contra SPICE: ≤0,091 dB / 0,97° en 20 Hz–20 kHz a la tasa 4×) y
  `Cp` con TPT. Contraste del circuito completo en
  `assets/fixtures/reference/output_transformer/` (ngspice 47 + analítica
  independiente) y `crates/b18n-dsp/tests/output_transformer_ac.rs`.
- **Memoria de acoplo PI→7027A:** los 0.05 µF son capacitores TPT por rejilla
  con el retorno de 100 kΩ al bias y conducción de rejilla: el *blocking
  distortion* es real y se recupera (test `blocking_recovers_after_overdrive`).
- **Pendiente:** las mejoras de P2 restantes (OT no ideal: saturación B18-014;
  carga reactiva B18-015; cierre NFB B18-009). El plugin VST3 (P1.7) está
  implementado y empaquetado; quedan como pasos manuales el Steinberg
  Validator y la prueba en DAW.
- **Sobremuestreo:** 4× con FIR de media banda (63 taps, Blackman) diseñados al
  inicializar; latencia de **47 muestras** reportada al host (verificada con
  impulso en P1.7). A excitación extrema quedan productos internos de orden muy
  alto dentro del dominio de 192 kHz; 8× no está implementado y no se expone en
  el plugin (queda como opción documentada si se necesita más margen).

## Válvulas (notas generales)

- Punto de partida de pequeña señal 6SL7 en `250 V`: `mu≈70`, `rp≈44 kΩ`, `gm≈1.6 mS`,
  `Ia≈2.3 mA` (referencia local del JSON). No caracteriza saturación.
- Las curvas de 6SL7, 7199 y 7027A **no** están en el JSON (`null`). Mientras no existan
  modelos contrastados con procedencia/licencia, el motor no debe fingir una validación.
- `plate_current_idle_a_each` y `screen_current_idle_a_each` son `null`: el reposo se
  resolverá con el modelo y se etiquetará como `approximate` hasta contrastarlo.

## Estado inicial

- Los condensadores no se cargan con tensiones nominales «a mano». El reposo se resuelve al
  inicializar hasta convergencia; si falla, el motor conserva el último estado válido y
  activa una bandera de diagnóstico.

## Niveles de fidelidad

`topology_unverified` → `approximate` → `calibrated` → `measured`. El nivel debe aparecer en
los reportes por etapa. Un preset agradable no asciende de nivel por sí solo.

## Extensiones modernas

- Control `Sag`: si se implementa, es extensión moderna, nunca mando original.
- `cabinet_enabled` e IR externa: opcionales, identificados como tales.
- Cualquier atajo de *clipping* de salida para protección es moderno y debe documentarse.

## Speaker profiles and 2x (current iteration)

The inactive-preamp suspension from optimization round 2 has been removed at the user's request. Both preamp channels always run; all other validated optimizations remain.

Modern acoustic profiles are explicitly **estimated voicings**, not historical measurements. The nominal amplifier load remains 8 ohms in both modes; no Thiele-Small parameters, impedance curve, cabinet geometry or measured IR are inferred from diameter.

| Selector | High-pass (2nd order) | Bass peak (Q 0.7071) | Low-pass (2nd order) |
| --- | --- | --- | --- |
| Direct (default) | None | None | None |
| 18 inch (approx.) | 40 Hz | +2 dB at 80 Hz | 3200 Hz |
| 15 inch (approx.) | 50 Hz | +2 dB at 110 Hz | 4200 Hz |

High/low-pass Q is 0.7071. Cutoffs are limited to 45% of sample rate for low-rate stability. Filters run after the amplifier at the base sample rate and have no added buffering latency. Both profiles keep their histories warm; selection blends over a 15 ms exponential time constant. Direct remains the default so existing sessions retain their previous amp output until a speaker profile is selected. This does not imply a measured difference attributable solely to speaker diameter.

Oversampling choices are 1x / 2x / 4x. The 2x path uses one 63-tap half-band interpolation/decimation pair; its measured impulse peak and reported latency are 31 base-rate samples (0.646 ms at 48 kHz). 1x remains 0; 4x remains 47 samples. All three stereo engines are allocated at initialization; factor changes and reset allocate nothing in the audio callback. 8x is rejected consistently rather than advertised by ProcessSpec.

The oversampling enum is now ordered 1x, 2x, 4x. Stable preset IDs 1x and 4x are retained; their normalized endpoint values remain 0 and 1, while the new midpoint selects 2x. Raw numeric enum index for 4x changes from 1 to 2. The new speaker parameter ID is speaker, with direct / 18inch / 15inch variant IDs.

Offline renderer: `--oversampling 2 --speaker 18` (or `15`, or `direct`). Speaker selection is included in the JSON report. Audible algorithm version is updated consistently in plugin and renderer.

## Carga eléctrica del altavoz (B18-015)

Dos modelos de carga separados de la coloración acústica:

1. **Resistencia nominal** (`cabinet.nominal_load_ohms = 8 Ω`, JSON). Es el
   **fallback declarado** y la carga por defecto en todos los caminos: sin
   curva de impedancia medida no se sustituye la 8 Ω documentada.
2. **Equivalente RLC estimado** del altavoz de 18" (overlay
   `speaker.reactive_load.*`, `estimated`, con método y fecha): red en
   movilidad eléctrica `Re + Le + (Res ∥ Les ∥ Ces)` con
   `Re=6.5 Ω` (≈0,8×Znom), `Le=0.8 mH` (esquina 1,3 kHz; típica declarada),
   pico `|Z(fs)| = 30 Ω a fs = 45 Hz` con `Q = 1,2` (típico de un 8 Ω de
   graves; **no** medido del Cleveland). Anclas: |Z|(200 Hz) ≈ 8,2 Ω,
   |Z|(1 kHz) ≈ 7,7 Ω, |Z|(20 kHz) ≈ 100 Ω. Regenerable/regresable con
   medición futura.

- La carga entra en la rama reflejada del solve del OT como **puerto Norton**
  (`i = G·v + Ieq`), no filtrando el WAV de salida. Discretizada toda con
  **TPT/trapezoidal**: la red tiene camino resistivo (`Re`, `Res`), no es un
  integrador puro como el flujo del OT (que usa Euler atrás por robustez); con
  Euler atrás la desviación frente al golden SPICE crecía a ≈1,8 dB en
  20 kHz (carga inductiva ≈100 Ω, ωT ya grande a 4×).
- Contraste SPICE: `assets/fixtures/reference/speaker_load/`
  (ngspice 47 sobre `scripts/spice/b18n_speaker_load.cir`; generador con
  comprobación analítica independiente). Motor vs golden: < 0,5 dB / < 10°
  en 20 Hz–20 kHz a 4×.
- La corriente de secundario sale del mismo avance (`secondary_current_a`); a
  la misma excitación, el pico de resonancia reduce la corriente frente a la
  zona nominal (v_i_sec coherente con Z(f)).
- Selección (`RuntimeControls::speaker_load`): `NominalResistance` (por
  defecto) o `EstimatedLumped`. Sin parámetros en el overlay, la selección
  estimada cae a la nominal (documentado; no se rellena una Z(f)). Con la
  **NFB provisional activa** también cae (bloqueo del lazo, ver abajo).
- **Bloqueo NFB (abierto, mide B18-009):** con el equivalente estimado y la
  NFB provisional activa (`z^-1` + polo 2,5 kHz), el lazo pierde todo el
  margen y entra en un ciclo límite sostenido (medido; ver el archivo de
  problemas en `instrucciones.md`, sección B18-015). Regla vigente: con la
  **NFB activa** la selección estimada cae a la resistencia nominal
  (`effective_speaker_load()` lo expone y hay test que lo fija). El modo
  reactivo se habilita solo con la NFB abierta del circuito
  (`negative_feedback.present = false`), que es la puerta por la que B18-009
  reabrirá el lazo físico. Ni el polo a 500 Hz/1,5 kHz ni cascadas de dos
  polos (2,5 kHz/900/600 Hz) hacen robusta la combinación (los impulsos
  fuertes se enganchan); no retocar la compensación para «conseguir» el modo.
- **Residuales del solve sin NFB, con impulsos violentos** (documentados y
  acotados en `tests/reactive_load.rs`): un salto de 0,2–1,0 Vp deja una
  ráfqaga de fallos del solve de potencia que se recupera (155–1.347 llamadas
  de 65.536 según la tasa; seno equivalente limpio). El impulso extremo 4 Vp
  a 44,1 kHz engancha el solve aislado (~52 % de llamadas) y está
  `#[ignore]` como `reactive_impulse_extreme_pending_b18_009`. No es
  problema del lazo; se re-abre en B18-009.
