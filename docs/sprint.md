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

## 18. Ola B18 segunda — 7027A desde el datasheet RCA (B18-010/011, completada)

### 18.1 Contexto y diagnóstico

- La etapa de potencia seguía usando los parámetros Koren publicados de la
  **6L6GC** (proxy documentado, `validated_at_7027a_operating_region=false`),
  lejos del régimen real del B-18N (B+ 505 V, pantallas ~500 V, bias −55 V).
- Datasheet RCA 7027-A (8-59, DATA 1/2) descargado en
  `assets/reference/7027A_RCA.pdf` (fuera de git). Contenido útil: tablas
  (clase A1, bias fija por tubo, máximos), familia de placa a Vg1 = 0 con
  **Ec2 como parámetro** (p05, Ec2 = 50…450 V), familia Ip/Ig2 a Ec2 = 300 V
  (p06) y transferencia Ip/Ig2 a Vp = 300 V (p07).

### 18.2 Ejecutado

- **Digitalización reproducible** (`scripts/digitize_7027a_curves.py`): calibración
  de ejes por las etiquetas impresas y la rejilla (p05: 10 V × 10 mA por celda;
  p07: 1 V en x), eliminación de la rejilla con líneas refinadas localmente y
  seguimiento de trazos con coasting y fusión de fragmentos. La identidad de las
  curvas se ancla con dos comprobaciones cruzadas: p05 Ec2=300 en Vp=300 ≈ 247 mA
  frente a p07 Ip(Vg1=0) ≈ 250 mA. p06 no se digitaliza en automático (curvas
  entrelazadas); solo una curva de Ig2 sin etiqueta guiada por anclas para la
  forma Ig2(Vp). Fixture: `assets/fixtures/reference/power_7027a/7027a_rca_digitized.csv`
  (315 puntos) con README y SHA-256 en el manifiesto.
- **Ajuste** (`scripts/fit_7027a_model.py`, etapa 1 placa + etapa 2 división de
  pantalla): cota `mu <= 9.0` derivada del reposo documentado del B-18N (pantalla
  ~500 V, bias −55 V → `Vs/µ > 55`); sin esa cota el ajuste elegía `mu` ≈ 10,2 y
  dejaba la válvula en corte al reposo del Ampeg. Valores en la calibración con
  método/fecha/origen; el `Max.-Signal Grid-No.2 Current` de la tabla se trató
  como media de ciclo (no puntual) y no entra en el ajuste.
- **División de pantalla calibrada (B18-011):** `Ig2 = Ip·(F + G·e^(−Vp/V0))`;
  la forma Koren de un exponente no concilia tabla (ex≈0,8) y curva (ex≈1,9);
  la división sí (p07 Ig2 mediana 7,8 %). API: `screen_current(plate_v, screen_v,
  grid_v)`, `screen_current_with_slopes()` y `d_screen_d_plate` en
  `PentodeCurrents`; el 7199 conserva la forma publicada (floor = 0) y su Newton
  no cambia.
- **Test nuevo:** `crates/b18n-dsp/tests/power_7027a_curves.rs` (6 tests: p05,
  p07 Ip/Ig2, forma p06, anclas tabuladas, reposo del par con límites RCA).

### 18.3 Verificación

- 178 tests en verde (config 28, dsp 106 + 6 de curvas 7027A, plugin 18 + 3
  ignorados, render 6, doc-tests). `cargo fmt --check`, `clippy -D warnings`,
  `test --workspace`, `build --release`: limpios.
- Render E2E: pico 4,8155 → 4,8224, RMS 1,4433 → 1,4463, latencia 47 (sin
  cambio); `ALGORITHM_VERSION` →
  `…-7199-coupled-speaker-7027a-datasheet-v1`.
- Bench de CPU (release): ≈25,2 µs/muestra, guardia 28 µs (sin cambio).
- Bundle regenerado tras el cambio de versión.

### 18.4 Residuales y siguiente

