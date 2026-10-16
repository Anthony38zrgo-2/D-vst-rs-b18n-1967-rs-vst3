# Ampeg B-18N / B-15ND (5/67): resolución de bloqueos y guía para el agente Rust/VST3

> **Bloqueo nuevo B18-009 (2026-10-16):** se implementó un candidato conjunto
> con capacitancias RCA, pero la referencia continua corregida tiene margen
> HF negativo y oscila tras un pulso de 10 µV; también falla el caso Rust
> reactivo cerrado. No atribuir ya todo el problema al `z^-1`: falta validar
> la topología/caracterización HF continua. El candidato permanece aislado;
> el plugin conserva el camino anterior. Véase
> [`B18_009_STATUS.md`](B18_009_STATUS.md) para pruebas y reproducción.

**Ámbito:** auditoría de los impedimentos consignados en `sprint.md` (actualización 2026-10-09).  
**Objetivo:** desbloquear la simulación de la 7199, impedir la propagación de errores del JSON inicial y proporcionar comprobaciones reproducibles para las fases D/E.  
**Regla:** distinguir *documentado* (esquema/manual), *derivado* (ecuaciones) y *candidato no validado* (modelos de terceros o estimaciones). No se ha inspeccionado el código real del repositorio; esta revisión se basa en el `sprint.md`, el JSON de referencia y los documentos históricos citados.

## 1. Resultado prioritario: pinout 7199 y mediciones

La documentación RCA de la 7199 especifica exactamente:

| Pin | Función | B-18N: lectura DC sin señal | Interpretación |
|---:|---|---:|---|
| 1 | Placa del **triodo** | +360 V | Voltaje referido a masa, no placa-cátodo |
| 2 | Placa del **pentodo** | +105 V | Pentodo, no rejilla |
| 3 | Rejilla pantalla (G2) del **pentodo** | +60 V | Nodo de pantalla |
| 4–5 | Filamento | — | 6,3 V nominal |
| 6 | Cátodo del **pentodo**, G3 y blindaje | +1,8 V | Resistencia catódica visible: 1,5 kΩ |
| 7 | Rejilla de control (G1) del pentodo | 0 V | Referencia de entrada de señal |
| 8 | Cátodo del **triodo** | +100 V | Nodo de salida de un divisor de fase |
| 9 | Rejilla del **triodo** | +65 V | **Lectura potencialmente perturbada por el voltímetro** |

Fuentes directas:
- RCA, `7199` (1963), diagrama del zócalo (vista inferior): https://www.drtube.com/datasheets/7199-rca1963.pdf
- Ampeg, *Technical Data*, B-18-N, tabla `D.C. VOLTAGE READINGS`: https://vintage-blue.com/assets/b18n_1967_service_manual.pdf
- Esquema original fechado `567 C.W.`: https://vintage-blue.com/assets/b18n-b15nd_schematic_5_67.pdf

**No hay error de pinout** que permita atribuir el pin 2 a una rejilla: es el ánodo del pentodo. La contradicción física está en el par de valores **pin 9 = +65 V** y **pin 8 = +100 V** si se tratan erróneamente como tensiones de trabajo *simultáneas y sin carga*. La resistencia de medición indicada en el plano es **20 000 Ω por voltio del rango seleccionado**, no una impedancia constante de 20 kΩ.

### Prueba del efecto de carga del voltímetro

Para un instrumento analógico de `20_000 Ω/V`:

```text
R_meter = 20_000 * full_scale_volts
# ejemplos, no se conoce el rango empleado por Ampeg:
# 100 V -> 2.0 MΩ
# 250 V -> 5.0 MΩ
# 500 V -> 10.0 MΩ
```

En un nodo de alta impedancia representado por Thévenin:

```text
V_measured = V_open * R_meter / (R_th + R_meter)
```

**Ejemplo ilustrativo, no medición:** si la rejilla en circuito abierto estuviera cerca de 100 V, el medidor de escala 250 V (`5 MΩ`) y `R_th = 2,69 MΩ` arrojarían aproximadamente 65 V. Ese orden de magnitud es compatible con una red de resistencias de varios megaohmios. La tabla Ampeg informa `3.3 mg` (3,3 MΩ) como lectura de resistencia asociada al pin 9; **no demuestra por sí sola cuál es la resistencia de Thévenin exacta durante una medición bajo tensión**.

