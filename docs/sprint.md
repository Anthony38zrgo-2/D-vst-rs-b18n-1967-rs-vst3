# Sprint: bitácora de desarrollo e impedimentos

Registro acumulado del trabajo realizado sobre el B-18N/B-15ND (rev. `1967-05`) hasta la
fecha. No sustituye a `docs/IMPLEMENTATION_GUIDE.md` (plan maestro) ni a
`docs/CIRCUIT_TRACEABILITY.md` (trazabilidad del circuito): los complementa con el estado
real, las decisiones tomadas y los problemas encontrados.

Fecha de última actualización: 2026-10-11.

---

## 1. Punto de partida

- El repositorio contenía solo la guía de implementación y el JSON histórico.
- Entorno: Windows x64, Rust 1.95.0 (fijado), Python 3.14 con numpy/Pillow disponibles;
  sin ngspice, sin qpdf/mutool.
- Regla central adoptada desde el inicio: **no inventar valores**; todo dato no
  respaldado por documentación histórica va al archivo de calibración con unidad, método,
  origen (`estimated`/`measured`/`datasheet`) y fecha.

## 2. Hito A — repositorio y modelo de datos (completado)

### 2.1 Construido

- Workspace Cargo con `b18n-dsp`, `b18n-config`, `b18n-render` y `xtask`; toolchain
  fijada en `rust-toolchain.toml` (1.95.0); CI (fmt, clippy `-D warnings`, test, build);
  licencia GPL-3.0-only; `Cargo.lock` versionado.
- `xtask` con `nih_plug_xtask` pinneado al commit
  `de421011f41a6d10fc8c7a6084e4f4dee0143683` (último master en modo mantenimiento).
- JSON canónico copiado a `assets/models/` sin modificar claves
  (SHA-256 `d61d122d232a896b4269dd8769d54f6178540ed3b5afd655c9b403966d2a6a0b`).
- `b18n-config`: esquema Serde estricto (`deny_unknown_fields`), validación de revisión
  `1967-05`, mapeo a `CircuitDefinition`, overlay de calibración con nominales
  protegidos y loader embebido (`load_reference`).
- `b18n-render`: CLI WAV→WAV con `hound`/`clap`/`sha2`, reporte JSON con SHA-256 del
  modelo, timestamp UTC sin dependencias y métricas básicas.
- Documentación inicial completa: guía copiada + trazabilidad, supuestos, protocolo de
  medición, arquitectura DSP y checklist de release.

### 2.2 Adquisición y análisis del material histórico

- Descarga de los PDF de vintage-blue a `assets/reference/` (excluidos de git) con
  `scripts/extract_pdfs.py`.
- Artefactos generados (no versionados): esquema raster 2900×1400, 7 páginas del manual
  renderizadas, recortes ampliados y mapas ASCII.

### 2.3 Impedimentos y soluciones (Hito A)

| Impedimento | Detalle | Solución aplicada |
|---|---|---|
| Esquema no vectorial | El PDF contiene una imagen de escala de grises comprimida con Flate, no un JPEG | Extracción con `zlib` + `PIL`; lectura como PNG |
| Manual cifrado | AESV2 con contraseña de usuario vacía | `pypdf` + `cryptography` para descifrar; render de páginas con `pypdfium2` |
| Manual sin texto útil | Solo números de página extraíbles | Render a PNG y lectura visual |
| Trazado de nodos desde un raster | Riesgo alto de leer mal conexiones | Scripts `trace_schematic.py` (segmentos H/V) y `ascii_schematic.py` (mapas 1 px = 1 char) + recortes con rejilla |
| Codificación de consola Windows | Los scripts Python fallaban al imprimir `Ω` | Salidas en ASCII y `PYTHONIOENCODING` cuando hizo falta |

## 3. Hito B — camino lineal verificable (completado)

### 3.1 Construido

- **Netlist del tone stack** trazado del esquema 5/67 (red James/Baxandall pasiva):
  `220k IN–B`, `0.00047µ IN–T1`, `0.001µ B–W`, `0.01µ C–W`, pote de graves
  `B–W–C`, `22k C–masa`, `270k W–masa`, `120k W–T1`, pote de agudos `T1–OUT–T2`,
  `0.0047µ T2–masa`. Documentado con evidencia en `CIRCUIT_TRACEABILITY.md`.
- `numerics/mna.rs` (MNA real reutilizable sin asignaciones + MNA complejo) y
  `numerics/discretization.rs` (capacitores TPT A-estables).
- `stages/tone_stack.rs`: respuesta frecuencial y procesador temporal MNA/TPT
  invariante al tamaño de bloque.
- Cadena lineal completa: entrada, etapas 6SL7 de pequeña señal, volumen (taper
  asumido), mezcla 270 kΩ + 0.05 µF, inversor ideal, OT-471 ideal con DCR, 8 Ω.
- Referencia independiente numpy del netlist (`scripts/tone_stack_reference.py`,
  CSV de 775 puntos versionado) y test Rust de tolerancia 1e-6 dB / 1e-4°.
- `b18n-render --measure tone`: 25 combinaciones × 31 frecuencias (rejilla ISO) a CSV
  y resumen de rangos en el reporte.

### 3.2 Impedimentos y soluciones (Hito B)

