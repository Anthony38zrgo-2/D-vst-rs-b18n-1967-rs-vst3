# Trazabilidad del circuito

Cada valor y cada conexión del modelo debe poder referenciarse a una fuente. Este documento
registra qué está respaldado, qué se ha leído del esquema y qué sigue pendiente.

## Fuentes

| Id | Fuente | Uso |
|---|---|---|
| `JSON-1967` | `assets/models/B18N_1967_simulation_parameters.json` (SHA-256 `d61d122d…`) | Valores de componentes y tensiones nominales |
| `ERR-1967` | `assets/models/b18n_1967_historical_errata.json` | Correcciones documentadas del JSON, enlazadas a su SHA-256 |
| `RCA-7199` | datasheet RCA 7199 (1963) | Pinout y funciones de electrodos |
| `SCH-567` | `assets/reference/b18n-b15nd_schematic_5_67.pdf`, esquema 5/67, hoja 1/1 `4010036` | **Conexiones**, polaridad, nodos |
| `SM-1967` | `assets/reference/b18n_1967_service_manual.pdf` (texto extraído en `extracted/text/`) | Medidas de reposo, recuadro `Printed CKT`, procedimientos |

Los PDF se conservan localmente y **no se redistribuyen** (excluidos de git).

## Cadena de señal verificada en `SCH-567`

### Canal 1 (arriba) y canal 2 (abajo)

- **Entradas:** dos jacks por canal (CH1: `bass`, `guitar`; CH2: `instrument`), cada uno con
  resistencia serie de **120 kΩ**; resistencia de fuga de rejilla de **5.6 MΩ** en CH1 y
  **2.2 MΩ** en CH2; **100 kΩ** de referencia a masa en la zona de jacks.
  *Estado (B18-007): trazado con recortes del raster.* El jack `guitar` (inferior) es
  **conmutado**: su llave, cerrada sin plug, corta su punta a masa a través de la **100 kΩ**
  (por eso V1/pin 4 mide **220 kΩ** = 5.6 M ∥ (120 k + 100 k), coherente con la tabla del
  manual ±10 %); al insertar el plug la llave abre. El jack `bass` (superior) es simple
  (manguito a masa). CH2 (`instrument`) no tiene serie: la punta va directa a la rejilla de
  V2/pin 4, con la llave a masa sin plug. El manual lista 220 kΩ en V2/pin 4 igual que en
  V1/pin 4, valor que el raster no respalda (se registra la discrepancia). Implementado en
  `InputJackNetwork` (`crates/b18n-dsp/src/stages/input.rs`) con golden SPICE
  `assets/fixtures/reference/input_jacks/`.
- **V1/V2 (6SL7GT), por canal:**
  > **Modelo de válvula:** el JSON no incluye curvas (`tube_6sl7gt_characteristic_curves: null`).
  > El motor usa un modelo Koren con parámetros **estimados** (calibración, método en
  > `scripts/fit_triode_model.py`) ajustados a las anclas del propio JSON. Procedencia y
  > limitaciones en `docs/MODEL_ASSUMPTIONS.md`.
  - Triodo 1: placa **470 kΩ**, cátodo **5.6 kΩ** sin bypass, DC de placa **175 V**, cátodo **2.4 V**.
  - Acoplo **0.022 µF** → potenciómetro **VOL 1 M** (el cursor alimenta la rejilla del triodo 2).
    *Estado (B18-005): trazado.* El potenciómetro va **a masa** por su extremo inferior; no
    hay fuga de rejilla separada: la propia sección inferior del pot es la carga DC (la tabla
    del manual mide V1/pin 1 = **0 Ω a 1 MΩ** según la posición). La red completa
    (placa → `Ro` → 0.022 µF → Rtop–cursor–Rbottom) se resuelve en el Newton del 6SL7#2
    (`VolumeNetworkState`) con golden SPICE `assets/fixtures/reference/volume_network/`.
  - Triodo 2: placa **220 kΩ**, cátodo **2.2 kΩ** sin bypass, DC de placa **185 V**, cátodo **2.0 V**.
  - Acoplo **0.022 µF** → red de tono.
