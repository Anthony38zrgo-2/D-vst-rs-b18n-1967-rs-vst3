# B18-009 — NFB física sobre red acoplada (2026-10-16)

**Estado: integrado en el motor y `ALGORITHM_VERSION` actualizado.** La causa
raíz de los márgenes negativos se aisló en SPICE (resonancia estimada del OT +
amortiguamiento HF ausente + carga RLC estimada, no la topología NFB), se
corrigió el `Cp` del overlay y la etapa de salida física acoplada
(`OutputNetwork`) sustituye al inversor + etapa de potencia + lazo NFB
artificial (`z^-1` + polo de 2,5 kHz). Batería completa debug/release en verde y
render E2E re-basado. Ver
[`docs/B18_009_SENSITIVITY.md`](B18_009_SENSITIVITY.md) para la tabla de
sensibilidad y el detalle.

## Historial: candidato aislado y bloqueo (2026-10-16, pre-integración)

**Estado de entonces: abierto/bloqueado para integración.** Se construyó un
candidato de solve conjunto, pero no superaba las puertas de estabilidad
independiente y CPU. El motor/plugin conservaba `z^-1` + LP de 2,5 kHz y el
fallback resistivo con NFB, sin cambiar `ALGORITHM_VERSION`.

**Publicación autorizada por el usuario (2026-10-16):** este diagnóstico y el
candidato aislado se versionan junto con sus dependencias B18-010/011/012/013/015
en `feat/7199-dynamic`.

## Actualización: causa raíz aislada en SPICE

`scripts/spice/nfb_sensitivity.py` parametriza el equivalente del OT y aísla
cada elemento. Resultado: los márgenes negativos provienen de la resonancia
estimada del OT (`Lleak = 12 mH`, `Cp = 100 pF` → 145 kHz) que cae en pleno
cruce del lazo, más el amortiguamiento HF ausente y la carga RLC estimada; la
topología NFB, la polaridad (realimentación negativa), la pasividad y la fuente
de 135 kΩ **no** son la causa. La corrección aplicada (`Cp = 1,5 nF`) da margen
positivo y transitorio estable en 8 Ω y RLC.

Paso 5 (re-test del candidato aislado con el `Cp` corregido): el caso RLC
cerrada que fallaba (62.335 fallos/65.536) **converge con 0 fallos** a las
cuatro tasas efectivas y a 0,1 mV/10 mV/1 V, validado contra la nueva
referencia `coupled_ac_corrected.csv` (error ≤0,107 dB / 0,61° a 768 kHz).

## Integración en el motor (cierre)

- `Cp` del OT corregido en `assets/models/b18n_1967_calibration.json`
  (`parasitic_capacitance_f` 100 pF → 1,5 nF, `estimated` con método/fecha) y en
  el espejo `circuit.rs::test_circuit`.
- `stages/output_network.rs` pasa a ser el camino de producción: Newton 11D
  directo (el nodo G ya no es lineal por la conducción de rejilla del pentodo),
  rama de corriente de rejilla del 7199 añadida, búsqueda de línea con norma L2.
- `engine.rs`: `LinearChain` usa `OutputNetwork` en lugar de
  `inverter` + `power` + `feedback_lowpass`/`feedback_state`. Se elimina el
  `z^-1` y el polo de 2,5 kHz. La carga RLC estimada ya no cae al fallback
  resistivo (el lazo físico es estable con ella).
- `ALGORITHM_VERSION` → `…-supply-ot471-linear-nfb-coupled-v2` (plugin y
  render).
- Fixtures versionados nuevos (los históricos con `Cp`=100 pF se conservan):
  `coupled_ac_corrected.csv`, `coupled_dc_corrected.csv`,
  `ot471_linear_spice_cp15n.csv`, `speaker_load_spice_cp15n.csv` y los
  `sensitivity_*`.
- Guardia de CPU re-basada a **40 µs** (medido ~36-38 µs estéreo 4×/48 kHz): la
  etapa acoplada de 11 nodos cuesta más que el lazo artificial anterior. No es
  una relajación de tolerancias de solver.
- `nfb_coupled.rs` valida topología contra el golden corregido a 768 kHz y
  de-ignora la prueba de RLC cerrada (`reactive_closed_response_matches_spice`),
  que ahora pasa. `output_transformer_ac`/`reactive_load` usan los golden
  corregidos; `nfb_rate_invariance` se reformula (ya no hay polo de
  compensación dependiente de la tasa).
- Render E2E (bass, 4×, 48 kHz, `--speaker direct`): pico 4,9170 → **4,9705**,
  RMS 1,4761 → **1,4957**, offset +0,0017, latencia 47; informe en
  `target/reports/b18n_b009.json`. Cambio audible registrado con la subida de
  `ALGORITHM_VERSION`.