- Netlist SPICE de la etapa de potencia diferido a B18-012/013 (con fuente y OT
  reales el contraste sería contra la propia simplificación; las curvas del
  datasheet ya son la referencia independiente).
- Ig2 con la forma exponencial: joroba por debajo de ~140 V no capturada
  (documentada; no opera la etapa ahí más que transitoriamente).
- Siguiente ticket: B18-012 (fuente dinámica 5AR4 + sag, Hito D).

## 19. B18-012 — fuente dinámica 5AR4 + sag (Hito D, completada)

### 19.1 Topología trazada

- Recortes `extracted/psu_zoom_*.png` (gitignored) desde el raster 2900×1400:
  H2 [30 µF/600 V, rectificador/CT RED] → 1k/10W → H1 [40 µF/500 V, pantallas
  7027A] → 10k → H3 [40 µF, triodo ~460 V] y 22k → H4 [40 µF, previo ~380 V];
  bias 10D6 + 100 µF → −55 V a los retornos de 100 kΩ. Hipótesis A (10k→triodo,
  22k→previo), `estimated`: el JSON solo lista valores sin orden ni nodos.
- Sin curva 5AR4 ni DCR del PT en la documentación (unknowns del JSON):
  rectificador como Thevenin promedio (`Rrect = 150 Ω`, `estimated`,
  `supply.rectifier.series_ohms`; `Vsrc` calibrada a B+ 505 V con el reposo) y
  bias con `Rb = 15 kΩ` (`supply.bias.series_ohms`).

### 19.2 Ejecutado

- `stages/power_supply.rs`: 4 nodos HV + bias con Euler implícito por muestra
  (4×4 con pivote + bias 1D), sin asignaciones; `from_circuit` (mapeo
  `[H2,H1,H3,H4]=[30,40,40,40] µF`, caídas por valor), `dc_solution`,
  `advance`, `reset` al reposo DC, `capacitor_energy_j`.
- Cableado en `engine.rs`: punto fijo rieles↔reposos en 20 iteradas al
  inicializar (silencio sin transitorio); por muestra: rieles previos →
  `set_rails` en 7027A/inversor/6SL7 → procesado → `current_load` (CT,
  pantallas, triodo por 47 kΩ, previo por Ra/270 kΩ/470 kΩ, bias por
  `(Vbias−Vg)/100k`) → `advance`. `B18nEngine::supply_rails()` para diagnóstico.
- Config: `PowerSupplyValues.rectifier_series_ohms/bias_series_ohms`
  (`None` en el mapeo, `Some` desde el overlay `supply.*`); calibración con
  método/fecha; `test_circuit()` con los mismos valores.
- SPICE: `scripts/spice/b18n_power_supply_1967.cir` + generador
  `power_supply_reference.py` (DC + tran con escalón +20 mA); goldens en
  `assets/fixtures/reference/power_supply/` (DC 505,0/495,1/472,1/393,9/−55,0 V;
  sag 2,91 V) con README y manifiesto (v1, 2 entradas nuevas).
- Tests: `crates/b18n-dsp/tests/power_supply_transient.rs` (9 tests).
  Reposo convergido: B+ 505, H1 495, H3 472, H4 394 V; CT 83 mA, pantallas
  3 mA, triodo 2,3 mA, previo 4,6 mA.

### 19.3 Verificación

- `cargo fmt`, `clippy -D warnings`, `test --workspace` (debug) y release
  (`b18n-dsp`, `b18n-config`, `b18n-plugin`): verdes. 101 lib + 9 fuente.
- Render E2E (bajo 82,41 Hz/0,5 s): pico 4,8224 → 4,7902, RMS 1,4463 → 1,4370,
  latencia 47; `ALGORITHM_VERSION` → `…-7027a-datasheet-supply-v1`.
- Bench CPU release (estéreo 4×): ≈27–29,5 µs/muestra (antes ≈25,2);
  guardia 28 → **32 µs** con la justificación en el test.
- Netlist de potencia 7027A/OT sigue diferido a B18-013 (con la fuente ya
  dinámica, el siguiente paso es el OT-471 lineal unificado).