**Acción del agente:** calcular un reposo físico *sin medidor*; después conectar en el netlist una resistencia `R_meter` del pin observado a masa y repetir el equilibrio DC. Ensayar varios rangos plausibles, registrarlos como condiciones de medida y reportar las diferencias. No forzar el solver a reproducir `Vg9 = 65 V` sin carga; no reemplazar el 65 V documental por un valor inventado. Confirmar con la 7199 real el signo de `Vgk = V9 - V8` en el reposo sin instrumento.

**Estado (2026-10-09):** **cerrado.** `scripts/spice/b18n_7199_meter_loading.cir` (subckts Koren ajustados a las anclas del boletín RCA 1963) da, sin instrumento: pin 1 = 356,1 V, pin 8 = 102,7 V, rejilla (pin 9) = 88,3 V, `Vgk ≈ −20 V`. Con el medidor de 20 kΩ/V en el rango de 250 V (5 MΩ) la lectura del pin 9 es **62,7 V** frente a los **65 V** del manual; en 1/2/10 MΩ da 29,0/43,7/73,4 V. La red trazada es **10 MΩ de la placa del triodo a la rejilla** y **3,3 MΩ de rejilla a masa**; el carril del triodo (≈460 V, inferido de `Vp+Vk=Vrail`) es distinto del carril del pentodo (~380 V). Golden y tabla completa en `assets/fixtures/reference/phase_7199/`. Residual documentado: el ajuste del pentodo sobreestima `Ip` ~8 % en pantalla baja y el pin 2 queda ~16 % bajo el manual; no se retocan valores para forzarlo. Criterio P0.3 cumplido (corrientes plausibles, `Vgk` razonable y lectura histórica explicada por carga del instrumento).

### Comprobaciones físicas adicionales

- Pentodo: `Vpk = V2 - V6 = 103,2 V`; `Vg2k = V3 - V6 = 58,2 V`; `Vg1k = V7 - V6 = -1,8 V` **si las lecturas respectivas se toman como referencias DC sin perturbación importante**.
- Corriente catódica tentativa por la resistencia de 1,5 kΩ: `Ik ≈ 1,8 / 1.500 = 1,2 mA`. Es una **estimación a partir de la lectura histórica**, no un valor medido de corriente: comparar con `Ip + Ig2 (+ otras fugas según modelo)`.
- Triodo: `Vak = V1 - V8 = 260 V` a partir de las lecturas. El valor `Vgk = -35 V` obtenido de pin 9/pin 8 no debe usarse como condición DC sin reproducir la medición analógica.
- En la ficha RCA figuran ejemplos de operación del pentodo cerca de `Va=100 V, Vg2=50 V` y del triodo a `Va=215 V`, que sirven de anclas independientes **para el modelo de válvula**, no como puntos obligatorios del B-18N.

### Modelos externos útiles, pero NO calibración histórica automática

- *Vacuum Tube SPICE Models*, diyAudio, página 171: un usuario publicó un subcircuito parametrizado `7199_P` del pentodo, ajustado a curvas; inspeccionar la definición y licencia antes de incorporarlo: https://www.diyaudio.com/community/threads/vacuum-tube-spice-models.243950/page-171
- Página 63 del mismo hilo: ejemplo de modelo Koren `7199T_Syl` del triodo, derivado de curvas Sylvania: https://www.diyaudio.com/community/threads/vacuum-tube-spice-models.243950/page-63
- RCA AN-183 sobre aplicaciones de la 7199 como amplificador e inversor: https://www.one-electron.com/Archives/RCA/RCA-AppNotes/RCA%201959%20AN-183%20Applications%20of%20the%207199%20in%20Hi-Fi%20Audio%20Equipment.pdf

No reutilizar modelos de terceros como si representaran sin ajuste el lote de válvulas original; documentar fórmula, procedencia, rango de tensiones de ajuste, condiciones de ensayo y error de predicción. La arquitectura del ejemplo RCA AN-183 es **otra aplicación de la 7199**, no el netlist del Ampeg.

## 2. ERROR que conviene corregir antes del modelado 7199: resistencias cruzadas

Al leer el esquema Ampeg 5/67 junto con el mapeo de pines RCA, se observa:

| Campo en `B18N_1967_simulation_parameters.json` | JSON actual | Lectura del esquema 5/67 |
|---|---:|---:|
| `phase_inverter.pentode_plate_resistor_ohms` | 470000 | **270000** (al pin 2) |
| `phase_inverter.pentode_screen_feed_resistor_ohms` | 270000 | **470000** (hacia pin 3) |

Esta inversión de roles puede alterar radicalmente las corrientes y llevar al agente a compensar con parámetros de válvula incorrectos. Es **una errata en el JSON inicial** y no una variante sonora del circuito.

**Importante para la política de inmutabilidad:** `sprint.md` indica que el JSON canónico se copió con SHA fijo y que el overlay de calibración no puede reescribir componentes históricos. NO resolverlo inyectando un override secreto en `b18n_1967_calibration.json`. Alternativas lícitas:

1. Versionar una corrección deliberada del JSON canónico, con nueva huella SHA-256, changelog, evidencia y actualización de fixture/tests; o
2. Crear `assets/models/b18n_1967_historical_errata.json`, aplicado **antes** de construir `CircuitDefinition` y registrado explícitamente en la huella del modelo efectivo. No tratarlo como parámetro estimado de calibración.

Antes de confirmar la modificación en código, anotar los nodos exactos a los que se conecta el otro extremo de cada resistencia (pueden proceder de carriles B+ distintos o filtrados). El valor correcto por electrodo no basta para reconstruir el netlist de alimentación.

**Estado (2026-10-09):** aplicado por la vía 2 (`assets/models/b18n_1967_historical_errata.json`, enlazado al SHA-256 canónico, con guarda anti-deriva y nodos documentados). Trazado: placa (pin 2) por **270 kΩ** desde el carril de previo; pantalla (pin 3) por **470 kΩ** desde el mismo carril. El reporte de render registra `errata_sha256` y `effective_sha256`. Criterio P0.1 cumplido.

## 3. Resistencia de 150 kΩ — dónde buscarla

**Confirmado en fuente primaria:** el manual de servicio presenta **150 kΩ en la tabla de `RESISTANCE READINGS` para V3 / pin 3**. Pin 3 es la **pantalla G2 del pentodo 7199** según RCA.

- La resistencia está por tanto asociada a la red DC de pantalla, **no debe añadirse como resistor de la rejilla del triodo por el mero hecho de estar cerca del símbolo del inversor**.
- La medida histórica de 150 kΩ desde pin 3 hacia el negativo común respalda una rama resistiva de ese valor o una resistencia equivalente vista por el instrumento.
- **Pendiente de trazado preciso:** confirmar si la resistencia de 150 kΩ está conectada físicamente desde ese nodo a masa o si la lectura incluye una ruta a través de otros elementos. Usar el raster con las cruces/nodos del esquema, y la tabla del manual como prueba de continuidad equivalente.

**Test mínimo:** construir el netlist DC de la pantalla con los resistores nominales, calcular `R(pin3, masa)` con fuentes de DC desactivadas conforme al procedimiento de medidas resistivas (incluyendo la red de alimentación), y contrastar con el dato Ampeg de 150 kΩ. Registrar el caso del equipo encendido/apagado donde cambien rutas por el interruptor de standby. No introducir `150 kΩ` entre placa y rejilla del triodo.

**Estado (2026-10-09):** trazada en el raster: **150 kΩ de pantalla (pin 3) a masa** (bajada en x≈1223 hasta el bus y≈931), coherente con `RESISTANCE READINGS` (V3/pin 3 = 150 kΩ, equipo apagado). No es componente de la NFB. **Cerrado con SPICE:** `scripts/spice/b18n_7199_dc_1967.cir` da `R(pin3, masa)` = **150 000 Ω** con el carril flotante (equipo apagado) y 113.7 kΩ como contraste con el carril referido a masa; golden en `assets/fixtures/reference/phase_7199/`. Criterio P0.2 cumplido.

## 4. Tone stack: el agente tiene una referencia válida, pero no una validación completa del hardware

`sprint.md` registra una red pasiva James/Baxandall y pruebas MNA frente a numpy del **mismo netlist**. Eso prueba consistencia matemática entre dos implementaciones; **no prueba que la transcripción del raster sea correcta**.