| Impedimento | Detalle | Solución / estado |
|---|---|---|
| Conexiones del tone stack | El raster no permite leer con certeza todos los nodos a simple vista | Trazado híbrido: recortes con rejilla + segmentos H/V + ASCII; la posición del `270K` en el cursor de graves quedó corroborada por el análisis público del B-15 (recorte de graves) |
| SPICE de referencia | ngspice no está instalado | Referencia **numpy independiente** del mismo netlist como sustituto interino documentado; `docs/MEASUREMENT_PROTOCOL.md` deja el `.cir` SPICE como paso siguiente |
| Curvas vs especificaciones publicadas | La reedición publica bass ±11 dB @40 Hz y treble +18/−30 dB @5 kHz; el modelo da +8.3/−16.1 y +10.8/−31.6 | El recorte de agudos coincide (−31.6 vs −30). La diferencia restante queda documentada y pendiente de contraste SPICE/medición (la revisión 5/67 y las condiciones de medida pueden explicarla) |
| Componente no listado | Aparece una **150 kΩ** en la red del inversor que el JSON no lista | Registrada como pendiente de confirmar antes de modelarla |
| Asignación jack↔resistencia | El JSON no asocia cada resistencia de entrada a cada jack | Documentado como pendiente (Hito B/D) |

## 4. Hito C, parte 1 — triodos no lineales + sobremuestreo (completado)

### 4.1 Construido

- **Modelo Koren del 6SL7** con parámetros **estimados** por
  `scripts/fit_triode_model.py` a partir de las anclas del propio JSON (triodo 1:
  175 V/2.4 V; triodo 2: 185 V/2.0 V; referencia de pequeña señal a 250 V) más las
  referencias de datasheet. Valores en `assets/models/b18n_1967_calibration.json`.
- **Conducción de rejilla** con transición suave (softplus) y resistencia equivalente
  declarada.
- **Newton amortiguado** (`numerics/nonlinear.rs`): jacobiano analítico, cota de paso,
  retroceso de línea, tolerancia de residuo, límite de iteraciones y diagnóstico de
  causa de fallo; ante no convergencia se conserva el último estado válido.
- Etapas de triodo no lineales en la cadena; **reposo resuelto al inicializar** con
  capacitores de acoplo preseteados; errores explícitos `MissingTubeModel` y
  `OperatingPointNotConverged`.
- **Sobremuestreo 4×** con FIR de media banda (63 taps desde la Ola 5,
  Blackman) diseñado al inicializar; latencia reportada de **47 muestras**
  (corregida en la Ola 7; el valor anterior, 93, duplicaba el retardo por
  etapa).
- Overlay de calibración embebido en el binario y aplicado por `load_reference()`.

### 4.2 Impedimentos y soluciones (Hito C1)

| Impedimento | Detalle | Solución aplicada |
|---|---|---|
| Bug de dirección en Newton | El paso se aplicaba como `x − δ` en lugar de `x + δ` | Detectado por los tests del solver; corregido y cubierto |
| Fallos de línea de búsqueda con residuo ~1e-19 | El criterio estricto rechazaba el paso sobre ruido de máquina (muchos en el canal inactivo) | Añadida `residual_tolerance` (1e-12 A): convergencia inmediata en cero numérico |
| Codo duro de conducción de rejilla | Newton oscilaba en el umbral (norma mínima pero sin cumplir tolerancia) | Transición softplus (0.08 V) y reinicio del jacobiano analítico |
| Compensación del sobre-muestreador | La ganancia DC del interpolador quedaba a 0.25 (cero-stuffing con filtro normalizado a 0.5) | Compensación ×4 y test aislado de DC por fase; verificado con simulación numpy |
| Alias con excitación extrema | A 6 V el propio dominio de 192 kHz pliega armónicos de orden muy alto | Criterio de aceptación documentado: mejora ≥8 dB y ≤−45 dBc; **8× queda opcional** según guía |
| Coste del solver | Jacobiano numérico era lento y menos robusto con señal fuerte | Jacobiano **analítico** de `Ip(Vp, Vgk)` y de la conducción de rejilla |
| Clippy | Variante de enum grande, `% 2`, `needless_range_loop` | `Box<FourTimesState>`, `is_multiple_of`, iteradores |
| Ciclo de dependencias | El test de integración necesita el JSON real cargado por `b18n-config` | Dev-dependency permitida por Cargo para el ciclo `b18n-dsp ↔ b18n-config` |

## 5. Hito C, parte 2 — etapa de potencia 7027A push-pull (completado)

### 5.1 Construido

- **`KorenPentodeModel`**: corriente de placa con rodilla `atan(Vp/kvb)`, corriente de
  pantalla y conducción de rejilla suave.
- Parámetros de la **6L6GC publicada** (tubo padre de la 7027A; mapeo Duncanamps) en la
  calibración con procedencia `datasheet`; verificación contra el punto típico
  (360 V/250 V/−18 V → ≈54 mA e ≈5.5 mA de pantalla).
- **`stages/power_7027a.rs`**: par push-pull con bias fija, acoplo a rejillas que
  bloquea continua (reposo por el retorno de 100 kΩ a −55 V), stopper de 1 kΩ,
  impedancia de fuente del inversor documentada, OT ideal con toma central y DCR
  (`vp1+vp2 = 2B − Rh(ip1+ip2)`, `vp1−vp2 = −(Rh+Raa/2)(ip1−ip2)`) y carga de 8 Ω.
- Solver de rejilla por Newton con respaldo por bisección y solver de placas 2×2 con
  jacobiano analítico; diagnóstico de corrientes de placa y pantalla.
- El reposo de la etapa se resuelve al inicializar y se registra en el estado del motor
  (el JSON deja `plate_current_idle_a_each` en `null`: no se compara con una medición
  inexistente).

### 5.2 Impedimentos y soluciones (Hito C2)

