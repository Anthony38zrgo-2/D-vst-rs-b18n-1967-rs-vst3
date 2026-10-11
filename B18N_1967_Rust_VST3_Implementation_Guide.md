# B-18N Portaflex 1967 — Guía de implementación DSP en Rust y exportación VST3

**Tipo:** especificación de desarrollo / instrucciones para una implementación progresiva.  
**Objetivo:** emular el **circuito B-18N/B-15ND, revisión 5/67**, usando Rust, producir un procesador de audio independiente del host y empaquetarlo como efecto **VST3**.  
**Fuente de parámetros obligatoria:** [`B18N_1967_simulation_parameters.json`](../assets/models/B18N_1967_simulation_parameters.json).  
**Documentación del circuito:** [esquema 5/67](https://vintage-blue.com/assets/b18n-b15nd_schematic_5_67.pdf) y [manual técnico B-18N 1967](https://vintage-blue.com/assets/b18n_1967_service_manual.pdf).  
**Estado de partida:** el JSON aporta parámetros y referencias, **no** una netlist eléctrica completa ni las curvas de todas las válvulas. Por tanto, un VST3 funcional no garantiza todavía una emulación físicamente validada.

> **Regla central:** no inventar topologías, valores ni mediciones para rellenar vacíos del JSON. Todo parámetro calculado o estimado debe ir en un archivo de calibración separado y quedar identificado como `estimated`, `measured` o `datasheet`. No atribuir al hardware de 1967 características añadidas para mejorar el sonido.

---

## 1. Entregables y criterios de éxito

El proyecto debe producir:

1. Una librería Rust **`b18n_dsp`** reutilizable, sin dependencia de NIH-plug, interfaz gráfica, DAW o API VST3.
2. Una librería **`b18n_config`** para leer, validar y transformar el JSON histórico en datos tipados seguros para DSP.
3. Un ejecutable **`b18n_render`** para procesar archivos WAV sin DAW, medir resultados y generar datos de prueba.
4. Un plugin **`b18n_plugin`** compatible con VST3; CLAP se puede ofrecer además con el mismo DSP.
5. Pruebas automáticas de puntos de trabajo, respuesta de controles, estabilidad, antialiasing, latencia y consumo de CPU.
6. Presets, opciones de calibración y documentación separadas de los valores históricos.

**Criterio de aceptación global:** el plugin abre en un DAW, procesa mono/estéreo sin clics o NaN, conserva ajustes entre sesiones, automatiza parámetros, pasa el validador VST3 y reproduce resultados del motor offline para idéntica entrada y configuración.

**Fuera de alcance inicial:** renderizado 3D, motor gráfico, un clon exacto del gabinete sin una IR real, simulación térmica de filamentos y diseño de un host VST propio.

---

## 2. Tecnología y arquitectura

- **Lenguaje:** Rust estable; fijar versión probada en `rust-toolchain.toml`.
- **Plug-in:** [NIH-plug](https://github.com/robbert-vdh/nih-plug) para el adaptador VST3/CLAP, parámetros, estado y empaquetado. El proyecto original está en modo de mantenimiento; fijar un commit y reevaluar su mantenimiento antes de una distribución prolongada.
- **DSP:** Rust propio. `f64` internamente en solucionadores no lineales y matrices pequeñas cuando aporte estabilidad; buffers públicos `f32`.
- **Datos:** `serde` / `serde_json` exclusivamente en la capa de configuración y en herramientas offline, no dentro del bucle de audio.
- **Interfaz:** iniciar con la GUI genérica del DAW; añadir `nih_plug_egui` solo cuando el DSP esté medido y estable.
- **Tests:** `cargo test`, `cargo clippy`, `cargo fmt`, harness WAV, barridos y comparación con referencia SPICE o mediciones cuando estén disponibles.
- **Empaquetado:** `nih_plug_xtask` con `cargo xtask bundle b18n_plugin --release`.

### Dependencias permitidas (dirección única)

```text
assets/models/*.json ──────> b18n_config ──────> b18n_dsp (tipos de modelo)
                                               ↑
                                    b18n_plugin ├─> NIH-plug / VST3
                                    b18n_render └─> WAV / CLI / mediciones
```

El esquema expresa el **flujo de datos**. En Cargo, la relación de crates recomendada es:

```text
b18n_config ──depende de──> b18n_dsp (solo tipos de parámetros de modelo)
b18n_plugin ─depende de───> b18n_config + b18n_dsp + nih_plug
b18n_render ─depende de───> b18n_config + b18n_dsp
b18n_dsp ────no depende de──> los otros crates
```

`b18n_dsp` no conoce los nombres de archivos ni sabe que existe un VST3. `b18n_config` es el *adaptador* que traduce el JSON a `CircuitDefinition`. Los ajustes del usuario van en `RuntimeControls`, no modifican `CircuitDefinition`.

---

## 3. Estructura de carpetas y responsabilidad de archivos

```text
b18n-1967/
├── Cargo.toml                         # Workspace, perfiles y reglas comunes
├── Cargo.lock                         # Versiones reproducibles: versionar
├── rust-toolchain.toml                # Toolchain fijado tras validación
├── bundler.toml                       # Nombre del paquete VST3/CLAP
├── LICENSE                            # Licencia definida tras revisar NIH-plug/VST3
├── README.md                          # Inicio rápido y estado del proyecto
├── CHANGELOG.md
├── .gitignore
├── .cargo/
│   └── config.toml                    # Alias 'cargo xtask'
├── .github/
│   └── workflows/
│       ├── ci.yml                     # fmt, clippy, tests, checks
│       └── release.yml                # bundles Win/macOS/Linux cuando proceda
│
├── assets/
│   ├── models/
│   │   ├── B18N_1967_simulation_parameters.json  # CANÓNICO, no editar silenciosamente
│   │   ├── b18n_1967_calibration.example.json     # Solo inferencias/ajustes
│   │   └── README.md                              # Procedencia, fecha, hashes
│   ├── ir/
│   │   └── README.md                   # IR externa/medida: no inventar IR histórica
│   └── fixtures/
│       ├── signals/                    # Seno, multisinus, impulsos, transitorios
│       └── reference/                  # Goldens verificados; no WAV inventados
│
├── crates/
│   ├── b18n-dsp/
│   │   ├── Cargo.toml
│   │   ├── src/
│   │   │   ├── lib.rs                   # API pública estable
│   │   │   ├── engine.rs                # B18nEngine: ciclo de vida y cadena DSP
│   │   │   ├── circuit.rs               # CircuitDefinition (tipos físicos)
│   │   │   ├── controls.rs              # RuntimeControls (mandos de usuario)
│   │   │   ├── state.rs                 # Estado temporal / punto de reposo
│   │   │   ├── audio_context.rs         # Sample rate, buffer, latencia
│   │   │   ├── stages/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── input.rs              # Impedancia y selección de entradas
│   │   │   │   ├── triode_6sl7.rs        # Modelo I(Va,Vg), cátodo y acoplo
│   │   │   │   ├── tone_stack.rs         # Red RC original dependiente de potes
│   │   │   │   ├── channel_mixer.rs      # 270 kΩ por canal según conexión real
│   │   │   │   ├── pentode_7199.rs       # Pentodo de la 7199
│   │   │   │   ├── phase_splitter.rs     # Triodo inversor de la 7199
│   │   │   │   ├── power_7027a.rs        # Par push-pull, bias, pantalla, grids
│   │   │   │   ├── power_supply.rs       # B+, 5AR4, filtros y caída de tensión
│   │   │   │   ├── output_transformer.rs # OT-471 y carga reflejada
│   │   │   │   ├── feedback.rs           # Red NFB y polaridad
│   │   │   │   └── cabinet.rs            # Impedancia de altavoz + IR (separables)
│   │   │   ├── numerics/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── mna.rs                # Modified Nodal Analysis, redes RC
│   │   │   │   ├── nonlinear.rs          # Newton amortiguado o equivalente
│   │   │   │   ├── discretization.rs     # Integración estable, TPT/trapezoidal
│   │   │   │   ├── oversampling.rs       # Up/downsampling con filtros previos
│   │   │   │   └── interpolation.rs      # Tablas de curvas de válvulas
│   │   │   └── utilities/
│   │   │       ├── db.rs
│   │   │       └── smoothing.rs
│   │   ├── tests/
│   │   │   ├── dc_operating_points.rs
│   │   │   ├── tone_stack_response.rs
│   │   │   ├── triode_and_power.rs
│   │   │   ├── oversampling_aliasing.rs
│   │   │   ├── dynamic_sag.rs
│   │   │   └── stability.rs
│   │   └── benches/
│   │       └── process_block.rs
│   │
│   ├── b18n-config/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs                   # load_embedded_reference(), etc.
│   │       ├── schema.rs                # Structs serde reflejando claves reales
│   │       ├── validation.rs            # Rango, unidades, nulos y coherencia
│   │       ├── mapping.rs               # JSON -> CircuitDefinition tipada
│   │       └── calibration.rs           # Overlay sin mutar el JSON canónico
│   │
│   ├── b18n-plugin/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs                   # NIH exports, IDs, Plugin/ Vst3Plugin
│   │       ├── params.rs                # IDs estables, valores y smoothers
│   │       ├── processor.rs             # Adaptador de buffer del host
│   │       ├── mapping.rs               # Parámetros DAW -> RuntimeControls
│   │       ├── state.rs                 # Guardado/restauración compatible
│   │       └── editor.rs                # Opcional después del MVP
│   │
│   └── b18n-render/
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs                  # CLI: entrada WAV -> salida WAV
│           ├── wav_io.rs
│           ├── sweep.rs
│           └── report.rs                # CSV/JSON: THD, RMS, CPU, etc.
│
├── xtask/
│   ├── Cargo.toml
│   └── src/main.rs                      # Adaptador del bundler NIH-plug
│
└── docs/
    ├── IMPLEMENTATION_GUIDE.md         # Este documento
    ├── CIRCUIT_TRACEABILITY.md         # Cada nodo => PDF y evidencia
    ├── MODEL_ASSUMPTIONS.md            # Supuestos y precisión
    ├── MEASUREMENT_PROTOCOL.md         # Protocolo A/B y SPICE
    ├── DSP_ARCHITECTURE.md             # Diagramas de bloques/realimentación
    └── RELEASE_CHECKLIST.md            # QA/DAWs/licencias
```

**Principios de clean code aplicables:** responsabilidad única por módulo; nombres vinculados a elementos eléctricos reales; no `utils.rs` genérico gigante; composición en `engine.rs` y no lógica de válvulas dentro de la API VST; unidades explícitas (`_ohms`, `_farads`, `_volts`); errores de configuración tipados; pruebas junto al crate propietario del comportamiento. Evitar fragmentar cada componente en un crate diferente si ello no crea un límite de dependencia real.

---

## 4. Integración del JSON histórico

### 4.1. Instalación del archivo

Copiar el JSON proporcionado **sin modificar sus claves** a:

```text
assets/models/B18N_1967_simulation_parameters.json
```

No utilizar en su lugar el PDF `descarga.pdf` adjunto anteriormente: ese documento contenía un **fragmento de otro esquema B-18N**, y esta implementación apunta expresamente a `identity.schematic_revision = "1967-05"`.

### 4.2. Campos reales a consumir

| Módulo Rust | Ruta de JSON | Interpretación |
|---|---|---|
| `validation.rs` | `identity.schematic_revision` | Exigir `1967-05` para este modelo |
| `validation.rs` | `identity.documented_nominal_output_w` | Referencia de 50 W, **no** limitador DSP |
| `input.rs` | `controls.channel_1_inputs`, `channel_2_inputs` | Selección de conectores, verificar wiring en plano |
| `controls.rs` | `controls.each_channel.volume/bass/treble` | 1 MΩ; ley log en Volume, lineal en Bass/Treble |
| `triode_6sl7.rs` | `preamp.each_channel.*`, `preamp.tube_6sl7gt_reference_at_250v` | Valores de etapas; `mu/rp/gm` son referencia local |
| `tone_stack.rs` | `tone_stack.printed_board_*` | Componentes; **las listas no codifican conexiones** |
| `channel_mixer.rs` | `preamp.each_channel.channel_mixer_resistor_ohms` | 270 kΩ, posición real según esquema |
| `pentode_7199.rs` | `phase_inverter.pentode_*`, `phase_inverter.dc_pin_volts_no_signal` | Valores eléctricos y DC de validación |
| `phase_splitter.rs` | `phase_inverter.phase_splitter_resistors_ohms` | Dos resistencias de 47 kΩ, conectar según PDF |
| `power_7027a.rs` | `power_stage.*` | 505 V placa, 500 V pantalla, −55 V rejilla, etc. |
| `output_transformer.rs` | `output_transformer.*` | OT-471; 4300 Ω primario, 8/16 Ω secundarios |
| `feedback.rs` | `output_transformer.negative_feedback.*` | Resistencia de 22 kΩ; **nodo y fase pendientes del plano** |
| `power_supply.rs` | `power_supply.*` | 5AR4, B+ 505 V, filtros, circuito de bias |
| `cabinet.rs` | `cabinet.*` | Altavoz de 18", 8 Ω; sin IR ni curva de impedancia |
| Tests | `validation.*` | Tolerancias y tipos de comprobación |

### 4.3. Significado de los `null`

El JSON contiene `null` para curvas características, respuesta impulsional, corrientes de reposo y parámetros magnéticos. En Rust:

- Modelar estos campos como `Option<T>` (o equivalentes tipados).
- `None` **no significa** cero, resistencia infinita ni comportamiento ideal validado.
- Si un módulo requiere uno de esos datos, usar un **modelo simplificado con limitación explícita**, solicitar/calibrar un valor externo, o desactivar el módulo. No crear un valor oculto.
- Señalar en el informe de pruebas la fidelidad alcanzada (`topology_unverified`, `approximate`, `calibrated`, `measured`).

**Ejemplos específicos:**

```json
{
  "power_stage": {
    "plate_current_idle_a_each": null,
    "tube_characteristic_curves": null
  },
  "output_transformer": {
    "winding_inductance_h": null,
    "core_saturation_curve": null
  },
  "cabinet": {
    "measured_ir": null
  }
}
```

Este fragmento es **ilustrativo** de las claves existentes: no reemplazar el documento completo con él.

### 4.4. Loader embebido (fuera del callback)

Es preferible embeber el archivo original en el binario VST3; así no depende de una ruta del disco en el equipo del usuario. Ejemplo conceptual en `crates/b18n-config/src/lib.rs`:

```rust
const HISTORICAL_JSON: &str = include_str!(
    "../../../assets/models/B18N_1967_simulation_parameters.json"
);

// Llamar desde initialize() / carga de modelo, NUNCA desde process().
pub fn load_reference() -> Result<CircuitDefinition, ConfigError> {
    let raw: HistoricalB18nJson = serde_json::from_str(HISTORICAL_JSON)?;
    validate_source(&raw)?;
    convert_to_circuit(raw)
}
```

`HistoricalB18nJson` debe reflejar las claves reales; evitar deserializar todo a un `serde_json::Value` dinámico durante la ejecución. Si `CircuitDefinition` necesita completar topología mediante el plano, documentar cada conexión en `CIRCUIT_TRACEABILITY.md`.

**Recomendación:** añadir al manifiesto externo del proyecto una versión de esquema de datos y hash SHA-256 del JSON, sin alterar el original solo para agregar metadatos de compilación.

### 4.5. Archivo de calibración separado

Usar un archivo como `assets/models/b18n_1967_calibration.example.json` para parámetros no respaldados por la documentación:

```json
{
  "target_revision": "1967-05",
  "notes": "Ejemplo ilustrativo; sin calibraciones medidas",
  "assumptions": {},
  "measured": {},
  "provenance": []
}
```

Cada campo futuro debe incluir unidades, origen, método de estimación y fecha. No permitir que este overlay cambie tácitamente los valores originales de resistencias, condensadores o tensiones nominales.

---

## 5. Cadena de señal y responsabilidades DSP

```text
AUDIO INPUT (mono o estéreo)
  │
  ├── Adaptación a tensión física + impedancias de entrada
  │
  ├── CH1 o CH2: 6SL7 triodo 1 -> acoplo RC -> Volume
  │                           -> 6SL7 triodo 2 -> tono pasivo
  │
  ├── Mezcla de canales según resistencias y conexiones reales
  │
  ├── 7199 pentodo -> 7199 triodo inversor de fase
  │                        ├── fase positiva
  │                        └── fase negativa
  │
  ├── 2 × 7027A push-pull con bias fijo
  │            ↕ B+ dinámica (5AR4 + filtros + PT)
  │
  ├── Transformador OT-471 + carga eléctrica del altavoz
  │        └──── NFB: señal desde secundario al nodo del plano ──┐
  │                                                               │
  ├── Etapa electroacústica: impedancia + respuesta cabinet/IR     │
  ├── Atenuación/volumen final no histórico                         │
  └── AUDIO OUTPUT + gestión de latencia                           │
                                                                  └─ retroalimentación
```

**Importante:** el lazo real de realimentación y el transformador forman un sistema acoplado. El diagrama es lógico, no presupone que puedan implementarse como plugins DSP independientes en cascada sin resolver el lazo.

### 5.1. Entrada y dominios de unidades

- Entradas de bajo representadas como tensión, no solo muestras `[-1, 1]`.
- Definir `Vpk_per_full_scale` como una calibración de software controlada o documentada. Ejemplo de convención inicial **no histórica**: `1.0` de amplitud de muestra = `1.0 V pico` antes del ajuste de entrada.
- Mantener por separado `InputTrimDb` (adaptación moderna), `Volume` (potenciómetro histórico) y `OutputTrimDb` (adaptación DAW).
- El VST3 es un **efecto de audio**, no un amplificador físico conectado a un altavoz real; su salida es line-level digital y debe escalarse sin hard clipping no deseado.
- Empezar con flujo mono; para entrada estéreo usar **dos instancias de motor independientes**, una por canal, nunca sumar L+R por defecto, porque puede cancelar señales de fase opuesta.
- Para reproducción original de un solo amplificador, ofrecer modo mono explícito; documentar si el host alimenta ambas instancias.

### 5.2. V1/V2: dos triodos 6SL7 por canal

- Construir un modelo físico con `I_plate = f(V_plate, V_grid, V_cathode)` y, cuando sea pertinente, conducción de rejilla.
- Resistencias del JSON por canal: 470 kΩ / 5.6 kΩ en primer triodo, 220 kΩ / 2.2 kΩ en segundo; acoplos 22 nF.
- No suponer bypass de cátodo: los campos `triode_1_cathode_bypass_farads` y `triode_2_cathode_bypass_farads` son `null`.
- La ficha parcial `mu=70, rp=44 kΩ, gm≈1.6 mS` sirve para iniciar verificaciones de pequeña señal, pero **no** caracteriza toda la región de saturación.
- Resolver las corrientes de polarización e impedancias de carga; conservar memoria en capacitores de acoplamiento.
- El camino de señal y la red de tono deben revisarse con el esquema antes de cerrar la topología.

### 5.3. Tono: NO sustituir por dos biquads independientes

Datos del JSON:

- `printed_board_resistors_ohms = [220000, 22000]`.
- `printed_board_capacitors_farads = [1e-9, 1e-8, 4.7e-10, 4.7e-9]`.
- `additional_resistors_visible_in_full_schematic_ohms = [120000, 270000]`.
- `controls.each_channel.bass/treble.ohms = 1000000` con ley lineal.

**La posición de cada componente NO se deduce del orden de los arrays.** Digitalizar las conexiones desde el recuadro `Printed CKT` del manual y el esquema 5/67. Resolver la red con **análisis nodal modificado (MNA)** o un modelo WDF equivalente; las posiciones de Bass y Treble modifican simultáneamente la respuesta. Incluir impedancias de origen y destino.

Prueba requerida: medir la función de transferencia para Bass y Treble en 0/25/50/75/100 % (25 combinaciones); comparar contra SPICE lineal del mismo netlist. No imponer frecuencias fijas de ecualización: ambos campos `fixed_*_frequency_hz` están en `null`.

### 5.4. 7199: ganancia e inversión de fase

Implementar el pentodo y el triodo con modelos separados pero conectados eléctricamente. Incorporar componentes de `phase_inverter`, incluidas dos resistencias de 47 kΩ y capacitores de 20 nF donde indique el esquema. Usar los voltajes DC por pin (`dc_pin_volts_no_signal`) como **puntos de comparación**, no constantes rígidas de la solución.

El manual aclara que ciertas medidas fueron realizadas con voltímetro de 20 000 Ω/V. Si se simula la medición, añadir la carga del medidor a los nodos pertinentes; no comparar sin considerar esa impedancia.

### 5.5. Par push-pull 7027A

- `plate_voltage_v_nominal = 505`.
- `screen_voltage_v_nominal = 500`.
- `control_grid_voltage_v_nominal = -55`.
- `grid_stopper_ohms_each = 1000`.
- `grid_return_ohms_each = 100000`.
- `inverter_to_grid_coupling_farads_each = 5e-8` (50 nF).
- Modelar corriente de ánodo y corriente de pantalla; esta última afecta B+ y saturación.
- Modelar conducción de rejilla y carga de capacitores cuando se excede el margen de bias, para reproducir **blocking distortion**.
- **No inventar corriente de reposo:** `plate_current_idle_a_each = null`. Calibrar con curvas de la 7027A y mediciones/punto de trabajo comprobado.
- Tratar AB1 como operación nominal; permitir transición a conducción de rejilla cuando se sobreexcite, sin definir el circuito entero como «siempre AB1».

### 5.6. Fuente de alimentación con 5AR4

- `rectifier = "5AR4/GZ34"`, `bplus_idle_vdc = 505`, secundario alta tensión `800 Vac` con toma central.
- Capacitores nominales de filtrado `30/40/40/40 µF` (usar **nodos y conexiones** según esquema, no solo la lista).
- Separar la rectificación de alta tensión 5AR4 de la rectificación de bias con `10D6`.
- La primera versión debe tener DC estable en reposo. En una segunda etapa introducir rectificador, impedancia del PT, capacitores de filtro y carga dinámica.
- No emular *sag* con una compresión arbitraria de volumen si se pretende una simulación eléctrica: derivarlo de variación de B+ y demanda de corriente. Si se añade un control `Sag`, etiquetarlo como **extensión moderna**, no mando original.
- Los valores de pérdidas del PT y curva de conducción 5AR4 faltan en el JSON: cubrir con modelo de datasheet/estimación trazable y medición posterior.

### 5.7. OT-471 + NFB + impedancia de carga

- Impedancia nominal primaria: 4300 Ω placa a placa.
- Salidas secundarias: 8 Ω y 16 Ω.
- DCR primaria total: 300 Ω; DCR secundario: 0.4 Ω (8 Ω), 0.7 Ω (16 Ω).
- Relación nominal estimada desde impedancias: 23.18:1 (8 Ω) y 16.39:1 (16 Ω). **Las resistencias DC no son impedancias nominales**.
- Primera implementación: transformador ideal con resistencias serie y carga nominal.
- Segunda implementación: inductancia de magnetización y fuga, pérdidas de núcleo, histéresis/saturación, capacitancias parásitas y carga reactiva del altavoz.
- `output_transformer.negative_feedback.resistor_from_secondary_ohms_visible = 22000` solo registra una resistencia. **El nodo de retorno y la fase se obtienen exclusivamente del plano 5/67**.
- Tests de estabilidad del lazo para 8 Ω, 16 Ω, impulsos y carga reactiva; no invertir fase «a oído».

### 5.8. Altavoz de 18 pulgadas y gabinete

- `cabinet.nominal_speaker_diameter_inches = 18`, `nominal_load_ohms = 8`.
- `cabinet.speaker_impedance_curve`, `speaker_thiele_small` y `measured_ir` son **null**.
- Para el MVP: carga eléctrica nominal de 8 Ω y salida **sin gabinete** o IR cargada por el usuario, debidamente identificada.
- Para fidelidad acústica: medir **impedancia compleja** del altavoz concreto (afecta la potencia/NFB) e **IR de recinto+altavoz+micrófono** (afecta salida audible). No confundir ambos: una IR sola no reproduce la carga dinámica vista por el transformador.
- Mantener IR opcional; no distribuir grabaciones/capturas de terceros sin licencia.
- Preset `Cabinet: Off` para uso con IR loader externo. Si hay IR activada, igualar ganancia y tener en cuenta latencia.

---

## 6. Métodos numéricos y diseño de motor de tiempo real

### 6.1. Simulador offline de referencia y motor VST3

Mantener **un solo núcleo DSP** compartido por `b18n_render` y `b18n_plugin`. El renderizador offline debe poder operar con pasos más pequeños y/o modo `high_precision`, pero no desarrollar un «algoritmo distinto» que impida comparar resultados.

Implementar por etapas:

1. **Modelo lineal/equivalente:** red de entrada, RC, tone stack, inversor ideal, transformador ideal y 8 Ω. Útil para validar topología y frecuencias.
2. **Triodos no lineales:** curvas estáticas de válvula, resistencias, cátodo, acoplos con memoria.
3. **7199 y push-pull 7027A:** solucionador de corrientes, polarización, excitación diferencial.
4. **Fuente dinámica + NFB:** estados acoplados, puntos de trabajo, estabilidad.
5. **OT no ideal + altavoz reactivo + gabinete:** calibrados solo cuando exista evidencia.

### 6.2. Solucionadores

- **MNA** para redes lineales RC y topología recuperada del esquema.
- Integración temporal trapezoidal/TPT o un método estable equivalente para capacitores e inductores; tratar bien los cambios de sample rate.
- **Newton-Raphson amortiguado** para sistemas no lineales; límite explícito de iteraciones por muestra y criterio de convergencia.
- Cota de incrementos de voltaje/corriente durante iteración (sin hard-clipping de la señal audible); escalado de ecuaciones y fallback estable ante no convergencia.
- Interpolación monotónica/continua para tablas de curvas de válvulas, si se usan tablas.
- Estado de reposo resuelto **al inicializar** el motor; no empezar con 505 V instantáneos insertados en todos los condensadores sin estado congruente.
- No introducir `NaN`/`Inf`; si hay fallo, conservar el último estado válido, registrar una bandera atómica de diagnóstico y usar la política de fallback documentada.

### 6.3. Sobremuestreo y antialiasing

- Aceptar 44.1, 48, 88.2, 96 y 192 kHz del host.
- MVP: 4× (conmutable a 1× para diagnóstico). Calidad opcional: 8× si aporta ventaja medida.
- Ejecutar dentro del dominio sobremuestreado las etapas no lineales y los estados temporales pertinentes. Filtros de oversampling diseñados para suprimir aliasing; usar halfband FIR/IIR según presupuesto de fase/latencia.
- Compensar y **reportar al host** la latencia real de los filtros, especialmente al cambiar calidad.
- La convolución de gabinete puede operar a la tasa base si los límites de banda y la arquitectura lo permiten.
- No cambiar factor de sobremuestreo construyendo buffers durante el callback de audio: diferir el cambio a reconfiguración segura del motor.
- Medir THD/aliasing a distintas frecuencias y niveles de entrada; no evaluar solo con notas graves.

### 6.4. Reglas estrictas para `process()`

**Prohibido dentro del callback de audio:**

- Asignación de heap (`Vec::push` que realoca, `Box::new`, creación de FFT, `serde_json::from_str`).
- Acceso a disco, red, logging síncrono, `println!`.
- `Mutex`/`RwLock` bloqueantes, espera de otros threads o sleeps.
- Crear/finalizar hilos, cargar IR de archivo o reconstruir tablas grandes.
- `panic!`, llamadas no acotadas o solucionadores con iteraciones infinitas.

**Obligatorio:**

- Preasignar scratch buffers en `initialize()` según tamaño máximo de buffer y sobremuestreo.
- Estado propio por canal y por instancia del plugin (sin variables globales mutables).
- `reset()` restablece estados, historial de IR, filtros, smoothers y puntos de trabajo en forma consistente.
- Procesar bloques variables, incluido tamaño 1, 0 cuando el host lo permita y cambios de sample rate/buffer.
- Controles automatizados con *smoothing* (por ejemplo 5–30 ms para ganancia/tono, sujeto a pruebas), sin romper la ley de los potenciómetros.
- Denormals/subnormals: evitarlos con técnicas compatibles con las plataformas; no usar ruido perceptible para esconderlos.
- Cálculo de CPU estable y sin picos extremos de inicialización dentro de `process()`.

### 6.5. API mínima del núcleo

El siguiente bloque es una **interfaz sugerida**, no una implementación compilable completa:

```rust
pub struct B18nEngine { /* states + preallocated buffers */ }

pub struct ProcessSpec {
    pub sample_rate_hz: f64,
    pub max_block_frames: usize,
    pub oversampling: u8,  // 1, 4, 8 (modos implementados)
}

pub struct RuntimeControls {
    pub input_trim_db: f32,     // añadido moderno
    pub channel: InputChannel,
    pub volume_1: f32,          // 0..=1, posición de potenciómetro
    pub bass_1: f32,
    pub treble_1: f32,
    pub volume_2: f32,
    pub bass_2: f32,
    pub treble_2: f32,
    pub output_trim_db: f32,    // añadido moderno
    pub cabinet_enabled: bool, // añadido moderno
}

impl B18nEngine {
    pub fn new(circuit: CircuitDefinition, spec: ProcessSpec) -> Result<Self, EngineError>;
    pub fn set_controls(&mut self, controls: RuntimeControls);
    pub fn reset(&mut self);
    pub fn process_mono(&mut self, input: &[f32], output: &mut [f32]);
    pub fn latency_samples(&self) -> u32;
}
```

`CircuitDefinition` contiene valores físicos y redes eléctricas; `RuntimeControls`, posiciones de controles. Un mismo `CircuitDefinition` se comparte inmutablemente entre instancias si conviene. La API puede ampliarse a buffers intercalados/slices sin forzar `Arc<Mutex<_>>` en audio.

---

## 7. VST3 con NIH-plug

### 7.1. Requisitos del adaptador

- Nombre de trabajo: **B18N '67** o **B18N Circuit Model**; no presentarlo como producto oficial de Ampeg.
- Categoría: efecto de audio (amplificador/saturación).
- Entrada/salida: 1→1 y 2→2, con motor separado por canal en modo estéreo.
- Sin MIDI en la primera versión.
- Preservar IDs de automatización y el identificador VST3 entre versiones.
- Soportar tamaño variable de bloque, suspensión/reactivación, `reset`, guardar/restaurar estado.
- Usar parámetros NIH-plug (`FloatParam`, `EnumParam`, `BoolParam`), con `#[derive(Params)]` e IDs estables `#[id="..."]`.
- Activar `SAMPLE_ACCURATE_AUTOMATION` si las pruebas de suavizado y rendimiento lo permiten.
- Mantener el render fuera del callback del editor GUI.

### 7.2. Parámetros expuestos

| Grupo | Parámetro | Histórico | Detalle |
|---|---|---|---|
| Input | Channel / Connector | Sí | Enrutamiento Bass, Guitar, Instrument según esquema |
| Channel 1 | Volume | Sí | Potenciómetro 1 MΩ log; 0..100 % de recorrido |
| Channel 1 | Bass / Treble | Sí | 1 MΩ lineales; resolver red RC |
| Channel 2 | Volume | Sí | 1 MΩ log |
| Channel 2 | Bass / Treble | Sí | 1 MΩ lineales |
| Input | Trim (dB) | **No** | Adaptación niveles DAW↔voltios |
| Output | Trim (dB) | **No** | Normalización final |
| Quality | Oversampling | **No** | 1×/4×/8×; avisar/reconfigurar latencia |
| Cabinet | Enable / IR | **No** | Impulso medido o externo, inicialmente opcional |
| Advanced | Debug DC / B+ / THD | **No** | Solo diagnóstico, no controles de audio históricos |

Si se implementa mezcla simultánea de canales, mapear los conectores y la suma como aparecen en la red de entrada real, no sumar dos señales idealmente sin su resistencia de mezcla.

### 7.3. Workspaces y bundle

`Cargo.toml` de la raíz:

```toml
[workspace]
resolver = "2"
members = [
  "crates/b18n-dsp",
  "crates/b18n-config",
  "crates/b18n-plugin",
  "crates/b18n-render",
  "xtask",
]

[profile.release]
lto = "thin"
codegen-units = 1
panic = "abort"

[profile.profiling]
inherits = "release"
debug = true
strip = "none"
```

`crates/b18n-plugin/Cargo.toml` (plantilla; completar **commit verificado**):

```toml
[package]
name = "b18n_plugin"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "rlib"]

[features]
default = ["vst3"]
vst3 = ["nih_plug/vst3"]

[dependencies]
b18n_dsp = { path = "../b18n-dsp" }
b18n_config = { path = "../b18n-config" }
nih_plug = {
  git = "https://github.com/robbert-vdh/nih-plug.git",
  rev = "<COMMIT_SHA_VERIFICADO>",
  default-features = false
}
```

`xtask/Cargo.toml`:

```toml
[package]
name = "xtask"
version = "0.1.0"
edition = "2021"

[dependencies]
nih_plug_xtask = {
  git = "https://github.com/robbert-vdh/nih-plug.git",
  rev = "<MISMO_COMMIT_SHA_VERIFICADO>"
}
```

`xtask/src/main.rs`:

```rust
fn main() -> nih_plug_xtask::Result<()> {
    nih_plug_xtask::main()
}
```

`.cargo/config.toml`:

```toml
[alias]
xtask = "run --package xtask --"
```

`bundler.toml`:

```toml
[b18n_plugin]
name = "B18N 67"
```

La API `Vst3Plugin` requiere un `VST3_CLASS_ID` de **16 bytes estable**; crear uno propio y no copiar IDs de plugins ajenos. Declarar en `lib.rs`:

```rust
// Extracto orientativo del adaptador: faltan Plugin, Params y DSP.
use nih_plug::prelude::*;

// impl Plugin for B18nPlugin { /* NAME, I/O, initialize, reset, process, ... */ }
// impl Vst3Plugin for B18nPlugin {
//     const VST3_CLASS_ID: [u8; 16] = *b"B18N1967VST3DSP!";
//     const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
//         &[Vst3SubCategory::Fx, Vst3SubCategory::Distortion];
// }
// nih_export_vst3!(B18nPlugin);
```

Las líneas están comentadas porque la estructura `B18nPlugin` y los traits deben implementarse primero. **No presentar el fragmento como un plugin listo para compilar.** La identificación de 16 bytes propuesta es un ejemplo: una vez distribuido, mantenerla sin cambios.

### 7.4. Compilación y validación

```bash
rustup show
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --release
cargo xtask bundle b18n_plugin --release
```

Requiere reemplazar los marcadores de commit anteriores. El resultado esperado en la configuración NIH-plug habitual es un bundle en:

```text
target/bundled/B18N 67.vst3
```

En Windows, instalar en la carpeta VST3 del usuario o sistema según el host (por ejemplo `%CommonProgramFiles%\VST3` para instalación de sistema). Probar en al menos un host real y ejecutar **Steinberg VST3 Validator**, cuya CLI admite `vstvalidator [options] vst3module`.

**Importante: licencias.** El README oficial de NIH-plug advierte que sus bindings `vst3-sys` usados con `nih_export_vst3!()` se distribuyen bajo GPLv3; la publicación de VST3 con ese stack exige cumplir sus términos. Revisar licencias de código, librerías y distribución antes de decidir si el proyecto será propietario. El plugin CLAP puede facilitar prototipado, pero no elimina las obligaciones de un binario VST3 creado con esos bindings.

Referencias: [NIH-plug](https://github.com/robbert-vdh/nih-plug), [ejemplo `gain`](https://github.com/robbert-vdh/nih-plug/blob/master/plugins/examples/gain/src/lib.rs), [Steinberg Validator](https://github.com/steinbergmedia/vst3_public_sdk/blob/master/samples/vst-hosting/validator/source/validator.cpp).

---

## 8. Programa offline de pruebas (`b18n_render`)

Implementar primero `b18n_render`, antes de la interfaz de plugin.

CLI sugerida:

```bash
# Ejemplo de interfaz objetivo: requiere programar el CLI.
cargo run -p b18n_render --release -- \
  --input assets/fixtures/signals/bass_test.wav \
  --output target/renders/b18n_output.wav \
  --model assets/models/B18N_1967_simulation_parameters.json \
  --sample-rate 48000 \
  --oversampling 4 \
  --channel bass \
  --cabinet off

cargo run -p b18n_render --release -- \
  --measure dc,frequency,thd,aliasing,sag \
  --report target/reports/b18n_1967.json
```

Esos comandos describen el **comportamiento a implementar**, no afirman que exista todavía el ejecutable. El CLI debe rechazar parámetros inexistentes, JSON incompleto y referencias de IR ausentes con errores claros.

Mínimo de mediciones:

1. **Reposo DC:** voltajes de 6SL7, 7199, 7027A vs. los valores disponibles en `preamp`, `phase_inverter` y `power_stage`; comparar dentro de tolerancias documentadas una vez fijados el modelo de válvula, instrumento y carga.
2. **Respuesta en frecuencia:** barrido de 20 Hz–20 kHz a nivel bajo, para varias posiciones de Bass/Treble. Comparación SPICE cuando exista netlist verificada.
3. **Distorsión:** THD y espectros H2–H10 vs. frecuencia y amplitud; comparar ambos semiciclos del push-pull.
4. **Potencia:** comprobar 50 W nominales bajo condiciones de prueba definidas. Como referencia para resistencia de 8 Ω, 50 W equivalen a 20 V RMS en la carga; **no** usar ese valor como umbral digital de clipping sin definir escala.
5. **Transitorios graves:** trenes de pulsos y notas de 40–120 Hz para observar B+, bias y recuperación.
6. **NFB/estabilidad:** impulsos, cargas 8 Ω/16 Ω y modelos reactivos; detectar oscilaciones.
7. **Alias:** señales de alta frecuencia y sobremuestreo 1×/4×/8×; medir componentes aliasados en baseband.
8. **Cabinet:** comprobar que IR deshabilitada no altera la señal salvo etapa eléctrica ya modelada; IR habilitada debe reproducir un ensayo conocido.
9. **Determinismo:** misma entrada/configuración = misma salida dentro de tolerancia de punto flotante, incluyendo bloques de 1, 7, 64, 257 y 1024 muestras.
10. **Robustez:** silencio, DC, amplitudes elevadas, controles extremos, cambios de sample rate, proceso prolongado y detección de valores no finitos.

Guardar en reportes el `sha256` del JSON, versión de Rust, versión del algoritmo, configuración y fecha. Una referencia basada en el propio DSP no valida la fidelidad física: las comparaciones clave deberán provenir de medidas o SPICE independiente.

---

## 9. Plan de desarrollo por hitos

### Hito A — repositorio y modelo de datos

- [ ] Inicializar workspace, alias `xtask`, `cargo fmt`, clippy, CI.
- [ ] Incorporar JSON original en `assets/models` sin modificarlo.
- [ ] Implementar `HistoricalB18nJson` con Serde y validación estricta.
- [ ] Construir `CircuitDefinition` inmutable y `RuntimeControls`.
- [ ] Documentar qué arrays del JSON carecen de conectividad y digitalizar el esquema 5/67.
- **Aceptación:** `cargo test --workspace` pasa; el loader detecta nulos/revisiones incorrectas.

### Hito B — camino lineal verificable

- [ ] WAV render CLI con entrada/salida y punto de trabajo inicial.
- [ ] Entrada, ambas rutas 6SL7 en aproximación lineal, acoplos y mezcla.
- [ ] Tone stack MNA con posición continua de potenciómetros.
- [ ] Inversión ideal y transformador ideal; 8 Ω resistivos.
- [ ] Comparar curvas con modelo SPICE lineal de topología verificada.
- **Aceptación:** las 25 combinaciones de tono concuerdan con SPICE dentro de tolerancia numérica acordada; no hay glitches por tamaño de bloque.

### Hito C — no linealidad a válvulas

- [ ] Curvas de 6SL7, pentodo/triodo 7199 y 7027A desde datasheets o modelos contrastados.
- [ ] Newton amortiguado, saturación asimétrica, capacitores con memoria.
- [ ] Punto de reposo válido antes de activar audio.
- [ ] Sobremuestreo 4×; pruebas de alias y desempeño.
- **Aceptación:** señales fuertes no causan NaN, iteraciones fuera de cota ni discontinuidades no explicadas.

### Hito D — fuente, NFB y transformador

- [ ] B+ con 5AR4, filtros y carga dinámica.
- [ ] Bias negativo de la etapa 7027A y conducción de rejilla.
- [ ] NFB de 22 kΩ conectada conforme al PDF 5/67; validar polaridad.
- [ ] OT-471 ideal primero, parasitismos luego cuando haya parámetros defensables.
- [ ] Carga de altavoz impedante; gabinete IR externa opcional.
- **Aceptación:** reposo, salida bajo carga y transitorios documentados; las estimaciones están etiquetadas.

### Hito E — VST3, GUI y versión candidata

- [ ] Implementar `b18n_plugin` sobre el motor DSP probado.
- [ ] Automatización suave, presets, estados e IDs estables.
- [ ] Bloques variables, cambio de tasa, oversampling y latencia reportada.
- [ ] GUI genérica primero; visualización propia opcional después.
- [ ] VST3 Validator y al menos un DAW objetivo; pruebas de CPU y ausencia de asignaciones en audio.
- [ ] QA de licencias, empaquetado, changelog y release.
- **Aceptación:** bundle VST3 instalable, reproducible, sin fallos críticos en validación y con CPU dentro del objetivo definido para el equipo objetivo.

---

## 10. Definición de completado: fidelidad vs. utilidad

**`MVP audible`:** funciona VST3, cadena aproximada, controles y exportación WAV; faltan mediciones del gabinete, curvas magnéticas y alguna parte de calibración. Debe anunciarse como **inspirado en el circuito B-18N 1967**, no como réplica eléctricamente exacta.

**`Circuit-model validated`:** conectividad contrastada con el PDF, todas las etapas no lineales tienen modelos fundamentados y las tensiones/corrientes nominales son coherentes; red de tono y puntos de trabajo comparados contra referencia independiente.

**`Electroacoustic validated`:** además hay transformadores y altavoz concretos caracterizados, mediciones de impedancia/IR, pruebas A/B niveladas y documentación de incertidumbre. Solo aquí es defendible hablar de una emulación altamente fiel de una unidad física determinada.

No equiparar un preset musicalmente agradable con una validación del circuito. Tampoco confundir **50 W eléctricos** con volumen digital de salida: fijar un mapeo voltios↔escala digital y una ganancia de salida independiente.

---

## 11. Verificaciones pendientes antes de programar la topología final

1. Trazar conexiones exactas del bloque `Printed CKT`, potenciómetros y rutas de entrada en el esquema 5/67.
2. Verificar asignación de cada resistencia/capacitor al pin correcto de V1/V2/V3/V4/V5. El JSON agrupa valores en arrays y no es una netlist.
3. Confirmar polaridad del transformador, punto real de retorno de NFB y conexiones 8/16 Ω.
4. Diferenciar tensiones medidas en el manual de las tensiones que resultan de un circuito resuelto con modelos de válvulas elegidos.
5. Conseguir modelos contrastables para **6SL7, 7199, 7027A y 5AR4**, preservando procedencia/licencia de las curvas.
6. Medir o estimar justificadamente la resistencia efectiva del PT y las inductancias de magnetización/fuga del OT-471.
7. Elegir altavoz concreto de 1967 y obtener curva de impedancia/IR, o dejar gabinete externo como opcional.
8. Validar la implementación en un DAW y en el validador Steinberg, con el mismo procesamiento del render offline.

### Referencias externas

- [JSON local `B18N_1967_simulation_parameters.json`](../assets/models/B18N_1967_simulation_parameters.json) — fuente inmediata de datos del proyecto.
- [B-18N/B-15ND, esquema de mayo de 1967](https://vintage-blue.com/assets/b18n-b15nd_schematic_5_67.pdf) — autoridad para **conexiones** y componentes.
- [Manual de servicio B-18N](https://vintage-blue.com/assets/b18n_1967_service_manual.pdf) — medidas de reposo y especificaciones.
- [NIH-plug](https://github.com/robbert-vdh/nih-plug) — API y requisitos de licencia del plugin.
- [Ejemplo Gain de NIH-plug](https://github.com/robbert-vdh/nih-plug/tree/master/plugins/examples/gain) — integración host, parámetros, bundle.
- [Steinberg VST3 Public SDK / Validator](https://github.com/steinbergmedia/vst3_public_sdk/tree/master/samples/vst-hosting/validator) — pruebas de compatibilidad.

**Orden operativo aconsejado:** JSON validado → netlist 5/67 trazada → respuesta lineal y tono → válvulas → etapa de potencia y alimentación → transformador/altavoz → wrapper VST3 → pruebas y calibración. El wrapper VST3 nunca debe convertirse en el lugar donde se «corrige» a posteriori una simulación DSP incompleta.