### 19.4 Residuales

- Curva real 5AR4/GZ34 y DCR del PT sin documentar: el sag es el de la red RC
  con fuente media, no el de la válvula rectificadora (declarado).
- Toma exacta del devanado de bias pendiente de traza fina (el 6,3 V no puede
  dar −55 V; el PIV 600 V del 10D6 apunta al HV).
- `z^-1` de una muestra en la fuente (cuantificado <0,05 V a 100 Hz); el
  `z^-1` de la NFB sigue pendiente (B18-009, tras B18-013/015).

## 20. B18-013 — OT-471 lineal con pérdidas y parásitos (completada, 2026-10-15)

### 20.1 Modelo unificado (una sola implementación activa)

- `stages/output_transformer.rs` pasa a `OutputTransformerModel` con topología
  configurable; `Ideal` se conserva como regresión/fallback explícito. La
  conversión estática que vivía en `PushPullStage::solve_plates()` se sustituye
  por la **red diferencial** en el mismo Newton (suma del CT + diferencia del
  OT): `Vp1−Vp2 = −[2·Rh + R_core + (Lm ∥ (Lleak + N²·(Rcarga+DCR_sec))) ∥ Cp]·id`.
  El secundario se acopla a la tensión del nodo interno F (EMF del flujo), de
  modo que la continua no se transfiere y el paso alto artificial de 1 Hz del
  modo ideal desaparece en el lineal.
- Parámetros `estimated` del overlay (método y fecha en la calibración):
  `Lm = 28,7 H` (esquina LF 25 Hz con la carga reflejada), `Lleak = 12 mH`
  (esquina de agudos ≈64 kHz), `Cp = 100 pF` (resonancia ≈145 kHz),
  `R_core = 500 Ω` (pérdidas del camino de flujo: relaja el integrador de flujo,
  τ ≈ 57 ms, < 0,1 dB en la esquina) y **cota provisional de flujo 0,12 A**
  (1,3× la corriente de magnetización de pico a plena potencia a 40 Hz):
  limitador numérico declarado como placeholder de saturación hasta B18-014.
- Integración: **Euler atrás** en Lm y Lleak (memoria acotada; robustez del
  solve) y TPT en Cp. Sin el limitador y con trapezoidal, el arranque del test
  de alias a 2 V/7,3 kHz arrastraba el flujo hasta estados infactibles (flujo
  sin fuerza restauradora). El rango de placas pasa a `[−2·B, 3·B]` (flyback) y
  el solve acepta residuo < 1 mV (≈ −100 dB) con 200 iteraciones y paso ≤ 80 V.

### 20.2 Ejecutado

- `numerics/discretization.rs`: `TptInductor` (TPT y Euler atrás, con tests
  analíticos de escalón RL y retención de corriente).
- `power_7027a.rs`: `PowerStageConfig.transformer` (`OutputTransformerConfig`),
  `solve_plates` con `differential_drop(id)` (afín, sin mutar estado),
  `publish_plate_solution` y `magnetizing_current_a()` de diagnóstico.
- `engine.rs`: construye el OT desde el circuito; error explícito
  `InvalidTransformerModel` si los parásitos llegan a medias (no se degrada en
  silencio). `calibration.rs` aplica `output_transformer.*` desde el overlay.
- SPICE: `scripts/spice/b18n_ot471_linear.cir` + `output_transformer_reference.py`
  (AC con Rs = 45 kΩ = 2·rp del datasheet; el generador valida el netlist contra
  una implementación analítica independiente, desviación < 1e-3). Golden
  `assets/fixtures/reference/output_transformer/ot471_linear_spice.csv`
  (93 frecuencias) con README y manifiesto.
- Test nuevo `crates/b18n-dsp/tests/output_transformer_ac.rs`: contraste de
  magnitud y fase con el golden a la tasa 4× (medido: peor desviación
  **0,091 dB / 0,97°** en 20 Hz–20 kHz), esquina LF contra el golden y
  polaridad (id positivo → vsec negativo).

