# Supuestos del modelo

Este documento separa lo que proviene de documentación histórica de lo que es una decisión
de ingeniería moderna. Nada de la lista «moderno» se presenta como característica del
hardware de 1967.

## Escala y flujo

- `Vpk_per_full_scale = 1.0` (convención inicial): una muestra de amplitud `1.0` equivale a
  `1.0 V` de pico **antes** de `InputTrimDb`. Es una calibración de software documentada,
  no una medida histórica.
- `InputTrimDb` y `OutputTrimDb` son adaptaciones modernas DAW↔voltios. El **trim de
  salida por defecto del plugin es −16,5 dB** (Ola 8): con senos de graves a escala
  completa la instancia nueva producía pico 5,57 y rebasaba la escala digital del host;
  el valor elegido deja esa instancia en ≈0,83 (−1,6 dBFS) y es una decisión de adaptación
  de nivel, no una característica del amplificador.
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

- **Taper del volumen:** aproximación asumida `f(p) = (10^(2p) − 1)/99` (10 % a mitad de
  recorrido). Sustituible por una curva medida sin cambiar la API.
- **Fuga de rejilla de la 7199 (mezcla):** 1 MΩ asumidos hasta digitalizar la red de
  entrada de la válvula; solo fija el polo del acoplo de 0.05 µF (~2.8 Hz).
- **Mezcla de canales:** resistores de 270 kΩ idénticos; el nodo vale
  `g·(v1+v2)`, `g = (1/Rm)/(2/Rm + 1/Rg)`. No se modela la impedancia de salida del canal
  inactivo (Hito D la desarrollará).
- **Carga de la red de tono:** 270 kΩ (resistor de mezcla) para las curvas estándar.
- **Ganancia de la cadena:** con el inversor 7199 real (P1.4) la cadena tiene
  ~45× más ganancia que con el inversor ideal del Hito B; el control de salida
  permite ajustarla. El OT sigue siendo ideal con DCR (bloqueo de continua del
  transformador no modelado: la asimetría del split-load puede dejar un pequeño
  offset en el secundario; documentado, P2).
- **Parasitismos del OT:** `winding_inductance_h`, fugas y capacitancias son `null`; no se
  inventan (Hito D).

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
- **Etapa de potencia 7027A (Hito C, parte 2):** modelo Koren de pentodo con
  corriente de placa y de pantalla y conducción de rejilla suave. Los parámetros
  son los **publicados de la 6L6GC** (tubo padre de la 7027A según el mapeo de
  Duncanamps), etiquetados `datasheet` en la calibración; la base de parámetros
  es la convención `PWR+PWRS` de la publicación, verificada contra el punto
  típico de la 6L6GC (360 V/250 V/−18 V → ≈54 mA, ≈5.5 mA de pantalla).
- **Alimentación de la etapa de potencia:** B+ 505 V en la toma central y
  pantallas a 500 V **rígidas**; la dinámica de B+/bias y el *sag* llegan con
  el Hito D.
- **Acoplo a las rejillas de la 7027A:** se modela el bloqueo de continua del
  acoplo de 0.05 µF (reposo fijado por el retorno de 100 kΩ a −55 V) y la
  excitación AC a través del stopper de 1 kΩ más una impedancia de fuente
  estimada del inversor (47 kΩ, documentada). La carga real del capacitor
  (blocking distortion) llega con el Hito D.
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
- **Inversor 7199 en el motor (P1.4, implementado y contrastado; solve en la
  Ola 8):** el pentodo se resuelve por **Newton 3D acoplado**
  (`x = [Vs, Vc, Ip]`, jacobiano analítico de las derivadas de
  `tube_models`) con la placa implícita en `Ip` por la red de placa y la carga
  de la rama de acoplo linealizada en el reposo; respaldo declarado de
  bisección monótona sobre `Ip` en el bracket físico `[0, A]` cuando el paso
  cruza la zona `Vp ≤ 0` del modelo, y Newton de reinicio para pulir pantalla/
  cátodo acoplados. El triodo de carga dividida se resuelve por Newton 1D con
  fallback de bisección. Los modelos reciben tensiones **referidas al cátodo**
  (placa-cátodo y pantalla-cátodo). El acoplo pentodo→triodo en AC se modela
  como **paso alto de un polo** con polo efectivo `1/3.3 M + (1+|α|)/10 M`
  (≈3,9 Hz, α la ganancia de placa del split-load) y la carga real de esa rama
  sobre la placa del pentodo (declarado); el reposo resuelve la malla completa
  10 M/3.3 M. Las salidas 0.05 µF alimentan `process_grids`. Contraste SPICE:
  `.op` del pentodo idéntico y triodo dentro del 0,3 %; `.ac` magnitud ≤1,5 %
  desde 200 Hz y fase ≤6,5° desde 1 kHz (por debajo de 200 Hz la fase no se
  valida: la red exacta llega con P1.6); `.tran` de extremos al 8 %.
  Simplificación declarada (Ola 8): el solve del pentodo **no modela la
  conducción de rejilla de la 7199** (la rejilla es fuente ideal para el
  motor); solo importa a drive de entrada extremo y queda documentado.
- **Alias con la cadena real:** el FIR de media banda se subió a **63 taps**
  (latencia **47 muestras** ≈1 ms; corregida en P1.7 con la respuesta al
  impulso, el cálculo anterior de 93 duplicaba el retardo por etapa) y su ruta identidad es limpia (espurios
  < −300 dB). Con la cadena del inversor, el alias 4× está dominado por
  **intermodulación entre las imágenes del interpolador y los armónicos** del
  pentodo (medido ≈−22 dBc a 1,5 V). Tras B18-002 el test exige cota absoluta
  ≤ −18 dBc en 1× y 4× (medido a 4 V: −31,7 y −25,1 dBc; la comparación
  relativa «+5 dB a favor de 4×» era un artefacto del polo de NFB dependiente
  del sobremuestreo). Un interpolador de mayor atenuación queda como mejora
  documentada (B18-018).
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
- **Bloqueo de continua del OT:** el transformador no pasa continua; se modela
  con un paso alto de **1 Hz** (cota conservadora asumida; la inductancia real
  de primario llega con P2). También elimina el offset de secundario.
- **Memoria de acoplo PI→7027A:** los 0.05 µF son capacitores TPT por rejilla
  con el retorno de 100 kΩ al bias y conducción de rejilla: el *blocking
  distortion* es real y se recupera (test `blocking_recovers_after_overdrive`).
- **Pendiente:** la dinámica de B+/bias (Hito D) y las mejoras de P2 (OT no
  ideal, gabinete). El plugin VST3 (P1.7) está implementado y empaquetado;
  quedan como pasos manuales el Steinberg Validator y la prueba en DAW.
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