Contrastar las siete patillas de la red PEC del manual (página con `PRINTED CKT. /Tone control/`): 220 kΩ, 22 kΩ, 0,001 µF, 0,01 µF, 0,00047 µF y 0,0047 µF. Hay resistencias externas de 120 kΩ/270 kΩ y controles de 1 MΩ cuyo cableado influye en las curvas.

- Distinción importante: hay **resistencias de 270 kΩ de mezcla de canales** y **otra de 270 kΩ conectada al cursor de graves**. No agruparlas por valor ni suponer la misma función. En TalkBass se aclaró expresamente que la resistencia adicional del cursor produce recorte de graves, a diferencia de las resistencias de mezcla: https://www.talkbass.com/threads/ampeg-b15-tone-control-question.1530291/
- Las cifras publicadas para **reediciones** B-15 o revisiones diferentes no son tolerancias normativas del B-18N 5/67. La discrepancia con «bass ±11 dB @40 Hz» por sí sola NO justifica mover resistencias o ajustar la EQ hasta igualar ese producto.
- Como próxima verificación, generar SPICE con exactamente los nodos de `docs/CIRCUIT_TRACEABILITY.md`. Igualar fuentes, carga, posición de controles y referencia de normalización. Comparar respuesta compleja, no solo magnitud en dB.

**Estado (2026-10-09):** **cerrado.** `scripts/spice/b18n_tone_1967.cir` y el barrido `scripts/spice/tone_stack_sweep.py` (ngspice 47) reproducen las 25×31 condiciones de Rust/numpy; golden `assets/fixtures/reference/tone_stack/tone_stack_spice.csv`, contraste complejo (magnitud y fase) con error SPICE–numpy ≤ 1e-9 dB / 0°. Sin reajustar valores por las cifras de reediciones. Criterio P1.5 cumplido.

## 5. Realimentación NFB de 22 kΩ y transformador OT-471

El esquema de mayo de 1967 contiene el retorno NFB desde el secundario; el sprint dice haber trazado la toma **16 Ω** y localizado nodo y polaridad, quedando pendiente el netlist fino. Hay que preservar ese rastro y no sustituirlo por un lazo de ganancia arbitraria.

Valores documentados del OT-471 (manual): primario 4,3 kΩ placa a placa, derivaciones 8/16 Ω; DC primario 300 Ω, DC secundario 8 Ω = 0,4 Ω, DC secundario 16 Ω = 0,7 Ω. Relaciones ideales de tensión:

```text
n_8  = sqrt(4300/8)  ≈ 23.18 : 1
n_16 = sqrt(4300/16) ≈ 16.39 : 1
V_16 / V_8 = sqrt(16/8) = sqrt(2)  # para taps ideales, mismo primario
```

Si la realimentación toma el terminal de 16 Ω mientras la carga está en el de 8 Ω, el voltaje de feedback no es igual al del altavoz: el modelo debe representar ambos taps. Verificar el sentido del bobinado para que el retorno sea **negativo**.

**Advertencia numérica:** la realimentación global a través de 7027A + OT + red de retorno forma un problema acoplado no lineal. Introducir un `z^-1` de un sample solo para esquivar el solver cambia su respuesta de fase y puede generar inestabilidad. Priorizar un sistema implícito/monolítico reducido; si la implementación usa una separación explícita, etiquetarla como aproximación y medir márgenes de estabilidad a 44,1–192 kHz.

Para el OT real siguen faltando inductancia de magnetización, inductancia de dispersión, capacitancia interbobinado, curva de saturación y sus pérdidas. No asignar cifras ficticias para «sacar sonido» como si fueran documentales; es aceptable conservar el OT ideal provisional y dejar un módulo de caracterización con datos de procedencia.

> **Estado (B18-013, 2026-10-15):** implementado el OT lineal con parámetros
> **estimados y etiquetados** (overlay `output_transformer.*`: Lm = 28,7 H —
> esquina LF 25 Hz con la carga reflejada —, Lleak = 12 mH, Cp = 100 pF,
> R_core = 500 Ω y cota provisional de flujo 0,12 A) en la red diferencial del
> Newton de placas, con el secundario acoplado al EMF del flujo (bloqueo de
> continua físico; el paso alto de 1 Hz queda solo en el modo ideal de
> regresión). Contraste AC contra ngspice + analítica independiente
> (`assets/fixtures/reference/output_transformer/`): ≤0,091 dB / 0,97° en
> 20 Hz–20 kHz a la tasa 4×; polaridad y tomas verificadas. **Pendiente:** la
> curva de saturación/histéresis (B18-014, solo con datos) y la carga reactiva
> del altavoz (B18-015), prerrequisito de B18-009.