- Presupuesto CPU **re-basado a 40 µs** (medido ~36-38 µs estéreo 4×/48 kHz);
  el `OutputNetwork` de 11 nodos cuesta más que el lazo artificial anterior.
  No es una relajación de tolerancias de solver; una optimización futura
  (Schur no lineal correcto / Jacobiano analítico en los triodos del previo)
  podría devolverlo a 32 µs.

## Trabajo implementado para revisión

- `stages/output_network.rs`: candidato **aislado del motor**, once nodos
  eléctricos (G/P/S/K/TG/TP/TK/U/L/A/B), eliminación exacta del nodo lineal G
  y Newton 10D con Jacobiano analítico y búsqueda de línea. Resuelve 7199,
  carga de las salidas del split-load, acoplos de 50 nF + stopper de 1 kΩ,
  dos 7027A, puerto del OT y NFB de 22 kΩ en la misma muestra. Sin heap ni
  avance de estados en tanteos; solo publica las historias tras converger.
- Nueve capacitancias del overlay, `origin=datasheet`, con carga completa o
  error de configuración si falta alguna. RCA 7199 7-63, DATA 1: pentodo
  0,06 pF **máximo** / 5 pF / 2 pF; triodo 2 / 2,3 / 0,3 pF. RCA 7027-A
  8-59, DATA 1: 1,5 / 10 / 7,5 pF. Orden gp/gk/pk. Las capacitancias a
  cátodo incluyen los electrodos agrupados en el datasheet; aproximación a
  masa AC, sin añadir valores supuestos de zócalo o cableado.
- `OutputTransformerModel::secondary_voltage_port()`: tensión y sensibilidad
  afines de la misma muestra, sin mutar historias. Test de equivalencia con
  `advance()` para 8 Ω, 16 Ω y RLC.
- `KorenPentodeModel::plate_grid_slopes()`: evaluación de placa y derivadas
  para pantalla fija; evita recalcular la partición de pantalla en tanteos.
- Referencias ngspice nuevas y reproducibles: 24 puntos AC (magnitud y fase),
  cuatro puntos de reposo completos, retorno de lazo y transitorio. El
  manifiesto contiene sus SHA-256.

## Corrección de la referencia anterior

La exploración `nfb_loop_reference.py` de sprint §22 no es una puerta válida
de aceptación: su orientación E/F del secundario no coincide con la
convención del motor; además coloca `R_core` asimétricamente en una mitad
del primario DC. No se sobrescribió ese archivo histórico. La nueva
`nfb_coupled_reference.py` separa el puerto común del CT y el diferencial,
conservando el equivalente de OT declarado en B18-013, y orienta E/F de
forma consistente con `secondary_voltage_port()`.

**Se retira la conclusión de §22.3 de que una respuesta AC sin pico prueba
estabilidad analógica.** Un análisis AC puede devolver una respuesta suave
aun cuando existan polos inestables. Se midió el retorno y se comprobó el
transitorio; el problema ya no se puede atribuir solo al retardo digital.

## Evidencia independiente que bloquea el cierre

Rieles fijos: previo 380 V, triodo 460 V, CT 505 V, pantallas 500 V,
bias −55 V. Mezcla idealizada con Rth=135 kΩ (270 kΩ/2), OT y RLC estimados
del overlay. La toma de NFB es un sensor ideal sin carga sobre el secundario:
no modela la pequeña corriente reflejada de la 22 kΩ. La impedancia real
de salida del tono y los parásitos de montaje siguen sin caracterizar.

Retorno: se abre únicamente el sensor de tensión y se inyecta 1 V AC,
manteniendo la carga de 22 kΩ en el cátodo y el punto DC. `T=-Vreturn/Vtest`,
ecuación característica `1+T`. Barrido 1 Hz–100 MHz, 200 puntos/década.
La fase se referencia a la rama de banda media. Se registran todos los
cruces; los márgenes siguientes corresponden al cruce descendente HF.

| Carga | Cruce de 0 dB HF | Margen de fase | Margen de ganancia a −180° |
|---|---:|---:|---:|
| Nominal 8 Ω | 212,4 kHz | **−55,38°** | **−11,23 dB** a 150,9 kHz |
| RLC estimada | 324,8 kHz | **−9,54°** | **−4,95 dB** a 247,0 kHz |

Comprobación transitoria ngspice: pulso de **10 µV**, retirado a 2,1 µs,
paso máximo 0,1 µs, simulación de 2 ms. El pico del secundario crece de
menos de 0,5 mV en los primeros 10 µs a unos **15 V nominal / 41 V RLC**,
y la oscilación persiste después de retirar la entrada. Las amplitudes
grandes son diagnósticas: los subcircuitos SPICE de esta referencia no
incluyen conducción de rejilla; no predicen amplitudes reales de hardware.
La combinación de márgenes negativos y crecimiento desde pequeña señal
sí invalida la aceptación del lazo propuesto.

