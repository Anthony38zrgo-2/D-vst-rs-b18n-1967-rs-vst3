# Arquitectura DSP

## Capas

```text
assets/models/*.json ──► b18n-config ──► CircuitDefinition (en b18n-dsp)
                                             ▲
                              b18n-plugin ───┤ (NIH-plug / VST3, Hito E)
                              b18n-render ───┘ (offline, WAV + reportes)
```

- `b18n-dsp` no conoce el sistema de archivos, ni serde, ni APIs de host.
- `CircuitDefinition` es inmutable; `RuntimeControls` lleva posiciones de mandos.
- Un motor por canal; sin variables globales mutables.

## Cadena lógica (Hito B en adelante)

```text
audio in ─► input ─► 6SL7#1 ─► volumen ─► 6SL7#2 ─► tono (MNA) ─► mezcla 270k
        ─► 7199 pentodo ─► 7199 triodo (inversor) ─► 2×7027A push-pull
        ─► OT-471 ─► carga de altavoz ─► (IR opcional) ─► salida
                  └──────── NFB 22k ◄── secundario 16 Ω
```

El lazo OT↔NFB es un sistema acoplado: no se asume que funcione como etapas independientes
en cascada sin resolver la realimentación.

## Fases de implementación

1. **Lineal (Hito B, implementado):** entrada, 6SL7 de pequeña señal con acoplos RC,
   volumen con taper asumido, red de tono resuelta con MNA (frecuencial y TPT temporal),
   mezcla de 270 kΩ al nodo de rejilla del inversor (B18-006: sin acoplo en ese nodo),
   OT-471 ideal con DCR y carga de 8 Ω.
2. **Triodos no lineales (Hito C parte 1, implementado):** modelo Koren del
   6SL7 con parámetros de calibración, Newton amortiguado con jacobiano
   analítico, cota de paso y retroceso de línea; conducción de rejilla
   aproximada; reposo resuelto al inicializar; sobremuestreo 4× con FIR de
   media banda de 63 taps y latencia reportada (**47 muestras**, corregida en
   P1.7 midiendo la respuesta al impulso; el valor anterior de 93 contaba cada
   etapa al doble de su retardo real). El solver Newton es const-genérico y
   sin asignaciones dinámicas; `reset()` restaura estado reutilizando buffers
   (verificado por el test no-alloc del plugin).
3. **Push-pull 7027A (Hito C parte 2, implementado):** pentodos Koren de placa
   y pantalla, bias fija con acoplo que bloquea continua, conducción de rejilla,
   OT ideal con toma central y DCR.
4. **Inversor 7199 real (P1.4, implementado):** pentodo Koren ajustado al
   boletín RCA con su red de pantalla/cátodo, acoplo 0.02 µF y malla
   10 M/3.3 M resuelta en el reposo, triodo de carga dividida 47 kΩ/47 kΩ por
   bisección monótona, y salidas por 0.05 µF hacia las rejillas de la 7027A
   (`process_grids`). Aproximación declarada: el acoplo pentodo→triodo en AC se
   modela como paso alto de un polo (el lazo completo llega con P1.6).
5. Fuente dinámica + NFB (estados acoplados, estabilidad).
6. OT no ideal + altavoz reactivo + gabinete (solo con evidencia).

Estado del Hito B: la red de tono se contrasta contra una referencia numpy independiente
(775 puntos, tolerancia 1e-6 dB / 1e-4°) y la cadena completa es invariante al tamaño de
bloque. El sobremuestreo efectivo y la latencia llegan con las etapas no lineales.

## Métodos numéricos

- MNA para redes lineales; TPT/trapezoidal para elementos reactivos.
- Newton-Raphson amortiguado para no lineales, con límite de iteraciones y cota de pasos.
- Interpolación monotónica de tablas de válvulas cuando existan.
- Sobremuestreo MVP 4× (conmutable 1×), 8× opcional si aporta ventaja medida; latencia
  compensada y reportada al host.

## Reglas del callback de audio

Prohibido: heap, disco, red, logging, mutex, hilos, panic, FFT no preasignada.
Obligatorio: scratch preasignado en `initialize`, estado por canal, `reset()` coherente,
smoothing 5–30 ms, soporte de bloques variables (incluido tamaño 0/1) y de cambios de
sample rate/buffer, sin NaN/Inf.

## API del núcleo (Hito A)

```rust
pub struct B18nEngine { /* ... */ }
pub struct ProcessSpec { pub sample_rate_hz: f64, pub max_block_frames: usize, pub oversampling: u8 }
pub struct RuntimeControls { /* mandos y trims */ }

impl B18nEngine {
    pub fn new(circuit: CircuitDefinition, spec: ProcessSpec) -> Result<Self, EngineError>;
    pub fn set_controls(&mut self, controls: RuntimeControls);
    pub fn reset(&mut self);
    pub fn process_mono(&mut self, input: &[f32], output: &mut [f32]);
    pub fn latency_samples(&self) -> u32;
}
```

En el Hito A `process_mono` es un passthrough determinista; las etapas se sustituyen en los
hitos B–D sin cambiar la API.