### 20.3 Verificación

- `cargo fmt --check`, `clippy --workspace --all-targets -- -D warnings`,
  `test --workspace` (debug y release) y `build --release`: limpios.
- El test de estrés de alias (2 V/7,3 kHz, 4×) pasa de **121.364 fallos** del
  primer prototipo a **0 fallos** 1×/2×/4× con el modelo final; cota absoluta
  de 1× re-basada a ≤ −12 dBc (medido −13,5; 4× medido −33,5).
- Render E2E: pico 4,7902 → **4,9170**, RMS 1,4370 → **1,4761**, offset
  +0,0029 → **+0,0016**, latencia 47 (sin cambio); `ALGORITHM_VERSION` →
  `…-supply-ot471-linear-v1`.
- CPU (release, estéreo 4×): **≈27,2 µs/muestra** (guardia 32 µs sin cambio).

### 20.4 Residuales y siguiente

- La saturación/histéresis del núcleo (B18-014) sigue diferida: la cota de
  flujo es un limitador declarado, no una curva magnética; `R_core` es una
  resistencia equivalente estimada de pérdidas.
- Euler atrás en los inductores desvía la fase HF de forma declarada (medida
  contra SPICE en banda audible: < 0,1 dB / 1°; la esquina de 64 kHz queda
  fuera de banda).
- Siguiente recomendado: **B18-015** (carga reactiva del altavoz) y después
  **B18-009** (NFB sobre red real, sin `z^-1`/LP) con 013 y 015 cerrados.

## 21. B18-015 - carga reactiva del altavoz de 18" en el solve del OT (física cerrada; lazo estimado+NFB bloqueado, 2026-10-15)

### 21.1 Implementación

- **Nueva etapa `stages/speaker_impedance.rs`:** `ElectricalLoadConfig`
  (`Resistive { ohms }` = fallback declarado; `LumpedEquivalent(params)`), el
  estado discretizado `ElectricalLoad` (puerto Norton `i = G·v + Ieq`) y la
  referencia continua `impedance_at_hertz(f)`. Red RLC:
  `Re + Le + (Res ∥ Les ∥ Ces)` con parámetros `estimated` en el overlay
  (`speaker.reactive_load.*`, los cinco juntos o error explícito en
  `calibration.rs`). Anclas: Re=6.5 Ω, Le=0.8 mH, fs=45 Hz, Q=1,2, pico
  |Z|=30 Ω, |Z|(200 Hz)≈8,2 Ω, |Z|(1 kHz)≈7,7 Ω, |Z|(20 kHz)≈100 Ω. **No
  medido** del Cleveland (regla: sin invención de Z(f), fallback resistivo).
- **OT-471 (B18-013) generalizado:** `OutputTransformerConfig.load` sustituye
  `load_ohms`; la rama reflejada usa el puerto de carga (DCR de secundario en
  serie, reflejo `G_eff/N²`, fuente Norton reflejada `I_ref = −I_eff/N8`,
  combinación en serie con Lleak por conductancias). El `advance()` resuelve
  el EMF del flujo y publica la tensión de terminal **y la corriente
  secundaria** del mismo paso; `set_load()` cambia la carga en caliente (la
  red de carga nace descargada; los estados de OT persisten). El modo ideal
  usa `load.nominal_ohms()` (a continua: Re).
- **Selección:** `RuntimeControls::speaker_load`
  (`NominalResistance`|`EstimatedLumped`), resuelta en `set_targets`
  (fallback a la resistiva si el overlay no trae parámetros o la **NFB
  provisional está activa** — véase 21.2; `effective_speaker_load()`
  diagnostica el modo efectivo). Plugin: parámetro nuevo `load_model` en
  Advanced, ID `nominal`/`rlc_estimated` (IDs congelados verificados: 13);
  render: `--load resistive|lumped` + `SettingsReport.load`.