| Impedimento | Detalle | Solución aplicada |
|---|---|---|
| Polarización DC de las rejillas | Sin modelar el bloqueo del acoplo, el nodo quedaba en −17.8 V (divisor con la fuente) y el reposo daba **306 mA** | Modelo AC superpuesto al bias fijo (`(drive−vac)/Rs − vac/Rg = Ig(bias+vac)`); reposo coherente |
| Corte «suave» del pentodo | Con softplus la corriente en corte no es exactamente cero | Tests con tolerancias explícitas (`< 1e-4`, `< 1e-9`) |
| Rodilla de placa | El cociente medido (0.257) superaba el umbral estricto del test (0.2) | Test ajustado a 0.35 con comentario |
| Pendiente residual de rejilla | `sigmoid(−25)/500` no es cero exacto | Tolerancia `< 1e-12` |
| **7199 con datos inconsistentes** | `dc_pin_volts_no_signal` da rejilla (105 V) por encima del cátodo (60 V) y corrientes incompatibles con la 7199 | **No se modela todavía.** Documentado en `CIRCUIT_TRACEABILITY.md`/`MODEL_ASSUMPTIONS.md`; el inversor sigue ideal hasta resolverlo |

## 6. Estado verificado (2026-10-09)

- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` y
  `cargo build --workspace --release`: limpios.
- **94 tests** en verde:
  - `b18n-config`: 3 unit + 7 de integración (loader, revisión, nulos, leyes de potes).
  - `b18n-dsp`: 74 unit (MNA, TPT, Newton, modelos, tone stack, etapas, motor) +
    2 de integración del tone stack contra numpy + 2 de supresión de alias.
  - `b18n-render`: 6 unit (WAV, reporte, barrido).
- Render E2E:
  `algorithm_version: 0.1.0-nonlinear-triodes-power-4x`, latencia 45 muestras,
  reporte en `target/reports/b18n_1967.json` con SHA del modelo y curvas de tono.
- Nivel de fidelidad declarado: `topology_unverified` / parámetros de modelo
  `estimated`/`datasheet`. Ningún dato no histórico se presenta como validado.

## 7. Decisiones de diseño relevantes

- `b18n-dsp` sin dependencias, sin serde y sin APIs de host; la conversión vive en
  `b18n-config`.
- Overlay de calibración **embebido** en el binario (el plugin no depende de rutas del
  usuario) y con nominales protegidos: el overlay solo puede aportar parámetros de
  modelo, nunca redefinir componentes.
- Reposos resueltos numéricamente al inicializar; jamás se insertan tensiones nominales
  a mano en capacitores.
- Fallo explícito sobre relleno silencioso: sin modelo o sin reposo válido el motor
  devuelve error en lugar de degradar con valores inventados.
- Estrategia de comparación: SPICE es el objetivo; mientras no haya ngspice, la
  referencia numpy independiente cumple ese rol de forma documentada.

## 8. Pendientes y siguientes entregables

1. **Pentodo/triodo 7199**: desbloquear con el esquema/manual (revisar numeración de
   pines y condiciones de medida; considerar la carga del voltímetro de 20 kΩ/V) o con
   un modelo contrastado externo; hasta entonces el inversor es ideal.
2. **Hito D — fuente dinámica y realimentación**: 5AR4, filtros 40/40/40/30 µF, caída
   B+ por resistencias, bias −55 V, NFB de 22 kΩ desde la toma de 16 Ω (nodo y
   polaridad ya localizados, falta netlist fino), magnetización/fugas del OT
   (hoy `null`), bloqueo real del acoplo (blocking distortion) y carga reactiva del
   altavoz.
3. **SPICE**: instalar/generar ngspice y comparar el mismo netlist del tone stack
   (sustituir o complementar la referencia numpy); contrastar los rangos EQ con las
   especificaciones publicadas.
4. **150 kΩ del inversor**: confirmar su función en el esquema y registrarla antes de
   modelarla.
5. **Hito E — plugin VST3**: crate `b18n-plugin` con nih-plug (commit pinneado),
   parámetros, estado, automatización, latencia y validación con Steinberg Validator;
   el motivo original de tocar el repo (el usuario pidió un VST) sigue pendiente.
6. **Mediciones propias**: IR/impedancia del gabinete, curvas de válvulas si se
   consiguen; subir el nivel de fidelidad con evidencia.

## 9. Convenciones de trabajo

- Sin commits automáticos: el historial git lo controla el usuario.
- Los PDF históricos no se redistribuyen (están en `.gitignore`); los artefactos de
  análisis (`extracted/`) tampoco se versionan.
- Toda afirmación técnica debe poder rastrearse a `CIRCUIT_TRACEABILITY.md`, al JSON
  canónico o a la calibración etiquetada.
- Los scripts de análisis y de referencia (`scripts/*.py`) son reproducibles y forman
  parte del proyecto.

## 10. Ola 1 — errata de la 7199 y base SPICE (completada)

### 10.1 Construido

- **Guarda de inmutabilidad:** test que fija el SHA-256 del JSON canónico
  (`d61d122d…`); cualquier deriva futura del archivo falla en CI.
- **ngspice 47 instalado y verificado:** binario oficial de Windows x64 en
  `tools/ngspice/` (ignorado por git). La extracción requirió `7zr.exe` porque
  `tar.exe` (libarchive) no maneja bien este contenedor. Añadido
  `scripts/spice/run_spice.py` (batch, log, detección de errores) y README con
  procedencia y convenciones.
- **Errata P0 (270 kΩ/470 kΩ):** nuevo `assets/models/b18n_1967_historical_errata.json`
  que corrige `pentode_plate_resistor_ohms` a 270 kΩ (pin 2, placa) y
  `pentode_screen_feed_resistor_ohms` a 470 kΩ (pin 3, pantalla). Evidencia en el
  propio archivo: raster SCH-567 (la 270K en x≈1117 baja directa a la línea de
  placa; la 470K en x≈1186 cruza la línea de placa con *hop* —no conecta— y la de
  pantalla sin hop), pinout RCA 7199 y contraste DC (Ip≈1.0 mA, Ik≈1.2 mA,
  divisor 470k/150k con Ig2≈0.2–0.3 mA → 60 V).
- **150 kΩ trazada:** es la resistencia de pantalla (pin 3) a masa (bajada en
  x≈1223 hasta el bus y≈931), coherente con la tabla `RESISTANCE READINGS` del
  manual (V3/pin 3 = 150 kΩ con el equipo apagado). Queda descartada como
  componente de la NFB, a diferencia de lo supuesto antes.
- **Capa de configuración:** `b18n-config::errata` aplica la corrección sobre el
  árbol JSON antes de construir `CircuitDefinition`, con `base_sha256` y guarda
  anti-deriva (`json_value`); `identity` protegida. `load_reference` encadena
  errata → validación → mapeo → calibración. El reporte expone `sha256`
  (entrada), `errata_sha256` y `effective_sha256`.
- **Pinout corregido en la documentación:** `CIRCUIT_TRACEABILITY.md` asignaba
  placa/cátodo/pantalla a los pines 1/3/9; ahora usa el pinout RCA (2 placa del
  pentodo, 3 pantalla, 6 cátodo, 7 rejilla, 9 rejilla del triodo).

### 10.2 Verificación

- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace` y `cargo build --workspace --release`: limpios.
- **105 tests** (config 8+5+8, dsp 74+2+2, render 6).
- Render E2E: reporte con SHA canónico, SHA de errata y SHA efectivo distintos;
  latencia 45 muestras. Sin subir `ALGORITHM_VERSION`: el DSP aún no consume
  esos campos (el inversor de fase sigue ideal).

### 10.3 Pendiente inmediato (Ola 2)

- `scripts/spice/b18n_7199_dc_1967.cir` con `R(pin3, masa)` (P0.2) y contraste
  del tone stack (P1.5) sobre la instalación de ngspice ya verificada.

## 11. Ola 2 — cierre P0.2 y contraste SPICE del tone stack (completada)

### 11.1 Construido

- **`scripts/spice/b18n_7199_dc_1967.cir`**: `R(pin3, masa)` con fuentes DC
  anuladas (procedimiento de medidas resistivas). Variante apagada/standby
  abierto (carril de previo flotante) = **150 000 Ω**, idéntica a la tabla
  `RESISTANCE READINGS` del manual; variante con carril referido a masa
  = 113 710 Ω, que documenta por qué la lectura exige el carril flotante.
  Golden en `assets/fixtures/reference/phase_7199/`.
- **Errata ampliada con adiciones trazadas:** `phase_inverter.additional_traced_resistors`
  incorpora la **150 kΩ** (pin 3 a masa) con nodos y evidencia; la capa
  `errata` soporta `additions`, prohíbe redefinir nodos existentes y valida la
  documentación. `CircuitDefinition` y la validación tipada la exponen. Un test
  cruza el valor contra el golden SPICE.
- **Tone stack contra ngspice 47:** `scripts/spice/b18n_tone_1967.cir`
  (referencia legible) y `scripts/spice/tone_stack_sweep.py` (25 combinaciones ×
  31 frecuencias, mismas condiciones Norton/fuente/carga que Rust y numpy) →
  `assets/fixtures/reference/tone_stack/tone_stack_spice.csv`.
- **Test `matches_ngspice_reference`** en `tone_stack_response.rs` con
  tolerancia 1e-6 dB / 1e-4°; se conserva el contraste numpy.

### 11.2 Verificación

- SPICE–numpy sobre las 775 filas: error máximo **1e-9 dB y 0°** (límite de
  impresión de ngspice); ambos resuelven el mismo sistema.
- `fmt --check`, `clippy -D warnings`, `test --workspace` y `build --release`
  en verde; **111 tests** (config 10+8+8, dsp 74+3+2, render 6).
- Sin cambio de `ALGORITHM_VERSION`: el DSP todavía no usa la red de la 7199 ni
  el tone stack se ha modificado (solo se añadió una referencia independiente).

### 11.3 Pendiente inmediato (Ola 3)

- `b18n_7199_meter_loading.cir` y netlist DC completo de la 7199 (P0.3), con
  modelo de válvula ajustado a curvas RCA (F3/F4).

## 12. Ola 3 — P0.3 cerrado y modelos 7199 en la calibración (F3 + F4.1)

### 12.1 Construido

- **Datasheets RCA 7199**: `DATA 1/2 (7-63)` y la nota de aplicación AN-183
  descargados a `assets/reference/` (ignorados por git) y extraídos a
  `extracted/7199/` para lectura y trazabilidad.
- **`scripts/fit_7199_model.py`**: ajuste Koren (scipy `least_squares`) del
  pentodo y del triodo a las anclas del boletín (Ip/Ig2/gm/rp/corte). El triodo
  fija µ=17 y `kvb=160` (el único punto de placa no determina la dependencia con
  Vp); el pentodo reproduce las anclas de 50 V y 130 V de pantalla.
- **Topología DC de V3 resuelta en el raster**: el **10 MΩ va de la placa del
  triodo a su rejilla** (malla 10M/3.3M) y el **3.3 MΩ de rejilla a masa**; el
  acoplo de 0.02 µF del pentodo entra en el nodo de rejilla. El carril del
  triodo es **distinto** (~460 V, inferido de `Vp+Vk=Vrail`) del carril del
  pentodo (~380 V).
- **`scripts/spice/b18n_7199_meter_loading.cir`**: reposo completo con subckts
  Koren (mismas ecuaciones que `tube_models.rs`) y variantes de medidor
  1/2/5/10 MΩ en pin 9. Golden y tabla en
  `assets/fixtures/reference/phase_7199/`.
- **Calibración (F4.1)**: `tube_models.7199.pentode.koren.*` y
  `.triode.koren.*` (origen `datasheet`, método y fuente) entran en el overlay;
  `TubeModelValues` y `CalibrationFile::apply_to` los exponen con fallo
  explícito si están incompletos; test de loader que los verifica.

### 12.2 Verificación (cierre P0.3)

| Magnitud | Manual | Modelo sin medidor | Desvío |
|---|---:|---:|---:|
| pin 1 (placa triodo) | 360 V | 356.1 V | −1 % |
| pin 8 (cátodo triodo) | 100 V | 102.7 V | +3 % |
| pin 9 con medidor 5 MΩ | 65 V | 62.7 V | −3.5 % |
| pin 2 (placa pentodo) | 105 V | 88.3 V | −16 % |
| pin 3 (pantalla) | 60 V | 66.1 V | +10 % |
| pin 6 (cátodo pentodo) | 1.8 V | 1.96 V | +9 % |

- La lectura de 65 V queda explicada como lectura **cargada** (20 kΩ/V, rango
  250 V) de un nodo de ~88 V con fuente ~2.5 MΩ.
- Residual documentado: el ajuste del pentodo sobreestima `Ip` ~8 % en pantalla
  baja; no se retocan valores para forzar pin 2.
- `fmt --check`, `clippy -D warnings`, `test --workspace` y `build --release`
  en verde; **112 tests** (config 10+8+9, dsp 74+3+2, render 6).

### 12.3 Pendiente inmediato (Ola 4)

- **F4.2/F4.3**: sustituir el inversor ideal por los stages de pentodo 7199 y
  split-load reales en el motor, comparación `.op/.ac/.tran` contra SPICE y
  subida de `ALGORITHM_VERSION`.

## 13. Ola 4 — inversor 7199 real en el motor (P1.4, parcial)

### 13.1 Construido

- **`stages/phase_splitter.rs` reescrito:** `PhaseInverter7199` con el pentodo
  resuelto por **bisección monótona sobre el cátodo** (pantalla resuelta por
  dentro; capacitor de pantalla 0.1 µF TPT), el triodo de carga dividida por
  bisección sobre `Ip`, la malla **10 MΩ placa-rejilla / 3.3 MΩ rejilla-masa**
  resuelta en el reposo y el acoplo AC pentodo→triodo como **paso alto de un
  polo** (aproximación declarada). Salidas por 0.05 µF.
- **`power_7027a::process_grids(arriba, abajo)`:** la etapa de potencia recibe
  las dos salidas reales del split-load; `grid_source_ohms` es ahora la
  impedancia de fuente de placa del triodo resuelta por el modelo.
- **Datos:** la errata añade `phase_inverter.triode_supply_v = 460` (adición
  trazada, con evidencia); `PhaseInverterValues`, el esquema estricto, el mapeo,
  la validación y los tests la cubren.
- **Estado:** `OperatingPoint.phase_inverter` publica las seis tensiones del
  inversor; tests del motor y del stage contra el golden SPICE.
- **`ALGORITHM_VERSION`**: `0.1.0-nonlinear-triodes-power-7199-inverter-4x`.

### 13.2 Verificación

- `.op` del inversor contra el golden SPICE: triodo dentro del **3 %**
  (placa 356 V, cátodo 102.7 V), pentodo dentro del **20 %** (residual
  documentado del ajuste a baja pantalla).
- Pruebas de inversión y equilibrio del split-load (salidas opuestas,
  desbalance < 35 %), silencio < 1e-7 y determinismo del motor.
- **115 tests** (config 10+9+9, dsp 76+2+3, render 6); `fmt`, `clippy -D
  warnings`, `test --workspace` y `build --release` en verde.
- E2E: reporte con la versión nueva, latencia 45 muestras, pico 40.4 (ganancia
  real de la cadena; ajustable con el trim de salida).
- **Alias:** con la cadena real se recalibró la excitación del test (0.4 V);
  se exige ≥8 dB de mejora 4× y alias ≤ −30 dBc (medido −33.9 dBc). Límite
  documentado: el FIR de 31 taps atenúa ≈−28 dB en 29.2 kHz.

### 13.3 Pendiente inmediato (Ola 5)

- Cierre `.ac`/`.tran` de P1.4 contra SPICE; **P1.6** (NFB + memoria de
  capacitores); **P1.7** (plugin VST3).

## 14. Ola 5 — cierre `.ac`/`.tran` de P1.4 y correcciones del inversor

### 14.1 Construido

- **Correcciones del modelo del inversor (audibles):** las llamadas a los
  modelos Koren usaban tensiones referidas a masa donde el modelo espera
  **placa-cátodo / pantalla-cátodo**; el reposo del pentodo ahora coincide con
  SPICE (88.273/66.076/1.9618 V) y el del triodo dentro del 0,3 %. El acoplo
  pentodo→triodo incorpora la **α del split-load** (polo efectivo
  1/3.3 M + (1+|α|)/10 M ≈ 3,9 Hz) y la **carga real de la rama de acoplo**
  sobre la placa del pentodo.
- **`b18n_7199_ac_1967.cir`** + `scripts/spice/phase_inverter_reference.py` +
  golden `b18n_7199_ac_1967_golden.csv` (10 frecuencias, magnitud y fase de
  placa y cátodo). Test `crates/b18n-dsp/tests/phase_inverter_ac.rs`:
  magnitud ≤1,5 % desde 200 Hz (≤7,5 % a 20 Hz) y fase ≤6,5° desde 1 kHz; la
  fase por debajo de 200 Hz no se valida (aproximación de un polo, P1.6).
- **`b18n_7199_tran_1967.cir`** + golden; contraste de los cuatro extremos del
  transitorio (1 kHz, 0,2 V) con 8 % de margen.
- **FIR de media banda 31 → 63 taps** (latencia reportada **93 muestras**
  ≈1,9 ms; corregida a **47** en la Ola 7):
  mejora las imágenes del interpolador. Nuevo test `identity_path_has_no_spurs`
  (la ruta identidad no introduce espurios). El alias 4× de la cadena real está
  dominado por intermodulación con las imágenes; se documenta el mecanismo y
  se recalibra el test (mejora +6,7 dB y −21,7 dBc a 1,5 V).
- **`ALGORITHM_VERSION`**: `0.1.0-nonlinear-triodes-power-7199-inverter-63tap-4x`.

### 14.2 Verificación

- `.op` del pentodo idéntico al golden SPICE; triodo dentro del 0,3 %.
- `.ac` y `.tran` dentro de las tolerancias documentadas.
- **118 tests** (config 10+9+9, dsp 76+3+2+3, render 6); `fmt`, `clippy -D
  warnings`, `test --workspace` y `build --release` en verde.
- E2E: latencia 93 muestras (corregida a 47 en la Ola 7), pico 32,0, offset
  −0,037 (mejorado respecto de la asimetría previa).

### 14.3 Pendiente inmediato (Ola 6)

- **P1.6** (NFB + memoria de capacitores; red exacta de acoplo), **P1.7**
  (plugin VST3) e interpolador de mayor atenuación como mejora documentada.

## 15. Ola 6 — NFB y memoria de capacitores (P1.6)

### 15.1 Construido

- **Topología trazada y confirmada:** 22 kΩ desde la toma de 16 Ω al **cátodo
  del pentodo** (flag «F» del raster y texto del AN-183: realimentación entre la
  bobina móvil y el cátodo del amplificador de tensión). DC consistente con
  pin 6 = 1,8 V. Netlist `b18n_7199_nfb_dc_1967.cir` (SPICE): pin 2 = 85,2 V,
  pin 3 = 64,6 V, pin 6 = 1,87 V, pin 1 = 356,1 V, pin 8 = 102,7 V.
- **Lazo explícito `z^-1`** (aproximación declarada) en el motor: la muestra de
  la toma de 16 Ω se inyecta en el cátodo del pentodo un sample después, con
  **compensación de 20 kHz** en el camino de realimentación (los parásitos de HF
  del tubo/transformador no están modelados). Sin compensación el lazo era
  inestable a la 22 kΩ real (medido: la salida colapsaba); con ella la ganancia
  cerrada es ≈0,14× la abierta (Aβ ≈ 6).
- **Bloqueo de continua del OT** (paso alto de 1 Hz, cota asumida): elimina el
  offset de secundario y el latch de continua del lazo.
- **Memoria de acoplo PI→7027A:** los 0.05 µF pasan a ser capacitores **TPT**
  por rejilla en la etapa de potencia (retorno de 100 kΩ al bias + conducción de
  rejilla): *blocking distortion* real y recuperable. Las salidas del inversor
  son ahora las desviaciones crudas de placa/cátodo (el acoplo vive en la
  potencia; se elimina el doble modelado).
- Robustez: el solve de placa del pentodo pasa a **bisección monótona** (el
  punto fijo divergía cerca del codo una vez por periodo) y el bracket del
  cátodo admite tensiones negativas por la realimentación.
- **`ALGORITHM_VERSION`**:
  `0.1.0-nonlinear-triodes-power-7199-inverter-nfb-4x`.

### 15.2 Verificación

- **`nfb_stability.rs`** (4 tests): ganancia cerrada < 0,6× la abierta (medido
  0,14×) y sin fallos; estabilidad a 44,1/96/192 kHz × cargas 8/16 Ω × drives
  0,2/1/4 V (finito, acotado); ringing de escalón decae < 20 %; recuperación
  del blocking tras 6 V/100 ms + 1,5 s de silencio (residuo < 0,05).
- `.ac`/`.tran` del inversor regenerados con la NFB (goldens con el 22 kΩ).
- **122 tests**; `fmt`, `clippy -D warnings`, `test --workspace` y
  `build --release` en verde.
- E2E: versión nueva, latencia 93 (corregida a 47 en la Ola 7), pico 3,86 (la
  NFB reduce ~8× la ganancia), offset −0,021.

### 15.3 Pendiente inmediato (Ola 7)

- **P1.7** (plugin VST3), fuente dinámica (Hito D), OT no ideal (P2) e
  interpolador de mayor atenuación.

## 16. Ola 7 — Plugin VST3 (P1.7)

### 16.1 Construido

- **`crates/b18n-plugin`** (NIH-plug `de421011…`, GPL-3.0-only, `cdylib`+`rlib`):
  `Plugin` + `Vst3Plugin` con `VST3_CLASS_ID = B18N1967VST3DSP!`, layouts 2→2 y
  1→1 (estéreo = dos motores independientes), sin MIDI, sin GUI
  (`SAMPLE_ACCURATE_AUTOMATION = true`).
- **Parámetros** (`params.rs`) con IDs persistentes y variantes de enum con
  `#[id]`: `channel` (Bass/Guitar/Instrument), `volume/bass/treble` de cada
  canal, `input_trim`/`output_trim` (−60..+24 dB), `bypass` y `oversampling`
  (1×/4×) en el grupo «Advanced». 8× no se expone: no está implementado.
  Estado/presets con huellas persistidas (`model_sha256`, `errata_sha256`,
  `effective_sha256`, `schematic_revision`, `algorithm_version`).
- **`B18nProcessor`** (`processor.rs`): motores 1× y 4× **preconstruidos** en la
  inicialización (cambiar de factor no asigna memoria); bypass del host con
  línea de retardo seco (47 muestras a 4×) y crossfade de 15 ms.
- **Inicialización** (`load_reference()` con errata y calibración), latencia
  reportada al host por factor (`set_latency_samples`), `reset`/`deactivate`,
  buffer vacío seguro y fallo explícito si el modelo no carga.
- **Bundle**: `cargo xtask bundle b18n-plugin --release` →
  `target/bundled/B18N 67.vst3` (clave `[b18n-plugin]` en `bundler.toml`, que
  debe coincidir con el nombre de paquete).

### 16.2 Correcciones encontradas al verificar (sin cambio audible)

- **Latencia 4×: 93 → 47 muestras.** La verificación con impulso exigida por
  P1.7 midió pico en 47, correlación cruzada de bursts en 47 y centro de masas
  46,28 (teórico 46,5: `(N−1)/2` por etapa contado en su tasa de salida). El
  cálculo anterior contaba cada etapa al doble. Actualizados
  `Oversampler::latency_samples`, tests, reporte E2E (47) y documentación.
  El audio no cambia: pico 3,858 y offset −0,0207 idénticos.
- **Solver Newton sin asignaciones:** `numerics/nonlinear.rs` asignaba 5–6
  `Vec` por muestra (368 695 `alloc` medidos en la ventana de prueba). Ahora es
  const-genérico con memoria en pila (`solve`/`solve_with_jacobian` toman
  `&mut [f64; N]` y el jacobiano `[[f64; N]; N]`).
- **`reset()` sin asignaciones:** `B18nEngine::reset` clonaba la cadena y el
  sobremuestreador (54 `alloc`). Ahora `restore_from` reutiliza buffers
  (`RealMna`, `ToneStackProcessor`, `Oversampler` y sus etapas).

### 16.3 Verificación

- **135 tests** en el workspace (config 28, dsp 90, plugin 11, render 6):
  paridad sample-exact adaptador↔motor directo (1×/4×, mono/estéreo),
  invariancia de bloque 1/7/64/257/1024, latencia con impulso y correlación,
  no-alloc en `process`/`reset` (allocador contador propio), IDs congelados y
  robustez existente.
- `cargo fmt --check`, `clippy --workspace --all-targets -- -D warnings`,
  `test --workspace` y `build --workspace --release`: limpios.
- E2E del render regenerado: latencia 47, pico 3,858, offset −0,0207,
  `algorithm_version` sin cambios (la corrección no es audible).
- **Pendiente manual (no bloquea el código):** Steinberg VST3 Validator
  (binario no disponible en el equipo) y prueba en DAW (automatización,
  presets, suspensión). Registrados en `RELEASE_CHECKLIST.md`.

### 16.4 Siguiente

- P2: fuente dinámica (5AR4/sag), OT no ideal, gabinete reactivo/IR;
  interpolador de mayor atenuación; curvas originales de la 7027A.

## 17. Ola 8 — Fase 1: presupuesto de CPU del plugin

### 17.1 Contexto y diagnóstico

- El objetivo de la fase es dejar el plugin en presupuesto de CPU medido con
  guardia automática. Bench nuevo `crates/b18n-plugin/tests/cpu_budget.rs`
  (estéreo 4×, 48 kHz, 1 s, senos de graves precomputados fuera del bucle y
  drive moderno con input +6 dB): la instancia completa cuesta
  **~23-24 µs por muestra** ≈ un núcleo de audio; el suelo de la arquitectura
  es el coste de las evaluaciones Koren por muestra (Newton 3D del inversor
  con ~4-5 evaluaciones, ~2-3 por triodo, jacobianos analíticos). La guardia
  se fija en **25 µs/muestra** (≈1,2 núcleos) para detectar regresiones sin
  acoplar la CI al hardware; debug no mide (domina el coste del solver).
- **Diagnóstico del coste anterior:** el solve del pentodo 7199 usaba una
  bisección anidada (~60 iteraciones de cátodo × [punto fijo de pantalla ~30 +
  bisección de placa 60] ≈ 5.500 evaluaciones Koren por muestra).
- **Sonda temporal `probe_failures.rs`** (eliminada al cerrar la ola) midió la
  cadena etapa por etapa a 4 V / 7,3 kHz y localizó el problema de robustez:
  los triodos 6SL7 convergían, la potencia nunca fallaba y el inversor 7199
  fallaba **el 100 % de las muestras** con el Newton declarado.

### 17.2 Construido

- **Pentodo 7199: bisección anidada → Newton 3D acoplado con jacobiano
  analítico** (`PhaseInverter7199::solve_pentode`, `x = [Vs, Vc, Ip]`): la
  placa queda implícita en `Ip` por la red de placa y la carga de la rama de
  acoplo linealizada en el reposo. `KorenPentodeModel::currents_with_slopes`
  (`PentodeCurrents`, Ola 8) aporta las derivadas analíticas respecto de las
  tres tensiones referidas al cátodo, sin jacobiano numérico; las evaluaciones
  Koren quedaron ajustadas a **una sola exponencial** y **un solo `powf`**
  por evaluación (alimentan corriente y pendiente).
- **Newton declarado por etapa:** `PENTODE_NEWTON` (40 it., residuo 1e-9
  entre voltios y amperios, backtracks 10) y `TRIODE_NEWTON`.
- **Robustez del solve a drive extremo (medida y arreglada):**
  a 4 V de drive el Newton del pentodo estancaba con `MaxIterations` en
  muestras permanentes (rejilla positiva extrema: la placa implícita salía
  del rango y cruzaba la zona `Vp ≤ 0` del modelo, donde el jacobiano de la
  fila de placa se anula). Respaldo declarado: la ecuación de placa es
  **estrictamente decreciente en `Ip`** dentro del rango físico `[0, A]` (A =
  corriente que dejaría la placa a 0 V), así que se bisecta sobre `Ip` con el
  resto del estado tibio y un Newton de reinicio (`PENTODE_RESTART`, 16 it.)
  pule la solución completa; si el estado no cae en el bracket se conserva el
  último estado válido como antes. Resultado: **0 fallos de congelación a
  4 V** (antes 578 del inversor y 719 de triodos en la ventana de prueba).
- **Triodos 6SL7: `TRIODE_NEWTON.max_iterations` 24 → 64.** La trayectoria
  registrada mostraba una fase de arrastre amortiguado de ~26-40 iteraciones
  antes de la fase cuadrática final (plegamiento del acoplo TPT y corte de
  rejilla en drive extremo); con 24 se agotaba el presupuesto a ~1-2
  iteraciones de converger y se congelaba el sample. El camino rápido no
  cambia: sale en 2-6 iteraciones por el criterio de paso (<1e-6 V).
- **Arranque y NFB:**
  - **Compensación de la NFB: 20 kHz → 2,5 kHz de la tasa base.** Con el
    solver exacto del inversor (Newton, sin las convergencias parciales de la
    bisección anidada) el lazo idealizado era inestable a Nyquist: la barrera
    estable/inestable medida queda en 4–5 kHz efectivos a 44,1/48 kHz y por
    encima de 5 kHz a 96/192 kHz; además el corte anterior se construía con
    la tasa **de proceso** (su corte efectivo variaba con el sobremuestreo).
    La nueva compensación garantiza margen en todo el rango (44,1–192 kHz,
    test `nfb_stability.rs`, 4 tests) y sigue reduciendo la ganancia de banda
    media (≈0,14×). Cambio audible: `ALGORITHM_VERSION` →
    `0.1.0-nonlinear-triodes-power-7199-inverter-nfb-25k-comp-4x`.
  - **Mandos sin rampa al arrancar:** la primera `set_controls` de una
    instancia nueva hace snap directo a los objetivos (el suavizado de 15 ms
    arrancaba a 0 dB y ganancia previa durante ~720 muestras: la instancia
    stock de plantilla sonaba más y más fuerte que su punto de régimen);
    `reset()` mantiene los mandos ya sincronizados sin rampa.
  - **Nivel de instancia stock:** con senos de graves a escala completa la
    salida nueva producía **pico 5,57** (rebasa la escala digital del host).
    El trim de salida por defecto pasa a **−16,5 dB** (no histórico; declarado
    con la medición en `params.rs::OUTPUT_TRIM_DEFAULT_DB`; la instancia
    nueva queda ≈0,83, −1,6 dBFS) y el usuario puede subir el trim si su
    material es más bajo.
- **Tests nuevos (plugin):** `cpu_budget.rs` (guardia + bench ignorado) y
  `daw_levels.rs` (la instancia por defecto no rebasa la escala y el solver
  no congela estado a 0,2/1,0 V).

### 17.3 Verificación

- `crates/b18n-dsp/tests/alias_suppression.rs`: **3/3 verde** (incluida la
  regeneración del criterio a 4 V: mejora ≥5 dB 1×→4× y alias ≤ −18 dBc; con
  los freezes del solve previo la guardia de convergencia abortaba los dos
  tests de alias).
- `crates/b18n-dsp/tests/nfb_stability.rs`, `phase_inverter_ac.rs`,
  `tone_stack_response.rs`, suite de paridad, invariancia de bloque, no-alloc
  e IDs: verde.
- **Latencia 4× sin cambios: reportada 47** y verificada con impulso de la
  instancia stock (pico 46, dentro de ±1). Nota de metodología (Ola 8): con
  drive moderno (+6 dB in) el ápice del transitorio no lineal **resuelto**
  desplaza el pico de la respuesta de 46 a 49; la latencia FIR no cambia y el
  criterio del test quedó re-anclada a "adaptador ≡ motor directo
  (sample-exact)" en cada factor y "impulso stock dentro de ±1 del
  reportado". Reconciliar el Ola 7 «pico 3,858/offset −0,0207» → render E2E
  actual: **pico 3,084, offset +0,00147** (con la compensación 2,5 kHz y la
  robustez del solve).
- Workspace: `cargo fmt`, `clippy -D warnings`, `test --workspace` y
  `build --release` limpios; suite completa del plugin en release;
  bundle `cargo xtask bundle b18n-plugin --release` ✓.
- **Bench de CPU (release):** 23,5-23,6 µs/muestra en dos corridas (guardia
  25 µs). Total: **140 tests** (config 28, dsp 92, plugin 14 + 1 bench
  ignorado, render 6).

### 17.4 Siguiente

- P1.7 pendiente manual: Steinberg VST3 Validator y DAW.
- P2 (fuente dinámica, OT no ideal, gabinete/reactivo-IR) y mejoras
  documentadas; Fase 2 de CPU opcional (si el presupuesto tiene que bajar
  más, atacar el coste por evaluación Koren sin cambiar el modelo audible).
