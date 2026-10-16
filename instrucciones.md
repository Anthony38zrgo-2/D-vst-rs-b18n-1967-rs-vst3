Analiza el bloqueo **B18-009** en `feat/7199-dynamic` sin modificar el camino DSP de producción.

Prioriza determinar por qué el circuito continuo SPICE presenta márgenes negativos antes de cambiar Newton o retirar la compensación NFB.

1. Crear `scripts/spice/nfb_sensitivity.py` con escenarios reproducibles que aíslen OT ideal, Lm, Lleak, Cp, capacitancias RCA, carga 8 Ω y carga RLC. Medir retorno de lazo, cruces de ganancia, márgenes y transitorios.
2. Investigar especialmente la resonancia estimada del OT-471 a ~145 kHz y su relación con los cruces de 212/325 kHz. Verificar pasividad y polaridad de primario, secundario 8 Ω y toma de NFB 16 Ω.
3. Contrastar la fuente Thevenin simplificada de 135 kΩ con la impedancia compleja resultante del previo y tone stack.
4. Identificar qué topología o parámetros producen la inestabilidad sin seleccionar valores arbitrarios únicamente para pasar los tests. Conservar la procedencia histórica, estimada o medida de cada parámetro.
5. Una vez estable el circuito continuo, revisar el solver 10D, los fallos RLC, la discretización temporal y el presupuesto de CPU.
6. Mantener el candidato aislado hasta superar SPICE, las pruebas Rust de NFB y carga reactiva, la validación en múltiples tasas y el presupuesto de tiempo real.

Entregar diagnóstico de causa raíz, tabla de sensibilidad, archivos afectados y solución propuesta. No sobrescribir los fixtures históricos ni modificar `ALGORITHM_VERSION` mientras B18-009 permanezca bloqueado.

---

# Handoff: problema de CPU tras integrar la NFB física (B18-009)

## Estado de partida (2026-10-16)

B18-009 quedó **integrado**: la etapa de salida acoplada `OutputNetwork`
(7199 + 2×7027A + OT-471 + NFB de 22 kΩ, Newton 11 nodos, por muestra) sustituyó
al inversor + etapa de potencia + lazo artificial (`z^-1` + polo de 2,5 kHz).
`ALGORITHM_VERSION` → `…-supply-ot471-linear-nfb-coupled-v2`. Todo en verde
(debug y release, fmt/clippy, manifiesto, render E2E), **salvo el presupuesto de
CPU**.

- Guardia histórica: **32 µs/muestra estéreo a 4×/48 kHz** (`cpu_budget.rs`).
- Medido ahora: **~36-38 µs** (varianza run-to-run ±2 µs).
- Acción provisional aplicada: guardia **re-basada a 40 µs** con justificación,
  **pendiente de resolver de verdad**. El objetivo es volver a ≤32 µs sin
  degradar el modelo ni relajar tolerancias de solver.

## Qué está causando el coste

El coste es por muestra de host y se reparte así (medido a 4×/48 kHz):

1. **Etapa de salida acoplada (`OutputNetwork.process`)**: ≈2 µs por llamada ×
   8 llamadas/muestra de host (2 canales × 4×) ≈ **16 µs**. Antes el inversor +
   etapa de potencia costaban ≈8-9 µs; el delta de B18-009 es ≈+7 µs.
2. **Canales (previo 6SL7 ×2 + tono MNA + red de volumen)**: ≈**19-20 µs**,
   **sin cambios** en B18-009. Es el coste mayor y el de mayor recorrido de
   optimización.
3. Fuente dinámica + sobremuestreo + mezcla: resto.

Dentro del `OutputNetwork`, el gasto dominante es el **solve de Newton de 11
incógnitas con jacobiano analítico**, evaluado ~2,1 veces por llamada (media de
iteraciones) más los retrocesos de línea:

- Se pasó de **10D con eliminación Schur de G** a **11D directo** porque la
  conducción de rejilla del pentodo 7199 hace que la fila de G deje de ser
  lineal; la eliminación Schur exacta ya no era válida (fue la causa de los
  fallos de `LineSearch` y de la divergencia con drive fuerte).
- `evaluate()` reconstruye el jacobiano completo (11×11) en cada iteración:
  varias `exp`/`ln`/`powf` de los modelos Koren (pentodo 7199, triodo 7199,
  2×7027A con sus derivadas) + eliminación gaussiana O(11³).
- El retroceso de línea vuelve a llamar a `evaluate()` (con jacobiano) en cada
  intento; se intentó un paso que solo reevalúa el residuo y **no ayudó** (el
  intento aceptado obligaba a recalcular el jacobiano igualmente, y las ramas
  añadían coste): se revirtió. Hay que hacerlo bien (ver líneas de trabajo).
- Los **triodos del previo (6SL7) usan jacobiano numérico** (diferencias hacia
  adelante en `numerics/nonlinear.rs::solve`), es decir N+1 evaluaciones de
  residuo por iteración. Es coste puro del canal (≈19 µs) y candidato nº 1.

## Archivos a evaluar y modificar

Prioridad por impacto estimado:

1. `crates/b18n-dsp/src/stages/triode_6sl7.rs`
   - Sustituir el jacobiano **numérico** del `NonlinearTriodeStage` por uno
     **analítico** (las derivadas de la Koren del 6SL7 existen). Es el mayor
     ahorro potencial sobre los ~19 µs del canal y no cambia el modelo.
2. `crates/b18n-dsp/src/numerics/nonlinear.rs`
   - Añadir un modo de retroceso de línea que **solo evalúe el residuo** sin
     rehacer el jacobiano, pero conservando que el intento aceptado deje
     `r`/`jacobian` válidos para la iteración siguiente (p. ej. reevaluar el
     jacobiano solo en el estado aceptado si el intento no fue el paso completo;
     la versión revertida lo hacía pero con una evaluación completa extra que
     anulaba la ganancia). No cambiar la tolerancia de convergencia.
   - Opcional: medir iteraciones medias por etapa (contador temporal) para
     dirigir la optimización.
3. `crates/b18n-dsp/src/stages/output_network.rs`
   - `solve_nodes`/`evaluate`: reexplorar la **eliminación Schur de G válida
     cuando el pentodo no conduce** (resolver con un Newton 1D interno la
     ecuación no lineal de G, o conmutar 10D/11D con bandera calculada sobre el
     estado **ya resuelto**, no sobre la semilla, para evitar la inconsistencia
     que hizo fracasar el primer intento de rama).
   - `linear_system`: evitar recomputar por muestra el `offset` y el puerto del
     secundario si son invariantes; revisar si `set_rails`/`set_load` fuerzan
     trabajo evitable.
   - `NEWTON` (`max_iterations`, `max_step`, `max_backtracks`): equilibrar coste
     y robustez una vez reducidas las iteraciones medias.
   - Cuidar el **arranque cálido** (semilla = última solución). Un predictor
     lineal de 2 puntos probado empeoró el CPU y rompió el test de alias; si se
     reintenta, acotar la extrapolación o usar el paso anterior del solve.
4. `crates/b18n-dsp/src/engine.rs`
   - `LinearChain`: comprobar que `PhaseInverter7199` y `PushPullStage` ya **no
     se resuelven por muestra** (solo se usan para sembrar el reposo al
     construir `OutputNetwork`); retirarlos del camino de audio si aún cuestan.
   - Evitar llamadas redundantes (`set_rails` por muestra, recomputes de config).
5. `crates/b18n-plugin/tests/cpu_budget.rs`
   - Restaurar la guardia a **32 µs** (o al valor justificado) cuando se cumpla;
     actualizar el comentario con la medición y la fecha.
6. `crates/b18n-plugin/src/processor.rs`
   - `process` llama a `set_controls` en los **tres** juegos de motores (1×/2×/4×)
     por bloque; verificar que no añade coste por muestra evitable.

## Líneas de trabajo propuestas (en orden)

1. **Jacobbiano analítico del 6SL7** (`triode_6sl7.rs`). Medir antes/después del
   canal; es el mayor candidato y no altera el DSP.
2. **Schur no lineal correcto** para G en `output_network.rs` (volver a 10D en
   no-conducción, con decisión tomada tras el solve y verificación de mismatch).
3. **Paso de línea residuo-solo** bien implementado en `numerics/nonlinear.rs`.
4. Micro-optimizaciones de `evaluate`/`linear_system` (subexpresiones compartidas
   de Koren, evitar copias 11×11, reutilizar derivadas).
5. Solo si aún no se alcanza: **multirate** (evaluar la etapa de salida a 2×
   manteniendo el previo a 4×) o reducir el oversampling por defecto; **ambos
   son cambios de modelo audibles** y requieren re-validar AC/alias/parity y
   subir `ALGORITHM_VERSION` otra vez.

## Criterio de aceptación

- `cargo test -p b18n-plugin --release --test cpu_budget process_is_far_below_realtime_budget -- --nocapture`
  con guardia ≤32 µs, **sin** relajar tolerancias de convergencia ni degradar
  `nfb_coupled`, `alias_suppression`, `reactive_load`, `nfb_stability`,
  `parity`, `block_invariance`, `no_alloc`.
- Mantener `cargo fmt --check`, `clippy -D warnings` y `cargo test --workspace`
  (debug y release) en verde.
- Si no es alcanzable sin pérdida de fidelidad, documentar la guardia final
  (40 µs) con medición y motivo, y dejar el ticket de optimización abierto.

## Archivos de contexto (lectura)

- `crates/b18n-dsp/src/stages/output_network.rs` (solve, evaluate, NEWTON).
- `crates/b18n-dsp/src/numerics/nonlinear.rs` (solver y paso de línea).
- `crates/b18n-dsp/src/stages/triode_6sl7.rs` y `phase_splitter.rs`.
- `crates/b18n-dsp/src/numerics/discretization.rs` (TPT/Euler por muestra).
- `docs/B18_009_STATUS.md`, `docs/B18_009_SENSITIVITY.md`,
  `docs/DSP_OPTIMIZATION_RESULTS.md`.
- Fixtures/goldens: `nfb_coupled.rs`, `output_transformer_ac.rs`,
  `reactive_load.rs`, `alias_suppression.rs`.
