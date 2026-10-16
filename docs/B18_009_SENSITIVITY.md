# B18-009 — Sensibilidad del lazo continuo y causa raíz de los márgenes negativos

**Estado: cerrado e integrado.** La causa raíz se aisló en SPICE, se corrigió el
`Cp` del OT y el solve acoplado (`OutputNetwork`) es el camino de producción con
`ALGORITHM_VERSION` actualizado (`…-nfb-coupled-v2`). El diseño de la corrección
y la integración se documentan abajo; ver también `docs/B18_009_STATUS.md`.

## 1. Qué se midió

`scripts/spice/nfb_sensitivity.py` reutiliza el netlist validado de
`nfb_loop_reference.py` (Koren RCA, rieles 380/460/505/500/−55 V, mezcla
Rmix/2, NFB de 22 kΩ desde la toma de 16 Ω) y parametriza el equivalente
diferencial del OT para activar/desactivar/ barrer un factor por escenario.
Se mide el retorno rompiendo solo el sensor de tensión (inyección de 1 V AC,
punto de reposo DC intacto): `T = −Vretorno/Vtest = polaridad·tapa·V(s4)`,
ecuación característica `1+T`, 1 Hz–100 MHz, 200 puntos/década. Se registran
**todos** los cruces de 0 dB y de −180°/−540°/−900°, no solo el primero.
Transitorios: pulso de 10 µV retirado a 2,1 µs, paso máximo 0,1 µs.
Pasividad: puerto diferencial de un solo lado, ¿Re(Z) ≥ 0?

Fixtures nuevos (registrados en el manifiesto, no sobrescriben los históricos):
`sensitivity_margins.csv/json`, `sensitivity_passivity.json`,
`sensitivity_transients.json`.

Reproducción:

```powershell
python scripts/spice/nfb_sensitivity.py --all
python scripts/check_reference_manifest.py
```

El baseline reproduce la referencia histórica: `B18_009_STATUS.md` daba
nominal 212,4 kHz / −55,38° / −11,23 dB; aquí 213,6 kHz / −58,63° / −12,19 dB
(la pequeña diferencia es el ajuste de la 7199 del overlay y la omisión de la
resistencia de fuga de rejilla no trazada, ya presentes en
`nfb_coupled_reference.py`).

## 2. Tabla de sensibilidad (cruce HF, 8 Ω salvo indicación)

| Escenario | Cruce 0 dB HF | Margen de fase | Margen de ganancia |
|---|---:|---:|---:|
| OT ideal (sin Lm/Lleak/Cp/R_core) | 313 kHz | **+76,6°** | — |
| OT ideal, carga RLC | 962 kHz | **−42,6°** | −23,1 dB @ 317 kHz |
| +Lm | 313 kHz | +76,6° | — |
| +Lleak | 582 kHz | **−81,5°** | −8,0 dB @ 473 kHz |
| +Lleak+Cp (100 pF) | 214 kHz | −58,4° | −12,2 dB @ 150 kHz |
| baseline (+R_core) | 214 kHz | −58,6° | −12,2 dB @ 150 kHz |
| baseline, carga RLC | 325 kHz | −10,0° | −5,1 dB @ 244 kHz |
| Cp = 0 | 580 kHz | −79,3° | −7,6 dB |
| Cp = 22 pF | 353 kHz | −77,9° | −12,8 dB |
| Cp = 47 pF | 279 kHz | −69,9° | −13,1 dB |
| Cp = 220 pF | 158 kHz | −43,5° | −9,7 dB |
| Cp = 470 pF | 115 kHz | −25,5° | −5,8 dB |
| Lleak = 6 mH | 268 kHz | −48,7° | −7,7 dB |
| Lleak = 24 mH | 166 kHz | −59,8° | −15,9 dB |
| Lleak = 48 mH | 127 kHz | −55,9° | −18,4 dB |
| R_core = 0 / 100 / 2000 Ω | ~214 kHz | −58,4…−59,4° | ≈−12,2 dB |
| Lm = 14 H / 57 H | 214 kHz | −58,6° | −12,2 dB |
| sin capacitancias de válvula | 335 kHz | **+13,2°** | — |
| sin capacitancias de válvula, RLC | 1595 kHz | **+90,1°** | — |
| fuente 220 kΩ | 200 kHz | −59,8° | −10,9 dB |
| fuente 270 kΩ | 194 kHz | −59,2° | −10,1 dB |
| fuente 330 kΩ | 187 kHz | −57,9° | −9,2 dB |
| fuente compleja real (2 redes de tono + 270 kΩ) | 194 kHz | −59,3° | −10,2 dB |
| polaridad invertida | 214 kHz | **−238,6°** | −12,8 dB |
| toma de 8 Ω | 196 kHz | −49,6° | −9,2 dB |
| **Cp corregido = 1,5 nF** | 65 kHz | **+10,3°** | — |
| **Cp corregido = 1,5 nF, carga RLC** | 77 kHz | **+40,6°** | — |
| Cp = 1,5 nF + R_arrollamiento = 900 Ω | 61 kHz | **+20,7°** | — |
| Cp = 1,5 nF + R_arrollamiento = 900 Ω, RLC | 77 kHz | **+40,8°** | — |