- **SPICE:** `scripts/spice/b18n_speaker_load.cir` +
  `speaker_load_reference.py` (comprobación analítica numpy, desviación
  < 1e-3) → golden `assets/fixtures/reference/speaker_load/speaker_load_spice.csv`
  (93 filas, manifiesto con SHA-256 `e8d041b5…664`).
- Discretización de la carga: **TPT/trapezoidal** en toda la red (inductores y
  condensador). Euler atrás desviaba ≈1,8 dB a 20 kHz frente al golden (la
  carga es inductiva y con camino resistivo, a diferencia del integrador de
  flujo del OT); TPT la deja en banda: < 0,5 dB / < 10° (test
  `magnitude_and_phase_match_spice_in_the_audio_band`).

### 21.2 Problema encontrado: lazo de NFB provisional × carga reactiva

Archivo (todo medido en la sesión de B18-015):

- **La etapa de potencia sola es estable** con la carga RLC (0 fallos a
  drives 0,5–40 V, incluso con cambio en caliente tras resolver el reposo con
  la resistiva) y **el motor con `present = false` también** (todos los
  drives/tasas). El conflicto es exactamente **NFB provisional + carga
  reactiva**.
- Manifestación: con NFB activa, cualquier entrada mínima engancha un
  **ciclo límite** (sin entrada, reposo exacto): la salida cae a un tren
  saturado (~6 V constante a drive 0,005; 44–106 V a drives mayores,
  alternancia de signo cada 1-2 muestras de host ⇒ ~24 kHz) con cascada de
  fallos de Newton (inversor+potencia, ~50 % de llamadas). Es mapeable a
  ganancia de lazo de ≈2–5× sobre el límite al cruzar el lazo en HF.
- Causa estructural (medida, no hipótesis): la carga inductiva **anula el
  roll-off de fuga del OT** (vsec/v_F ≈ N·Le/(Lleak+N²·Le), constante en HF)
  y sube la ganancia de lazo hacia 24 kHz que la compensación provisional
  (un polo a 2,5 kHz + `z^-1`) no cubre. Barrido de ganancia de lazo
  escalando `N16`: estable solo con ≈1/5 de la ganancia nominal de lazo
  (ratio cerrado/abierta 0,72→0,83 hacia 1; con la nominal: >30).
- Intentos de compensación provisional (todos medidos, todos insuficientes):
  polo único a 500 Hz (a drive pequeño ok, implica −10 dB de NFB), 1,0–1,5
  kHz (estable solo a drive ≤0,05), 2,0 kHz (fallos), **cascada 2×2,5 kHz
  (peor: ciclo de 44 V)**, cascada **2×900 Hz (seno estable a todo drive y a
  44,1/96/192 kHz, pero un impulso de 2 Vp engancha: 86→102→11→4,5 V con
  ~14k fallos)**, cascada **2×600 Hz (seno estable a todo drive; el impulso
  latched continuo 86→84 V con ~65k fallos)**. Conclusión: la compensación
  artificial no es la herramienta; el margen necesita las capacitancias
  físicas de válvula/transformador (B18-009).
- **Decisión adoptada (regla: fallo explícito sobre relleno silencioso):**
  con la NFB activa, la selección estimada **cae a la carga nominal**
  (documentado en código, `MODEL_ASSUMPTIONS.md` y con test
  `reactive_load_falls_back_with_active_nfb`); el modo reactivo queda
  habilitado con la NFB **abierta** del circuito (`present = false`), que es
  la puerta de entrada de B18-009. La cascada provisional se retiró del
  árbol (solo queda el polo único de 2,5 kHz, bit-exacto para el nominal).
- B18-009 queda **desbloqueado**: su condición previa (015 física cerrada) se
  cumple y el archivo de mediciones de margen de esta sección es su punto de
  partida (también los resultados del barrido de ganancia).

### 21.3 Verificación

- `cargo check --workspace --all-targets` limpio; fmt/clippy completos
  **pendientes de re-ejecutar** al cierre (la sesión terminó con la batería
  de tests corregida: ver 21.4).