Sensibilidad adicional: Rth=220/270/330 kΩ (rango alrededor de las lecturas
históricas de la entrada de mezcla) tampoco corrige el margen HF: nominal
aprox. −56…−54° y RLC −12…−13°. No se eligió una resistencia por ajuste de
estabilidad ni se modificaron parámetros estimados del OT para pasar tests.

## Resultados del candidato Rust

- Reposos: cuatro combinaciones contrastadas con SPICE, tolerancia 50 mV
  por nodo.
- AC validada: nominal abierta/cerrada a tasa efectiva 192 kHz y RLC abierta
  a 768 kHz; error máximo **0,0313 dB / 0,7783°**, cero fallos de Newton en
  esos casos. El caso RLC abierto usa 768 kHz para no confundir la resonancia
  HF y el warping de TPT con un error topológico; a 192 kHz la diferencia
  en 18 kHz es 0,768 dB / 3,87°, límite conocido, no oculto por tolerancias.
- **RLC cerrada a 192 kHz falla:** seno pequeño de 93,75 Hz, 0,1 mV en la
  entrada de mezcla, 62.335 fallos/65.536 muestras en el ensayo registrado.
  `reactive_closed_response_pending_b18_009` conserva la prueba de aceptación
  como `#[ignore]` con motivo explícito; al ejecutarla debe seguir fallando
  hasta resolver el bloqueo. Esto no es una prueba aprobada.
- El candidato llegó a pasar la batería existente `nfb_stability` del motor,
  pero eso **no cierra** la estabilidad analógica ni el caso reactivo anterior.
- CPU al integrar temporalmente el candidato: primera medida 41,3 µs por
  muestra estéreo a 4×/48 kHz; tras cachear matriz/puertos, eliminar el nodo G
  y reutilizar corrientes, última medida **32,487 µs**, aún por encima de la
  guardia **32 µs**, que no se amplió. Es un ensayo local, no una medida DAW.
- La integración se retiró después de estos fallos. El candidato permanece
  accesible solo construyendo `OutputNetwork` explícitamente; no hay control
  de host ni selección automática que lo active.

## Reproducción

```powershell
python scripts/spice/nfb_coupled_reference.py
python scripts/spice/nfb_loop_margins.py
python scripts/check_reference_manifest.py
cargo test -p b18n-dsp --release --test nfb_coupled -- --nocapture
# Prueba de aceptación actualmente FALLIDA, ejecución explícita:
cargo test -p b18n-dsp --release --test nfb_coupled reactive_closed_response_pending_b18_009 -- --ignored --nocapture
```

Las referencias CSV/JSON y los netlists reproducibles están en
`assets/fixtures/reference/nfb/`. Logs de desarrollo y resultados finales
de esta ronda: `target/baseline/b009_*` (ignorados por git).

## Qué falta para continuar B18-009

1. Revisar la topología y caracterización HF del conjunto real: conexiones
   de NFB, impedancia compleja de fuente, montaje y equivalente del OT.
   El equivalente de OT actual sigue siendo una estimación de ingeniería.
2. Corregir con evidencia el modelo continuo y demostrar margen de retorno
   y transitorios estables **antes** de integrar otra vez el solve discreto.
   No basta con añadir las capacitancias del datasheet ni con quitar el LP.
3. Resolver el residual RLC de pequeña/gran señal y el impulso extremo ya
   registrado en B18-015, a todas las tasas y factores.
4. Cumplir CPU ≤32 µs sin relajar tolerancias para aparentar convergencia;
   después, matriz E2E, alias, RT-safety, incremento de versión de algoritmo,
   render audible y bundle. La validación DAW/hardware queda separada.


## Verificacion final del camino conservado

- `cargo fmt --all -- --check` y clippy workspace/all-targets con `-D warnings`: en verde.
- Suites completas debug y release: 218 pruebas pasando y 5 ignoradas
  (4 previas + 1 del candidato), antes de dos tests unitarios finales de
  capacitancias/derivadas, que tambien pasan por ejecucion dirigida release.
- La prueba candidata ignorada se ejecuto expresamente y reprodujo los
  62.335 fallos: no se contabiliza como aprobada.
- Manifiesto: 17 fixtures con SHA verificados.
- Bundle VST3 regenerado correctamente, sin instalarlo ni publicar.
- Render conservado: 24.000 muestras bit a bit iguales a
  `target/renders/b18n_after15.wav`, diferencia maxima 0. Pico 4,916955,
  RMS 1,4760989, latencia 47 muestras a 4x. El fingerprint del modelo
  incorpora los metadatos nuevos de capacitancias; eso no implica un cambio
  audible ni valida el candidato.