> **Estado (B18-015, 2026-10-15):** la **física de la carga reactiva está
> cerrada**: `ElectricalLoad` (Resistive fallback + equivalente RLC estimado
> `Re+Le+Res∥Les∥Ces`, TPT) dentro de la rama reflejada del solve del OT, con
> corriente de secundario y contraste SPICE
> (`assets/fixtures/reference/speaker_load/`, < 0,5 dB / < 10° en banda
> audible). **El lazo con NFB provisional × carga reactiva queda bloqueado:**
> medido que el lazo (z^-1 + polo 2,5 kHz) pierde todo el margen (ciclo
> límite ~24 kHz, ~2–5× sobre el límite de ganancia; ninguna compensación
> provisional tuneada lo hizo robusta). Adoptado: la selección del equivalente
> estimado **cae a la carga nominal mientras la NFB esté activa** (fallo
> explícito/documentado, test fijado); el modo reactivo corre con la NFB
> **abierta** (`present = false`). **B18-009 queda desbloqueado** y debe
> reabrir la combinación con la red física (capacitancias de válvulas y OT);
> el archivo de mediciones está en `docs/sprint.md` §21 y
> `docs/MODEL_ASSUMPTIONS.md`.

## 6. Etapa 7027A: uso provisional del modelo 6L6GC

`sprint.md` indica que se inicializó con curvas publicadas de una 6L6GC y un punto `360/250/-18 V`, en contraste con las tensiones B-18N de `505/500/-55 V`. Las 7027A están relacionadas con la familia 6L6GC, pero **esa única comprobación del modelo está muy lejos del régimen real**; no demuestra fidelidad de pantalla, corte y transferencia a 500 V.

Acciones:

- Mantener una etiqueta explícita `tube_proxy=6L6GC`, `validated_at_7027a_operating_region=false`.
- Obtener curvas originales RCA/Sylvania 7027A, especialmente frente a tensiones de pantalla grandes. Ajustar **dos corrientes**, `Ia` e `Ig2`, y el corte, no solo `Ia`.
- En cada render de calibración registrar: `Ia_idle`, `Ig2_idle`, `Pa_idle=(Va-Vk)*Ia`, `Pg2_idle=(Vg2-Vk)*Ig2`, y balance entre ramas. No convertir la disipación máxima de placa de la ficha en un objetivo de bias ni fijar una corriente de reposo sin evidencia.
- Aumentar drive desde voltajes pequeños hasta el umbral de conducción de G1; comprobar continuidad de corrientes, estabilidad de Newton, recuperación del acoplamiento y *blocking distortion*. Mantener el límite de potencia térmica como restricción de plausibilidad, no como sustituto de curvas.

**Estado (2026-10-14, B18-010/011):** **cerrado con datos RCA.** Datasheet RCA
7027-A (8-59) descargado en `assets/reference/7027A_RCA.pdf` (fuera de git); sus
páginas 1–3 (tablas) y figuras p05/p07 digitalizadas por
`scripts/digitize_7027a_curves.py` en `assets/fixtures/reference/power_7027a/`
(manifiesto con SHA-256) y ajustadas con `scripts/fit_7027a_model.py`. La 7027A
ya no es proxy: `mu`=9,0 (cota documentada por la conducción en el reposo
B-18N), `ex`=1,298, `kg1`=1088, `kp`=42,4, `kvb`=38,4 V (errores: familia p05
mediana 2,9 %, transferencia p07 mediana 9 %; clase A1 Ip +15 %). La corriente
de pantalla usa la **división calibrada** `Ig2 = Ip·(F + G·e^(−Vp/V0))`
(B18-011; `F`=0,016, `G`=0,142, `V0`=255 V) con dependencia de placa medida en
el datasheet. Reposo del modelo: ≈44 mA/tubo, Pd ≈22 W. Test:
`crates/b18n-dsp/tests/power_7027a_curves.rs`. Netlist SPICE de la etapa de
potencia (`b18n_power_7027a_1967.cir`) diferido a B18-012/013: sin fuente ni OT
reales el contraste AC/tran sería contra la propia simplificación; las curvas
del datasheet son la referencia independiente. La etiqueta del proxy 6L6GC solo
queda como respaldo legacy de `kg2`.