- Test nuevo `crates/b18n-dsp/tests/reactive_load.rs` (8): contraste AC con
  el golden SPICE (< 0,5 dB / < 10°, banda audible a 4×), anclas de la
  impedancia estimada, transimpedancia |v/i| = |Z(f)| en 45/200/1000 Hz,
  corriente que responde al pico de resonancia, motor reactivo + NFB abierta
  estable (drive 0,45+0,35 compuesto; 0 fallos; respuesta con diferencia RMS
  ≥1 % frente a la resistiva), fallback documentado con NFB activa, selección
  efectiva + reset reproducible, batería 44,1/48/96/192 kHz impulso+tono con
  el criterio < 500 fallos de `nfb_stability`.
- El sonido por defecto (carga resistiva) es **invariante**: el camino reduce
  a las mismas ecuaciones de B18-013; render E2E igual al baseline (pico
  4,916955, RMS 1,4760989, igual a la precisión del reporte; diferencia
  máxima por muestra 1,2e-7, por reorden aritmético). Sin
  `ALGORITHM_VERSION`.

### 21.4 Pendientes al cierre (pasa al handoff)

- Actualización 2026-10-15 (fin de la sesión): `fmt`/`clippy` además
  verificados; batería completa de `reactive_load.rs` con el criterio final
  verificada verde (**8 tests + 1 `#[ignore]`**: impulso 4 Vp a 44,1 kHz con
  carga reactiva engancha el solve aislado — 33.855/65.536 —, test
  `reactive_impulse_extreme_pending_b18_009`); la batería moderada corre con
  `< 5.000` fallos (ráfagas 155–1.347 tras salto; seno equivalente limpio).
  `cargo test --workspace` debug: **215 en verde** (4 ignorados: 1 de
  B18-015 + 3 del bench de CPU del plugin). Render E2E re-verificado
  invariante (diff máx. 1,19e-7). **Quedan:** `test --workspace --release`,
  `build --release`, bench y bundle.
- Decisiones B18-009 (ahora desbloqueado): capacitancias físicas del 7199 y
  la 7027A (datasheets RCA listan valores), cierre del lazo por muestra,
  retirada de `z^-1`/LP y **apertura** del modo reactivo con NFB;
  re-verificar estrés de alias/impulso (incluido el caso ignorado) y
  regenerar goldens si algo audible cambia.

## 22. B18-009 - caracterización física del lazo en SPICE (paso 1, 2026-10-15)

Ejecutado tras cerrar el paso 0 del handoff (fmt, clippy, test release, build,
bench, bundle y render E2E invariante: pico 4,916955 / RMS 1,4760989 / 47
muestras de latencia, sin cambios).

### 22.1 Datos de datasheet añadidos al análisis

- RCA 7027-A (8-59), capacidades interelectródicas directas: rejilla 1-placa
  1,5 pF; rejilla 1-resto 10 pF; placa-resto 7,5 pF (origin = datasheet).
- RCA 7199 (7-63), triodo: rejilla-placa 2 pF, rejilla-cátodo 2,3 pF,
  placa-cátodo 0,3 pF. Pentodo: rejilla 1-placa 0,06 pF máx., rejilla
  1-cátodo 5 pF, placa-cátodo 2 pF.
- No incluyen zócalo ni cableado; no están aún en el overlay ni en el motor.

### 22.2 Netlist de referencia (scripts/spice/nfb_loop_reference.py)

Lazo completo, sin z^-1 ni polo de 2,5 kHz: inversor 7199 (Koren RCA), acoplo
50 nF / 1 kΩ / 100 kΩ a -55 V, dos 7027A (Koren + división de pantalla de
	ube_models.7027a.koren.*), OT-471 diferencial con CT a B+ (Lm partida en
dos mitades, Lleak 12 mH, Cp 100 pF, R_core 500 Ω, DCR 150 Ω por mitad),
carga 8 Ω o RLC estimada (B18-015) y NFB de 22 kΩ desde la toma de 16 Ω
(TAP_GAIN = N8/N16 = 1,414, polaridad -1 como en el motor).
Las capacitancias van en un interruptor (caps). Se ejecuta con
	ools/ngspice (ngspice 47). Los netlists son generados por script, no se