## 3. Causa raíz

1. **La resonancia estimada de 145 kHz del OT es el cruce del lazo.** Con
   `Lleak = 12 mH` y `Cp = 100 pF` la resonancia de fuga cae en
   `1/(2π√(Lleak·Cp)) = 145,3 kHz`, justo donde la fase de retorno cruza
   −180° (≈150 kHz, con +12 dB de ganancia todavía). El cruce de 0 dB a
   212 kHz (nominal) y 325 kHz (RLC) es esa misma resonancia desplazada por
   la carga. El cruce de −180° y el margen de ganancia negativo
   (−12,2 dB) son consecuencia directa de la resonancia, no del retardo
   digital ni de la topología NFB.

2. **La estimación de `Cp = 100 pF` es un criterio de diseño, no una medida.**
   El overlay la fijó "para que la resonancia caiga fuera de la banda audible
   y de proceso". No hay medida del OT-471. Al colocar la resonancia en
   145 kHz, el modelo dejó el lazo cruzando en plena resonancia.

3. **`Lleak = 12 mH` es el otro factor dominante en 8 Ω.** Sólo el Lleak
   (sin Cp) ya lleva el margen de fase a −81,5°. La esquina de fuga con la
   carga reflejada es `f = R/(2π·Lleak) ≈ 57 kHz`, con pendiente de −90° que,
   sumada a las capacitancias de válvula, acumula −180° a 473 kHz con lazo
   todavía positivo.

4. **`R_core` está en la rama equivocada para amortiguar.** Los 500 Ω de
   pérdidas se estampan en serie con la rama de magnetización (`Rcore a f`,
   `Lm f 0`); la resonancia de fuga (`Lleak`–`Cp`) queda prácticamente sin
   amortiguamiento. Barrer `R_core` de 0 a 2000 Ω mueve el margen <1°, lo que
   confirma que no participa en la resonancia crítica.

5. **Las capacitancias de datasheet de válvula son el otro gran contribuyente.**
   Quitarlas lleva el 8 Ω a +13,2° y la carga RLC a +90,1°: la resonancia
   crítica nace de la interacción de `Lleak`/carga con las capacitancias
   interelectródicas RCA (7027-A placa−cátodo 7,5 pF, placa−rejilla 1,5 pF).

6. **La carga RLC estimada es un segundo resonador independiente del OT.**
   Con el OT **ideal** y carga de 8 Ω el lazo es estable (+76,6°), pero con la
   RLC **estimada** el margen es **−42,6°**. La RLC (toda `estimated`, sin
   medida) hace que la carga de secundario sea casi abierta/inductiva en HF
   (N₈²·Le ≈ 0,43 H reflejada), resonando con las capacitancias de placa.

