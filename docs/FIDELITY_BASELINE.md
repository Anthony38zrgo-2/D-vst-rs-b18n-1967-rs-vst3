# Línea base de fidelidad (B18-001)

**Estado:** aprobada · **Fecha:** 2026-10-14 · **Rama:** `main` \
**Commit:** `fecf203bab9e48a899206c4400190332e5eba859` (`fixes`) \
**Propósito:** congelar el estado verificable **antes** de modificar DSP (puntos P0/P1 de
`b18-fixes.md`). Cualquier cambio audible posterior debe compararse contra este documento.

> Esta línea base es una **regresión técnica**, no una certificación acústica. No se ha
> realizado escucha A/B, ni medición de un B-18N físico, ni comparación contra hardware.
> Las etiquetas de procedencia (`estimated`, `SPICE`, `measured`) son las reglas de B18-021.

## 1. Entorno de captura

| Dato | Valor |
|---|---|
| Sistema operativo | Windows (win32), host de desarrollo del proyecto |
| Target Rust | `x86_64-pc-windows-msvc` (toolchain fijado en `rust-toolchain.toml`) |
| `rustc` | 1.95.0 (59807616e 2026-04-14) |
| `cargo` | 1.95.0 (f2d3ce0bd 2026-03-21) |
| ngspice | 47 local en `tools/ngspice/` (ignorado por git; no redistribuido) |
| Fecha de captura | 2026-10-14 |

## 2. Hashes de los artefactos de modelo

| Artefacto | SHA-256 |
|---|---|
| `assets/models/B18N_1967_simulation_parameters.json` (canónico) | `d61d122d232a896b4269dd8769d54f6178540ed3b5afd655c9b403966d2a6a0b` |
| `assets/models/b18n_1967_historical_errata.json` | `ac61b39d0bf2b42a28028b012b2abb9522d75d1e358523de595246baf05fd878` |
| `assets/models/b18n_1967_calibration.json` | `38ea6fbdab59c016f8050f2a399d65d6a566fec107b527a32621660d3666d82a` |
| JSON efectivo (canónico + errata, calculado por el cargador) | `7dbd8eafa6b23d56a902da6769cabadc010071b6dc5f961866297f6bfe02b3af` |

Revisión de esquema declarada: `1967-05` (B-18N/B-15ND 5/67).

## 3. Estado de herramientas (2026-10-14)

| Comando | Resultado |
|---|---|
| `cargo fmt --all -- --check` | pasa (sin salida) |
| `cargo clippy --workspace --all-targets -- -D warnings` | pasa (0 warnings) |
| `cargo test --workspace` | **151 pasan, 0 fallan, 3 ignorados** |
| `cargo test --workspace --release` | **151 pasan, 0 fallan, 3 ignorados** |

Los 3 ignorados son los benchmarks manuales que imprimen cifras de CPU
(`bench_stereo_1x_print`, `bench_stereo_2x_print`, `bench_stereo_4x_print` en
`crates/b18n-plugin/tests/cpu_budget.rs`); su guardia de regresión
(`process_is_far_below_realtime_budget`) sí se ejecuta. Fallos de convergencia en las
pruebas: 0 (los propios tests lo verifican por etapa).

Registro por suite (idéntico en debug y release):

| Suite | Pasan | Fallan |
|---|---|---|
| `b18n-config` (lib + `errata` + `loader`) | 28 | 0 |
| `b18n-dsp` lib | 89 | 0 |
| `b18n-dsp` `alias_suppression` | 3 | 0 |
| `b18n-dsp` `nfb_stability` | 4 | 0 |
| `b18n-dsp` `phase_inverter_ac` | 2 | 0 |
| `b18n-dsp` `tone_stack_response` | 3 | 0 |
| `b18n-plugin` lib | 3 | 0 |
| `b18n-plugin` `block_invariance` | 2 | 0 |
| `b18n-plugin` `cpu_budget` | 1 (+3 ignorados) | 0 |
| `b18n-plugin` `daw_levels` | 2 | 0 |
| `b18n-plugin` `latency` | 2 | 0 |
| `b18n-plugin` `no_alloc` | 1 | 0 |
| `b18n-plugin` `parity` | 3 | 0 |
| `b18n-plugin` `speaker_profiles` | 2 | 0 |
| `b18n-render` lib | 6 | 0 |