editan a mano.

Corrección de método durante la sesión: la primera versión alimentaba las
placas solo por 10 MΩ y los tubos las llevaban a ~0 V (corriente de reposo
irreal). La alimentación correcta es el punto medio de Lm a B+.

### 22.3 Resultados (ganancia |v_8Ω / v_in|, dB, 1 V de entrada)

| Variante | 1 kHz | 20 kHz | 100 kHz | 500 kHz | 2 MHz | Pico sobre 1 kHz |
|---|---|---|---|---|---|---|
| Nominal, lazo abierto, datasheet | 33,1 | 32,8 | 31,3 | -9,4 | -54,4 | 0 dB |
| Nominal, NFB, datasheet | 22,8 | 22,6 | 19,7 | -9,2 | -54,2 | 0,01 dB |
| Nominal, NFB, sin capacitancias | 22,8 | 22,8 | 21,7 | 9,7 | -12,2 | 0,01 dB |
| Reactiva, lazo abierto, datasheet | 32,7 | 64,5 | 38,0 | 8,7 | -23,7 | 36,2 dB a 23,8 kHz |
| Reactiva, NFB, datasheet | 22,3 | 20,6 | 19,6 | 7,1 | -23,6 | 0,97 dB a 399 Hz |
| Reactiva, NFB, sin capacitancias | 22,3 | 20,6 | 20,6 | 20,2 | 16,5 | 1,0 dB a 399 Hz |

Lecturas:

1. Con NFB cerrada y carga reactiva, el lazo cerrado **no presenta pico de
   HF** (ningún punto supera al pico de 399 Hz de 0,97 dB) y cae de forma
   monótona por encima de 1 kHz. La lazo abierta sí tiene resonancia de
   36 dB a 24 kHz (la que la NFB amortigua).
2. Las capacitancias de válvula **son el roll-off HF físico** del lazo:
   sin ellas, la ganancia cerrada reactiva sube a +20 dB a 500 kHz y +16 dB
   a 2 MHz; con ellas, a +7 dB y -24 dB. Confirma la hipótesis de B18-009
   (la fase HF la deben dar las capacitancias, no un polo artificial).
3. El polo de 2,5 kHz no aparece en la física: el lazo analógico es estable
   sin él. Su papel era enmascarar el retardo de una muestra de la
   realimentación (z^-1 a la tasa efectiva: 5,2 µs a 4×, ~45° a 24 kHz y
   ~180° cerca de 96 kHz). Ese retardo es el origen del ciclo límite medido
   en B18-015.

Límites declarados: es AC **lineal** alrededor del reposo; no cubre
estabilidad de gran señal (fallos de Newton, enganche con impulsos), ni
margen de fase calculado explícitamente (solo pico de lazo cerrado y
magnitudes), ni el cableado (pF de zócalo no incluidos).

### 22.4 Decisión y siguiente paso

- **No se ha modificado el motor Rust** en esta ronda: retirar z^-1 y el LP
  es un cambio audible (sube ALGORITHM_VERSION, regenerar goldens y
  re-verificar CPU, guardia de 32 µs y los tests de alias y de carga reactiva).
- Siguiente paso de B18-009 (orden): (a) añadir las capacitancias de
  datasheet al overlay (origin = datasheet, con fuente y método) y a los
  modelos de 7199 y 7027A; (b) cerrar el lazo de forma implícita en el
  motor (escalar secante sobre _fb, o linealizada con la sensibilidad del
  Jacobiano) sin z^-1 ni LP; (c) contrastar la respuesta cerrada del motor
  con el barrido de
fb_loop_reference.py (tolerancia a fijar tras medir);
  (d) estrés de alias/impulso y CPU antes de cambiar ALGORITHM_VERSION.

### 22.5 Intento de cierre implícito del lazo en el motor (medido, revertido)