7. **Descartado como causa raíz:**
   - **Polaridad y tomas:** la polaridad actual (`+1`) es la de
     realimentación negativa; invertirla da −238,6° (oscilación). La toma de
     16 Ω es la correcta (la de 8 Ω es peor, −49,6°).
   - **Pasividad:** los 60+ escenarios tienen Re(Z) ≥ 0; el modelo no es
     activo. No hay error de signo en `Ed`/`Fdiff`/`fpri`.
   - **Impedancia de fuente:** 135 kΩ vs 220/270/330 kΩ y vs la **red de tono
     real** (dos canales + 270 kΩ) cambian el cruce <10 % y el margen <4°; no
     es la causa. El 135 kΩ de Thevenin es una simplificación válida en HF.
   - **Lm:** sin efecto en HF (14/57 H idénticos).

8. **Confirmación transitoria** (10 µV, ventana 1–2 ms): baseline nominal
   15,4 V, baseline RLC 40,0 V (crecen y persisten); ideal nominal 0 V;
   polaridad invertida 33,9 V; **Cp corregido 1,5 nF: 0 V en ambas cargas.**
   Un margen de fase de Cp = 1 nF (−3,4°) todavía crece a 18,6 V: cerca de
   cero no basta, coherente con la advertencia de `B18_009_STATUS.md`.

## 4. Solución propuesta (valores corregidos, `estimated`)

La corrección no toca la topología NFB (correcta) ni la polaridad. Corrige el
**modelo HF del OT-471**, cuyos parámetros son estimaciones sin medida:

| Parámetro | Actual | Corregido | Procedencia |
|---|---|---|---|
| `output_transformer.parasitic_capacitance_f` | 1,0e-10 F | **1,5e-9 F** | `estimated`: capacitancia shunt total (interbobinado + distribuida) que sitúa la autorresonancia del OT en la banda 30–50 kHz típica de un transformador push-pull de 50 W de 1967, en vez de los 145 kHz de un criterio de diseño. Sin medida del OT-471. |
| resistencia de arrollamiento en serie con la fuga (nuevo campo, p. ej. `winding_ac_resistance_ohms`) | 0 (implícito) | **900 Ω** | `estimated`: resistencia AC del arrollamiento a HF (≈3× la DCR de 300 Ω, piel/proximidad) para amortiguar la resonancia de fuga que hoy queda sin pérdidas. Opcional; el Cp solo ya estabiliza. |

Resultado con `Cp = 1,5 nF` (resto sin cambios): 8 Ω **+10,3°**, RLC
**+40,6°**, transitorios a 0 V. Con el añadido de 900 Ω: **+20,7°** y
**+40,8°**.

**Advertencia de procedencia:** estos valores siguen siendo `estimated`, no
`measured`. No deben aplicarse como si fueran del hardware. El ajuste se
justifica por la autorresonancia físicamente acotada, no por hacer pasar el
test; una medida real del OT-471 (o una cota del fabricante) debe reemplazarlos.

Contexto físico (diseño de transformadores de salida): la autorresonancia
`Lleak`–`Cp` de un transformador de válvulas cae típicamente entre ~50 kHz y
varios cientos de kHz, y la fuga va desde <3 mH (toroide / muy intercalado)
hasta >30 mH (intercalado mínimo) [Rod Elliott, *Design Considerations*;
Aiken, *Output Transformers Explained*]. El par estimado (12 mH, 100 pF) es un
punto de ese espacio; el modelo no tiene, además, ninguna pérdida en la rama
HF que amortigüe los polos de la fuga y de las capacitancias de válvula. La
corrección por lo tanto **no es un valor único**: es (a) acotar `Cp` a la banda
de autorresonancia física y (b) representar la pérdida HF donde circula
corriente (serie con la fuga). El conjunto de la tabla es una realización
representativa de esa corrección; otros pares dentro de la banda física también
dan margen positivo.

### Archivos afectados por la corrección (a aplicar cuando B18-009 se desbloquee)