## 7. Hito D: fuente dinámica sin inventar un transformer model

**Base documental:** rectificador 5AR4/GZ34, B+ 505 V sin señal, bias aproximado −55 V, filtros mostrados como 40/40/40/30 µF, tensiones y conexiones de fuente según el plano, transformador de potencia PT-109 o PT-12-XA.

- Empezar por un **modelo de envolvente de B+** con capacitores y resistencias realmente trazados, cargas dinámicas por las corrientes de las etapas, y parámetros de resistencia serie/recuperación de rectificador etiquetados `estimated` cuando la ficha no alcance para fijarlos.
- Después implementar el rectificador según curva de conducción de la 5AR4 y la impedancia de la alimentación. Un simulador 50/60 Hz con todos los cruces de diodos a 4× puede ser costoso en un VST3; desacoplar la escala lenta del ripple/envolvente, y validar frente al SPICE offline antes de optimizar.
- No confundir el **diodo 10D6 de bias** con la rectificadora principal 5AR4. No asumir que el bias negativo sigue automáticamente la misma caída dinámica que B+ si los secundarios/circuitos difieren.
- Los capacitores de 0,05 µF del acoplamiento PI→7027A y los retornos de 100 kΩ producen memoria de bloqueo: pasar de un offset DC superpuesto a un estado de carga dinámico con `Vc(t)`.

> **Estado (B18-012, 2026-10-15):** implementado el **modelo de envolvente**
> (primer punto): red H2→1k→H1→10k/22k→H3/H4 + bias con `Rrect = 150 Ω` y
> `Rb = 15 kΩ` (`estimated`), cargas dinámicas de todas las etapas y contraste
> SPICE (`power_supply/`). Pendiente el segundo punto (curva de conducción de
> la 5AR4 e impedancia del PT por frecuencia) y la toma exacta del devanado de
> bias.

## 8. SPICE independiente: instalarlo en Windows y contrastar topología

Ngspice tiene binarios oficiales Windows x64. La página de descarga indica **ngspice-47** como estable y distribuye un archivo `.7z` para Windows:

- https://ngspice.sourceforge.io/download.html
- https://sourceforge.net/projects/ngspice/files/

Comprobar tras descomprimir `ngspice -v` desde el directorio `bin` apropiado; añadir a PATH solamente si es necesario. No convertir `scripts/tone_stack_reference.py` en un alias que llame al solver Rust: las referencias deben ser independientes. No ejecutar netlists de origen no revisado.

Propuesta de archivos versionados:

```text
scripts/spice/b18n_tone_1967.cir
scripts/spice/b18n_7199_dc_1967.cir
scripts/spice/b18n_7199_meter_loading.cir
scripts/spice/b18n_power_7027a_1967.cir
scripts/spice/README.md
assets/models/b18n_1967_historical_errata.json  # solo si se adopta política errata
crates/b18n-dsp/tests/phase_7199_reference.rs
crates/b18n-dsp/tests/nfb_stability.rs
crates/b18n-dsp/tests/power_supply_transient.rs
```

Pruebas recomendadas, en este orden:

1. `.op`: reposo físico 7199 sin instrumento; KCL por electrodo.
2. `.op`: mismo circuito con resistor de voltímetro de `2 MΩ`, `5 MΩ` y `10 MΩ` al pin 9; registrar `V8`, `V9`, `Vgk` e impedancia de Thevenin.
3. `.ac`: amplitud y fase en ambos extremos del inversor, con salida sin carga y con las dos cargas equivalentes a las redes de rejilla 7027A.
4. `.tran`: escalón y barrido del inversor; verificar amplitudes equilibradas, polaridades opuestas y limitación suave/no convergente.
5. `.ac/.tran`: red de tono contra Rust/numpy bajo **impedancias y controles idénticos**.
6. `.tran`: NFB desconectada y conectada con fuente/OT ideal; revisar signo, respuesta de escalón y ausencia de oscilación antes de introducir saturación.