- **Tono (recuadro `Printed CKT`, CH1 y CH2 idénticos):** red pasiva acoplada de tipo
  James/Baxandall con **BASS 1 M** y **TREBLE 1 M**. Netlist trazado del esquema
  (nodos `IN` tras el acoplo, `B`, `W` = cursor de graves, `C`, `T1`, `T2`,
  `OUT` = cursor de agudos):

  | Elemento | Conexión | Evidencia |
  |---|---|---|
  | 220 kΩ | `IN – B` | resistor serie a la entrada del recuadro |
  | 0.00047 µF | `IN – T1` | paso de agudos sobre la red de graves |
  | 0.001 µF | `B – W` | red de graves, extremo superior del pote |
  | 0.01 µF | `C – W` | red de graves, extremo inferior del pote |
  | POTE GRAVES 1 M | `B – W – C` (cursor) | cursor actuando sobre la red |
  | 22 kΩ | `C – masa` | freno de la red de graves |
  | 270 kΩ | `W – masa` | «270K from the bass control wiper to ground» (recorte de graves) |
  | 120 kΩ | `W – T1` | enlace hacia la red de agudos |
  | POTE AGUDOS 1 M | `T1 – OUT – T2` (cursor) | cursor como salida del tono |
  | 0.0047 µF | `T2 – masa` | derivación de agudos |

  **Estado: netlist trazado y contrastado.** La posición del 270 kΩ en el cursor de graves
  está corroborada por el análisis de la comunidad del B-15 (TalkBass: ese resistor
  produce el recorte de graves global) y por la respuesta calculada. La salida del tono
  va por su propio **270 kΩ de mezcla** (fuera del recuadro). El orden de los arrays del
  JSON no codifica conexiones y no se usó como topología.
  Recortes ampliados reproducibles con `scripts/extract_pdfs.py` + `PIL` (no versionados).

  **B18-003 (2026-10-14):** el acoplo de **0.022 µF** de la placa del triodo 2 vive
  **dentro del MNA del tono** (nodo `SOURCE` = placa AC detrás de su impedancia de
  salida `Ro`; capacitor entre `SOURCE` e `IN`). El motor ya no aplica un paso alto con
  pérdida de inserción **fuera** del procesador (el divisor y la resistencia de fuente
  duplicaban la carga). Fixture nuevo con fuente + `Ro` + acoplo + red + carga:
  `scripts/spice/b18n_tone_coupled_1967.cir` y
  `assets/fixtures/reference/tone_stack/tone_stack_coupled_spice.csv` (contraste de
  magnitud y fase a 1e-6 dB / 1e-4°). El fixture aislado de la red se conserva sin
  cambios.

  Curvas calculadas (relativas a mandos al 50 %): graves **+8.3 / −16.1 dB @ 40 Hz**,
  agudos **+10.8 / −31.6 dB @ 5 kHz**. El recorte de agudos coincide con el publicado
  para la reedición 1966 (−30 dB); el resto difiere de las cifras de reedición y queda
  como observación documentada. El netlist ya está contrastado con **ngspice 47**
  (`scripts/spice/b18n_tone_1967.cir`, golden `tone_stack_spice.csv`): SPICE–numpy
  coincide a 1e-9 dB / 0° (véase `assets/fixtures/reference/tone_stack/README.md`).

### Mezcla y V3 (7199)

Pinout RCA 7199 (1963), confirmado en el esquema: pin 1 placa del triodo, pin 2 placa
del pentodo, pin 3 pantalla G2, pin 6 cátodo del pentodo, pin 7 rejilla de control del
pentodo, pin 8 cátodo del triodo, pin 9 rejilla del triodo.

- Salidas de ambos tonos se suman a través de **270 kΩ** por canal (resistor de mezcla
  `channel_mixer_resistor_ohms`) en un nodo compartido que va **directo** a la rejilla del
  pentodo (**pin 7**, 0 V). *Corregido en B18-006:* **no** hay capacitor de 0.05 µF ni fuga
  de 1 MΩ en ese nodo (los 0.05 µF del esquema son los acoplos del inversor a las 7027A).
  Evidencia: la tabla `RESISTANCE READINGS` del manual mide V3/pin 7 = **220 kΩ con Bass
  c.c.w.** y **330 kΩ con Bass c.w.** —valores que salen de `(270 kΩ + Z_tono)/2` con las
  dos ramas en paralelo (test `mixer_node_resistance_matches_manual_reading`: 205 kΩ/302 kΩ
  dentro del ±10 %). La impedancia de salida de cada red de tono en serie con su 270 kΩ
  queda como limitación declarada (B18-008 resolvió el acoplo interno del 7199; la carga
  de la red de tono sobre el mezclador sigue pendiente).