## 4. Render offline de referencia

Comando (sintaxis verificada con `--help` en la versión local):

```powershell
cargo run -p b18n-render --release -- `
  --input assets/fixtures/signals/bass_test.wav `
  --output target/renders/b18n_baseline.wav `
  --sample-rate 48000 `
  --oversampling 4 `
  --channel bass `
  --cabinet off `
  --speaker direct `
  --measure tone `
  --report target/reports/b18n_baseline.json
```

Resultados (`target/reports/b18n_baseline.json`):

| Métrica | Valor |
|---|---|
| `algorithm_version` | `0.1.0-nonlinear-triodes-power-7199-inverter-nfb-25k-comp-2x-speaker-v1` |
| Latencia reportada | 47 muestras (4×, tasa base 48 kHz) |
| Pico | 3,0837548 |
| RMS | 1,0591571 |
| Offset DC | +0,0014739683 |
| Curvas de tono | 25 combinaciones × 31 frecuencias |
| `fidelity` | `topology_unverified` |

Has de las señales del baseline:

| Artefacto | SHA-256 |
|---|---|
| `assets/fixtures/signals/bass_test.wav` (entrada) | `5d9a4f6f71760690425494b14f31bdc139e101d0c596911e95ca05e9bc3dbdbd` |
| `target/renders/b18n_baseline.wav` (salida, no versionada) | `8e8732a6dbd65fe5174786781bab79a550de9be21cabfc53cb5b78b64324f1bc` |

La salida vive en `target/` (ignorado por git): es reproducible con el comando anterior.
La entrada `bass_test.wav` es una señal sintética del proyecto (E2 + armónicos, envolvente
exponencial), no una grabación; véase `assets/fixtures/signals/README.md`.

## 5. Cobertura declarada y matrices

- **Sample rates probados:** 44,1 / 48 / 96 / 192 kHz (`nfb_stability`,
  `speaker_profiles`, `latency`).
- **Oversampling:** 1× / 2× / 4× (8× no implementado; no se expone).
- **Latencia por factor:** 0 / 31 / 47 muestras de tasa base (FIR de 63 taps).
- **Cargas:** 8 Ω y 16 Ω resistive (`nfb_stability`); no hay carga reactiva todavía (B18-015).
- **Sample-exactitud:** adaptador VST3 ≡ motor directo por factor (`parity`), invariancia de
  bloque 1/7/64/257/1024 (`block_invariance`), sin asignaciones en audio (`no_alloc`).

## 6. Limitaciones y supuestos no validados (hipótesis, no medidas)

1. **OT-471:** `Lm`, inductancia de fuga, capacitancias e histéresis siguen `null`; la etapa
   principal usa el modelo ideal con DCR de `power_7027a.rs` (B18-013/014).
2. **Gabinete/altavoz:** los perfiles 15"/18" son filtros IIR elegidos como aproximación
   moderna; no hay IR ni curva de impedancia medidas (B18-016).
3. **NFB:** separación explícita `z^-1` y polo de compensación declarado en 2,5 kHz; el
   estado del polo respecto al factor de sobremuestreo se corrige en B18-002.
4. **7027A:** parámetros Koren de la 6LGC publicada; sin fit directo al 7027-A (B18-010/011).
5. **Fuente:** B+/pantallas/bias rígidos; sin 5AR4, filtrado ni sag dinámicos (B18-012).
6. **Niveles:** `1.0` de muestra = 1,0 Vp y trim de salida −16,5 dB son convenciones
   modernas, no datos Ampeg (B18-017).
7. **Previo:** impedancia de rejilla del 6SL7#2, mezclador y jacks Bass/Guitar usan
   aproximaciones registradas (B18-005/006/007).

La existencia de SPICE no eleva ninguna de estas categorías a `hardware-validated`.

## 7. Reproducción

Los logs completos de esta captura se conservan localmente en `target/baseline/` (ignorado
por git): `fmt.txt`, `clippy.txt`, `test_debug.txt`, `test_release.txt`,
`render_baseline.txt`. Para una captura nueva, ejecutar en la raíz:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --workspace --release
```