En los informes guardar archivo de circuito, hash de configuración, simulador/versión, datos de sweep y tolerancias aplicadas.

## 9. Hito E puede avanzar en paralelo (sin esperar a caracterizar el OT)

> **Estado (Ola 7):** ejecutado. `crates/b18n-plugin` existe con NIH-plug
> fijado, IDs de parámetros con `#[id]` de variante (no índice), layouts 1→1 y
> 2→2 con motores independientes, sin asignaciones en `process`/`reset`
> (test no-alloc), latencia **efectiva** verificada con impulso (**47 muestras**
> a 4×, no 93: el cálculo anterior duplicaba el retardo por etapa), bundle
> generado. Quedan los pasos manuales de Steinberg Validator y DAW.

El sprint informaba de **94 tests**, DSP funcional y `b18n-render` con latencia de **45 muestras**, pero aún no existía `b18n-plugin`. Esa era la vía de progreso independiente de la incertidumbre histórico-electrónica:

- Añadir `crates/b18n-plugin` con NIH-plug fijado al commit conocido del proyecto.
- Parámetros con IDs persistentes (no usar índice de enum como identificador permanente): input gain, canal, volume, bass, treble, output gain y bypass; no mostrar ajustes internos como si fueran controles originales de 1967 salvo en un panel avanzado explícitamente moderno.
- Adaptador mono→mono, opcional mono→estéreo por duplicación controlada; comprobar qué buses declara el VST3 y cómo el host negocia canales. No implementar falsamente dos canales del amplificador como dos salidas estéreo independientes.
- Inicializar/cambiar tamaño de bloque fuera de `process`; evitar asignaciones, locks, filesystem, panic/unwrap y logging en el callback. Filtrar denormales; garantizar ausencia de NaN/Inf.
- Reportar la latencia **efectiva** del procesamiento 4× (45 muestras según sprint) y verificarla con impulso en modo plugin; hacer compensación de bypass/dry si se ofrece mezcla sin desfase.
- Soportar estado de presets/autómatas e inicialización determinista a 44,1/48/88,2/96/192 kHz; comparar WAV del plugin contra `b18n-render` con mismos parámetros y mismo estado inicial.
- Empaquetar con el `xtask` NIH-plug y validar usando el validator oficial de Steinberg: https://steinbergmedia.github.io/vst3_dev_portal/pages/What%2Bis%2Bthe%2BVST%2B3%2BSDK/Validator.html
- **Licencias:** NIH-plug en mantenimiento; en la rama principal descrita públicamente los bindings VST3 son GPLv3, y el proyecto ya declara `GPL-3.0-only`. No inferir de la relicencia MIT del SDK Steinberg que la dependencia usada por NIH-plug se haya vuelto MIT: https://github.com/robbert-vdh/nih-plug

## 10. Orden de prioridades y definición de «desbloqueado»

| Prioridad | Acción | Criterio de cierre |
|---|---|---|
| P0 | Fijar inversión 270k/470k del JSON con trazabilidad | Consistencia de referencias pin2/pin3, hash/model-version controlados |
| P0 | Trazar 150k de pin3 con fuente y tierra correctas | Netlist verificable y resistencia pin3 a masa compatible con manual |
| P0 | Resolver reposo de la 7199 y medidor | Corrientes físicamente plausibles; `Vgk` sin carga razonable; lectura histórica explicada por carga dentro de incertidumbre |
| P1 | Sustituir inversor ideal por modelo 7199 validable | Comparaciones SPICE `.op`, `.ac`, `.tran`; pruebas de inversión y recorte |
| P1 | Instalar ngspice 47, contrastar tone stack | Mismo netlist y condiciones; diferencias de métodos dentro de tolerancia |
| P1 | Implementar NFB y memoria de condensadores | Lazo negativo estable en rangos de sample rate/drive |
| P1 (paralelo) | Crear plugin VST3 | bundle, presets, latencia, validator y host DAW en limpio |
| P2 | Rectificador, transformer no ideal, cab reactivo/IR | Datos etiquetados por procedencia; mejoras corroboradas por medición |