- **Pentodo:** placa por **270 kΩ** (**pin 2**, 105 V) desde el carril de previo;
  pantalla por **470 kΩ** (**pin 3**, 60 V) desde el mismo carril, con **150 kΩ a masa**
  y **0.1 µF** al cátodo; cátodo por **1.5 kΩ** a masa (**pin 6**, 1.8 V).
  La línea de placa cruza la bajada de la 470 kΩ con un *hop* (no conecta); el cruce
  con la línea de pantalla sí conecta (evidencia en `ERR-1967`).
  Contraste DC con carril ~380 V: `Ip≈1.0 mA`, `Ik=1.8/1.5k=1.2 mA≈Ip+Ig2`,
  divisor `470k/150k` con `Ig2≈0.2–0.3 mA` → ~60 V.
- Placa del pentodo → **0.02 µF** → rejilla del triodo (**pin 9**).
  **Red de polarización trazada:** **10 MΩ de la placa del triodo (pin 1) a la
  rejilla**, con **3.3 MΩ de rejilla a masa**. La rejilla queda en
  `Vg ≈ Vp·3.3/(10+3.3)` (≈88 V sin carga), explicando la lectura de 65 V del
  manual como efecto de la carga del medidor (20 kΩ/V, rango 250 V).
- **Triodo (inversor de fase de carga dividida):** placa por **47 kΩ** (**pin 1**,
  360 V) y cátodo por **47 kΩ** a masa (**pin 8**, 100 V). Como la misma corriente
  recorre ambas resistencias, `Vp + Vk = Vrail`; con las lecturas del manual el
  carril del triodo es **≈460 V**, distinto del carril del pentodo (~380 V): el
  raster muestra líneas de alimentación separadas. Acoplos **0.05 µF** hacia cada
  rejilla de las 7027A (el de placa sale del nodo de placa en `y≈465`).
- **150 kΩ (resuelto):** es la resistencia de pantalla a masa del pentodo (pin 3), no un
  componente de la realimentación; coherente con la tabla `RESISTANCE READINGS` del manual
  (V3/pin 3 = 150 kΩ con el equipo apagado).
  *Estado (B18-008):* la malla de acoplo **0.02 µF + 10 MΩ + 3.3 MΩ** y la conducción de
  rejilla del triodo se resuelven **acopladas con el pentodo en la misma muestra** (Newton
  6D, `crates/b18n-dsp/src/stages/phase_splitter.rs`); la placa del triodo realimenta su
  rejilla de forma dinámica (la aproximación de un polo queda solo como respaldo). El
  contraste contra el golden AC del circuito ampliado queda en ≤0,0002 % de magnitud y
  ≤0,05° de fase en 20 Hz–20 kHz.

### Realimentación negativa (NFB)

- `JSON-1967` registra **22 kΩ** desde el secundario. El esquema `SCH-567` muestra la
  **22 kΩ desde la toma de 16 Ω (GREEN)** y un flag «F» en el nodo del cátodo del
  pentodo (bajada de la 1.5 kΩ); la nota de aplicación **RCA AN-183** confirma la
  realimentación «entre la conexión de la bobina móvil y el cátodo del amplificador
  de tensión» (el pentodo). El retorno es DC-consistente con pin 6 = 1,8 V
  (22 kΩ a ~0 V ⇒ 0,08 mA). La **150 kΩ** de esa zona es la resistencia de pantalla
  (pin 3) a masa, no un componente del lazo.
  *Estado:* **cerrado (P1.6; polo corregido en B18-002)**. Implementado con
  separación explícita `z^-1` **de la tasa de proceso efectiva** y polo de
  compensación **provisional** a 2,5 kHz (declarado: no es una característica
  verificada del B-18N; los parásitos de HF no modelados están documentados).
  Desde B18-002 el polo se construye con la tasa de proceso efectiva y TPT con
  prewarp, de modo que su corte físico no depende del factor de sobremuestreo
  (antes, con la tasa base, el corte efectivo era ≈fc×OS). Invariancia medida
  en `crates/b18n-dsp/tests/nfb_rate_invariance.rs` y estabilidad en
  `crates/b18n-dsp/tests/nfb_stability.rs`. Golden DC con
  NFB: `scripts/spice/b18n_7199_nfb_dc_1967.cir` (pin 2 = 85,2 V, pin 6 = 1,87 V).