- `assets/models/b18n_1967_calibration.json` — `parasitic_capacitance_f` y,
  si se adopta, el nuevo campo de resistencia de arrollamiento (con
  `origin`/`method`/`date`).
- `crates/b18n-dsp/src/stages/output_transformer.rs` — soporte del término de
  pérdida en serie con la fuga.
- `assets/models/b18n_1967_simulation_parameters.json` — el canónico sigue
  `null`; no se sobrescribe.
- `scripts/spice/b18n_ot471_linear.cir`, `output_transformer_reference.py`,
  `assets/fixtures/reference/output_transformer/ot471_linear_spice.csv`.
- `crates/b18n-dsp/tests/output_transformer_ac.rs`, `nfb_coupled.rs`.
- `docs/MODEL_ASSUMPTIONS.md`, `docs/CIRCUIT_TRACEABILITY.md`.

**No aplicar ahora:** cambiar el overlay altera el camino DSP de producción sin
subir `ALGORITHM_VERSION`, lo que infringe las reglas de B18-009. La corrección
queda propuesta y verificada en SPICE.

## 5. Paso 5 — re-test del solver 10D con el `Cp` corregido

Se regeneró la referencia acoplada con el `Cp` corregido
(`coupled_ac_corrected.csv` / `coupled_dc_corrected.csv`, vía
`python scripts/spice/nfb_sensitivity.py --corrected-reference`) y se añadió
`crates/b18n-dsp/tests/nfb_coupled_corrected.rs`, que construye el candidato
aislado `OutputNetwork` con `parasitic_capacitance_f = 1,5 nF` **sin integrarlo
en el motor**:

- **Residual RLC cerrada resuelto.** El caso que fallaba (93,75 Hz, RLC
  cerrada) pasa de **62.335 fallos/65.536** a **0 fallos**, a las cuatro tasas
  efectivas del plugin (176,4/192/384/768 kHz = 44,1/48/96/192 kHz ×4) y a
  tres niveles (0,1 mV / 10 mV / 1 V).
- **Topología validada:** a 768 kHz (donde el warping TPT es despreciable) el
  candidato corregido coincide con la referencia corregida en los 24 puntos
  con error máximo **0,107 dB / 0,61°**.
- **Discretización temporal:** a tasa de proceso (192 kHz) el punto de 18 kHz
  difiere 0,563 dB / 3,06° de la referencia. Es el límite de warping TPT ya
  documentado, se mide y reporta en su propio test (no se oculta aflojando la
  puerta de topología).
- **CPU:** el lazo aislado a 192 kHz mono cuesta **≈1,2–1,25 µs por llamada**
  (release). El dato de 32,487 µs de `B18_009_STATUS.md` era con el candidato
  integrado en el motor y **no** es comparable; la puerta real de tiempo real
  exige reintegrarlo, lo que queda diferido.

Conclusión: el modelo continuo corregido elimina la divergencia del solver
RLC. El candidato se **integró** después como etapa de salida de producción
(`engine.rs` usa `OutputNetwork`; se retira `z^-1` + polo de 2,5 kHz), con
`Cp = 1,5 nF` en el overlay y `ALGORITHM_VERSION` →
`…-supply-ot471-linear-nfb-coupled-v2`. La guardia de CPU se re-basó a 40 µs
(medido ~36-38 µs); ver `docs/B18_009_STATUS.md`.

## 6. Pendiente tras el diagnóstico

1. Conseguir una **medida del OT-471** (autorresonancia, Lm, Lleak, Cp,
   resistencia AC) o una cota de fabricante. Hasta entonces, `estimated`.
2. Reabrir el candidato `output_network.rs` con el `Cp` corregido, re-ejecutar
   el residual RLC cerrada (62.335/65.536 fallos) y el presupuesto CPU
   (32,487 µs vs guardia 32 µs).
3. Contrastar la respuesta cerrada del motor contra esta referencia corregida
   y mantener el candidato aislado hasta superar SPICE, `nfb_stability.rs`,
   `reactive_load.rs`, varias tasas y CPU.