**Estado de ejecución (2026-10-11):** cerrados P0.1 (errata 270 kΩ/470 kΩ por archivo de errata), P0.2 (150 kΩ con SPICE), P0.3 (reposo 7199 y carga del medidor; véase el estado en la sección 5), P1.4 (inversor 7199 real con `.op` idéntico al golden, `.ac`/`.tran` dentro de tolerancias), P1.5 (tone stack contra ngspice 47), **P1.6** (NFB al cátodo del pentodo con separación explícita `z^-1` + compensación declarada — **2,5 kHz de la tasa base desde la Ola 8**; estabilidad medida a 44,1–192 kHz, cargas 8/16 Ω y drives hasta 4 V; blocking recuperable) y **P1.7** (plugin VST3 en `crates/b18n-plugin`, bundle `target/bundled/B18N 67.vst3`; quedan como pasos manuales el Steinberg Validator y la prueba en DAW). La verificación con impulso de P1.7 corrigió la latencia del sobremuestreo 4× de 93 a **47 muestras** (cada etapa se contaba al doble de su retardo real) y eliminó las asignaciones dinámicas del solver Newton y de `reset()`.

**Ampliación 2026-10-15 (B18-012/013):** la fuente dinámica (Hito D, B18-012) y el **OT-471 lineal con Lm/fuga/Cp estimados** (B18-013, sección 5) están implementados con contraste SPICE independiente (dos goldens nuevos en `assets/fixtures/reference/`); el modelo usa Euler atrás declarado en los inductores del OT y el test de estrés de alias queda en 0 fallos de solver. **Pendiente P2:** saturación del núcleo (B18-014, solo con datos), carga reactiva del altavoz (B18-015) y, tras ellas, el cierre de la NFB sobre red real (B18-009).

La **Ola 8 (Fase 1, CPU)** cerró el presupuesto del plugin: inversor con Newton 3D acoplado y jacobiano analítico (~2-6 evaluaciones Koren/muestra frente a ~5.500 de la bisección anidada), robustez a drive extremo con 0 fallos de congelación a 4 V, guardia de CPU en **23,6 µs/muestra** (bench estéreo 4×, límite declarado 25 µs), niveles de DAW verificados (`daw_levels.rs`) y nivel de instancia por defecto dentro de la escala del host (trim de salida −16,5 dB no histórico). **140 tests en verde**. Mejoras documentadas pendientes: fuente dinámica (Hito D, hecha en B18-012), OT no ideal (hecho en B18-013; falta la saturación B18-014), interpolador de mayor atenuación y Fase 2 de CPU (opcional).

**No hacer:** corregir la supuesta «7199 inconsistente» con voltajes DC inventados; retocar el tone stack para que coincida con reediciones ajenas; pasar valores de ajuste no documentados al JSON histórico; calibrar la curva de saturación del OT a oído como si fuera original; bloquear la entrega del primer VST3 por no disponer aún de un gabinete medido.

## Fuentes primarias y secundarias

1. Ampeg B-18N/B-15ND, esquema `5/67`: https://vintage-blue.com/assets/b18n-b15nd_schematic_5_67.pdf
2. Ampeg, *Technical Data*, cuarta edición (2.ª impresión 1968), secciones B-18N: https://vintage-blue.com/assets/b18n_1967_service_manual.pdf
3. RCA, datasheet 7199, 1963: https://www.drtube.com/datasheets/7199-rca1963.pdf
4. RCA, AN-183, aplicaciones de 7199: https://www.one-electron.com/Archives/RCA/RCA-AppNotes/RCA%201959%20AN-183%20Applications%20of%20the%207199%20in%20Hi-Fi%20Audio%20Equipment.pdf
5. Unofficial Ampeg, índice con redibujado B-18N/B-15ND: https://unofficialampeg.com/schematics
6. Publicaciones de modelos de 7199 de terceros (NO fuente histórica Ampeg): https://www.diyaudio.com/community/threads/vacuum-tube-spice-models.243950/page-171
7. Dudas acerca de la resistencia extra de 270 kΩ en el tone stack: https://www.talkbass.com/threads/ampeg-b15-tone-control-question.1530291/
8. Distribución oficial ngspice: https://ngspice.sourceforge.io/download.html
9. NIH-plug, estado/licencia: https://github.com/robbert-vdh/nih-plug
10. Validator VST3 oficial Steinberg: https://steinbergmedia.github.io/vst3_dev_portal/pages/What%2Bis%2Bthe%2BVST%2B3%2BSDK/Validator.html