### Etapa de potencia (2 × 7027A)

- Placas a **OT-471** (`BROWN` = V5, `BLUE` = V4), derivación central `RED` a **505 V**.
- Pantallas a **500 V** (ver fuente), con **1 kΩ** de stopper; rejillas de control con
  **1 kΩ** de stopper y **100 kΩ** de retorno a la línea de bias **−55 V**.
- Acoplo inversor→rejilla **0.05 µF**.
- Cátodos a masa (bias fija). `plate_current_idle_a_each` sigue **null**: no inventarlo.

> **Modelo implementado (Hito C, parte 2):** OT ideal con toma central y DCR
> (`vp1+vp2 = 2B − Rh(ip1+ip2)`, `vp1−vp2 = −(Rh+Raa/2)(ip1−ip2)`), pentodos Koren
> con corriente de pantalla y conducción de rejilla suave. Parámetros de la 7027A
> tomados de la **6L6GC publicada** (tubo padre; procedencia en la calibración).
> B+/pantallas rígidas y bloqueo real del acoplo: Hito D. El reposo del modelo se
> registra en el estado del motor y no se compara con una medición porque el JSON
> no la aporta.

### Transformador de salida y altavoz

- **OT-471:** primario 4300 Ω placa-placa; secundarios **16 Ω (GREEN)**, **8 Ω (YELLOW)**,
  común **BLACK**; tomas de NFB según arriba.
- Salida a jack de altavoz y **EXT. SPKR.**; altavoz **8 Ω**.
- Sin IR ni curva de impedancia medidas (`null`).

### Fuente de alimentación

- **PT-12XA** (también listado PT-109), 117 V / 240 V; secundario de alta tensión 800 Vac
  con toma central → **5AR4/GZ34** (V6) → filtros **40/40/40/30 µF** a 500 V.
- Red de caída visible: **1 kΩ 10 W**, **10 kΩ 1 W**, **22 kΩ 1 W**.
- **Bias:** rectificador **10D6**, **100 µF/100 V**, **47 kΩ**, **56 kΩ**, ajuste de hum
  **100 Ω/2 W**; línea de **−55 V** a las rejillas de salida.

## Diferencias entre el JSON y el esquema

| Elemento | JSON | Esquema | Acción |
|---|---|---|---|
| 270 kΩ/470 kΩ de la 7199 | intercambiados (`plate`=470k, `screen`=270k) | placa (pin 2) 270 kΩ, pantalla (pin 3) 470 kΩ | **Corregido** por `ERR-1967` (errata enlazada al SHA-256 canónico) |
| 150 kΩ en zona del inversor | no listado | resistencia de pantalla (pin 3) a masa | **Trazado y verificado**: SPICE `R(pin3, masa)` = 150 kΩ (equipo apagado); entra en el netlist DC de la 7199 |
| 22 kΩ NFB | valor + nota «ver esquema» | desde toma de 16 Ω | netlist fino en P1.6 |
| Arrays de tono | valores sin conexión | recuadro `Printed CKT` | digitalizar nodo a nodo en Hito B |
| Tensiones `dc_pin_volts_no_signal` | 1:360, 2:105, 3:60, 6:1.8, 7:0, 8:100, 9:65 | pinout RCA; 9:65 V es lectura **cargada** por el medidor (red 10M/3.3M) | **Resuelto** en SPICE (`b18n_7199_meter_loading.cir`); usar como referencia de contraste |

## Medidas del manual y carga del instrumento

`SM-1967` indica lecturas con voltímetro de **20 000 Ω/V**. Al comparar, añadir la carga del
instrumento (según escala usada) a los nodos de alta impedancia; no comparar en crudo.

## Protocolo para nuevas entradas

1. Toda conexión nueva se anota aquí con su fuente (`SCH-567`, `SM-1967`) y fecha.
2. Toda inferencia va a `assets/models/b18n_1967_calibration.example.json` como
   `assumptions`/`measured` con unidades, método y fecha.
3. Las correcciones documentadas del JSON canónico van a
   `assets/models/b18n_1967_historical_errata.json`, enlazadas por SHA-256 y con
   evidencia, nodos y fecha; el canónico no se edita.
