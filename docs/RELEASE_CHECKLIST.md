# Checklist de release

## Código y calidad

- [x] `cargo fmt --all -- --check` sin diferencias.
- [x] `cargo clippy --workspace --all-targets -- -D warnings` limpio.
- [x] `cargo test --workspace` verde (190 tests tras B18-012; determinismo y robustez incluidos).
- [x] Sin `unwrap()`/`panic!` en rutas de audio; sin asignaciones en
  `process()`/`reset()` (test no-alloc del plugin; solver const-genérico).
- [x] `reset()` verificado; cambio de sample rate y de tamaño de bloque probados.
- [x] Presupuesto de CPU medido con guardia (Ola 8): bench estéreo 4× a 48 kHz
  con 27-29,5 µs/muestra, guardia de `cpu_budget.rs` en 32 µs; niveles DAW
  verificados (`daw_levels.rs`: instancia por defecto sin rebasar la escala
  del host y 0 fallos de congelación a 0,2/1,0 V).

## Licencias

- [x] Revisado el estado de `vst3-sys`/NIH-plug (GPLv3) y cumplimiento antes de
  distribuir (proyecto `GPL-3.0-only`; commit de NIH-plug fijado).
- [ ] IR/presets/curvas de válvulas con licencia compatible y procedencia documentada.
- [ ] PDFs históricos **no** incluidos en el paquete.

## Plugin

- [x] `VST3_CLASS_ID` propio y estable (`B18N1967VST3DSP!`); IDs de parámetros
  congelados por test.
- [x] Bundle generado con `cargo xtask bundle b18n-plugin --release`
  (`target/bundled/B18N 67.vst3`).
- [ ] Steinberg VST3 Validator sin errores (pendiente: binario no disponible en
  el equipo; paso manual).
- [ ] Probado en al menos un DAW: automatización, guardado/restauración, suspensión
  (pendiente: paso manual).
- [x] Latencia reportada y coherente con la configuración de oversampling
  (47 muestras a 4×, 0 a 1×; verificada con impulso).

## Medición y calibración

- [ ] Reposo DC contrastado y documentado con tolerancias.
- [ ] Tono (25 combinaciones) comparado contra SPICE independiente.
- [ ] Alias medido a 1×/4× (8× no implementado); THD vs. nivel documentado.
- [ ] Todo dato no histórico etiquetado (`estimated`/`measured`/`datasheet`).
- [ ] Nivel de fidelidad del release declarado (ver `MODEL_ASSUMPTIONS.md`).

## Distribución

- [ ] Versión y `CHANGELOG.md` actualizados.
- [ ] README refleja limitaciones y carácter no oficial.
- [ ] `Cargo.lock` versionado; toolchain fijada.