y el comando de render de la sección 4. Cualquier divergencia de hashes (sección 2) debe
resolverse antes de comparar respuestas: indica cambio de modelo o de errata.

## 8. Criterios de bloqueo asociados

- Si algún test de la sección 3 falla tras un cambio, **no** se aprueba un baseline nuevo:
  se conserva el log y se documenta la regresión.
- No se ajustan tolerancias de tests para ocultar fallos; se corrigen o se justifica el
  cambio de criterio por escrito.
- El manifiesto de fixtures (`assets/fixtures/reference/manifest.json`, verificado por
  `scripts/check_reference_manifest.py`) fija la procedencia de cada golden existente.

## 9. Cambios posteriores registrados

- **B18-002 (2026-10-14, rama `fix/nfb-rate`, cambio audible):** polo de compensación de
  la NFB construido con la tasa de proceso efectiva y discretizado con TPT/prewarp
  (antes: tasa base + Euler). `ALGORITHM_VERSION` →
  `0.1.0-nonlinear-triodes-power-7199-inverter-nfb-25k-tpt-rate-2x-speaker-v1`. Render
  E2E nuevo: pico 3,0776, RMS 1,0571, offset +0,00140, latencia 47. Test nuevo
  `nfb_rate_invariance.rs` (dispersión 1×/2×/4× ≤0,64 dB/8,1° hasta 2,5 kHz). El test de
  alias se re-basó a cota absoluta ≤ −18 dBc en 1× y 4× (medido −31,7/−25,1 dBc; la
  comparación relativa anterior era un artefacto del polo dependiente del OS). Detalle en
  `CHANGELOG.md`.
- **B18-004 (2026-10-14, rama `fix/trim-smoothing`, sin cambio de sonido):** los trims de
  entrada/salida pasan de `15 ms × oversampling` a **15 ms físicos** (antes 30/60 ms a
  2×/4×). El render de la sección 4 es **byte-idéntico** (mismo SHA-256 de salida), por lo
  que los valores de este baseline siguen vigentes; test nuevo
  `crates/b18n-dsp/tests/control_smoothing.rs`. Detalle en `CHANGELOG.md`.
- **B18-003 (2026-10-14, rama `fix/tone-coupling-mna`, cambio audible):** el acoplo de
  0,022 µF del 6SL7#2 vive dentro del MNA del tono (nodo `SOURCE` + `Ro` + capacitor +
  red), eliminando el doble divisor externo (~6 dB); golden SPICE acoplado
  (`tone_stack_coupled_spice.csv`, 1e-6 dB / 1e-4°). Trim de salida por defecto del
  plugin −16,5 → −20,0 dB para mantener la protección del host. `ALGORITHM_VERSION` →
  `…-tone-mna-speaker-v1`; render E2E nuevo: pico 5,1420, RMS 1,5610, offset +0,0030.
  Detalle en `CHANGELOG.md`.
- **B18-005/006/007 (2026-10-14, rama `fix/preamp-loading`, cambio audible):** red de
  volumen física resuelta en el Newton del 6SL7#2 (pot a masa, golden ngspice), mezclador
  trazado sin fuga asumida (dos 270 kΩ directos al nodo de rejilla; manual V3/pin 7 =
  220–330 kΩ) y jacks diferenciados (`bass` con el shunt de 100 kΩ del jack libre,
  `guitar` sin él, `instrument` directo). `ALGORITHM_VERSION` →
  `…-tone-mna-preamp-loading-speaker-v1`; render E2E nuevo: pico 4,8210, RMS 1,4451,
  offset +0,0028; instancia stock del plugin 0,771 (−2,3 dBFS). Detalle en `CHANGELOG.md`.