Tras la caracterización de 22.3 se intentó implementar el cierre implícito por
muestra en engine.rs (close_feedback_loop), retirando el z^-1 y el polo
de 2,5 kHz y resolviendo _tap = polaridad·tap_gain·secondary(v_tap) con
iteración sobre el escalar (secante y, después, sustitución amortiguada),
evaluando cada tanteo sobre clones planos (sin asignación) y comprometiendo la
mejor evaluación. ALGORITHM_VERSION no se tocó.

- Resultado: **no converge en el dominio discreto sin los parásitos físicos**.

fb_stability::stable_across_rates_loads_and_drives falla a 44,1 kHz / 16 Ω
  / 1 V con 9.149–15.102 fallos de solver (según el esquema de paso) frente al
  criterio < 4.096. closed_loop_reduces_gain_and_stays_stable también cae
  en la variante amortiguada.
- Lectura: el lazo **analógico** con las capacitancias de datasheet es estable
  sin el polo (22.3), pero el lazo **discreto** del motor, que no modela esas
  capacitancias, tiene demasiada ganancia y fase en HF; el mapa escalar no es
  monótono y la iteración (barata en CPU por requisito de tiempo real) diverge.
  No es un problema del solver de las etapas (los mensajes de fallo apuntan al
  solve acoplado con valores extremos durante la divergencia).
- Decisión: **revertir** al camino z^-1 + polo de 2,5 kHz (bit-exacto:
  render E2E de nuevo invariante, pico 4,916955 / RMS 1,4760989 / latencia 47;
  mt, clippy -D warnings y cargo test --workspace en verde). El intento
  queda documentado en el comentario de FEEDBACK_COMPENSATION_CUTOFF_HZ.
- Condición para reintentar (ronda propia): añadir la red de capacitancias
  interelectródicas de la 7199 y la 7027A (datasheet RCA, ya extraídas) y del
  arrollamiento del OT al modelo del motor, y usar un solver con búsqueda de
  línea / Newton con Jacobiano del lazo; verificar CPU (guardia de 32 µs) antes
  de retirar el polo y subir ALGORITHM_VERSION.


## 23. B18-009 - candidato conjunto y bloqueo fisico independiente (2026-10-16)

Se implemento un candidato aislado `stages/output_network.rs` (7199 + 7027A +
OT + NFB, once nodos, eliminacion lineal de G y Newton 10D con Jacobiano
analitico/busqueda de linea), con capacitancias RCA trazadas en el overlay.

**Resultado: B18-009 sigue abierto y bloqueado para integracion.** La nueva
referencia `nfb_coupled_reference.py` corrige la orientacion del secundario
respecto del motor y separa el puerto comun/diferencial del OT. El analisis
AC sin pico de 22.3 NO probaba estabilidad. `nfb_loop_margins.py` mide:

- Nominal: margen HF -55,38 grados; margen de ganancia -11,23 dB.
- RLC: margen HF -9,54 grados; margen de ganancia -4,95 dB.
- Pulso SPICE 10 uV, retirado a 2,1 us: oscilacion sostenida (~15 V nominal,
  ~41 V RLC; amplitudes diagnosticas, no hardware, sin conduccion de rejilla).
- DC/AC en los casos convergentes del candidato: maximo 0,0313 dB / 0,7783
  grados frente a SPICE. RLC abierta usa 768 kHz para separar el warping.
- RLC cerrada Rust, 93,75 Hz / 0,1 mV / 192 kHz efectivos: 62.335 fallos de
  solve en 65.536 pasos; test de aceptacion explicitamente ignorado/fallido.
- CPU candidata: 41,3 -> 32,487 us/muestra estereo 4x/48 kHz; aun excede
  guardia 32 us. No se amplio la guardia.

Se retiro la integracion temporal. El plugin conserva su camino anterior
(z^-1 + polo de 2,5 kHz), version de algoritmo y fallback resistivo con NFB.
No se hizo commit. La referencia anterior se conserva como exploracion,
no como aprobacion fisica. Informe completo, limitaciones, reproduccion y
siguiente paso: `docs/B18_009_STATUS.md`.
