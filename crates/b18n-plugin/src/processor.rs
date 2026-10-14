//! Adaptador del motor para el host: estéreo = dos motores independientes.
//!
//! Los motores de los tres factores de sobremuestreo (1×, 2× y 4×) se construyen al
//! inicializar, de modo que `process()` nunca asigna memoria al cambiar de
//! factor. El bypass del host usa una línea de retardo que compensa la latencia
//! del camino procesado (FIR del sobremuestreador) con un crossfade corto.

use b18n_dsp::utilities::smoothing::SmoothedValue;
use b18n_dsp::{B18nEngine, CircuitDefinition, EngineError, ProcessSpec, RuntimeControls};

/// Suavizado del crossfade de bypass (guía §6.4: 5–30 ms).
const BYPASS_SMOOTHING_MS: f32 = 15.0;

/// Capacidad de la línea de retardo seco: cubre la latencia 4× (47 muestras).
const MAX_LATENCY_SAMPLES: usize = 128;

/// Canales máximos: un motor por canal (layouts 1→1 y 2→2).
pub const MAX_CHANNELS: usize = 2;

/// Motor por canal con factores de sobremuestreo preconstruidos.
pub struct B18nProcessor {
    engines_1x: [B18nEngine; MAX_CHANNELS],
    engines_2x: [B18nEngine; MAX_CHANNELS],
    engines_4x: [B18nEngine; MAX_CHANNELS],
    latency_1x: u32,
    latency_2x: u32,
    latency_4x: u32,
    dry_delay: [Vec<f32>; MAX_CHANNELS],
    dry_write: [usize; MAX_CHANNELS],
    bypass_mix: [SmoothedValue; MAX_CHANNELS],
}

impl B18nProcessor {
    /// Construye los motores de 1×, 2× y 4× para los dos canales.
    ///
    /// `spec.oversampling` se ignora: el adaptador construye los tres factores.
    pub fn new(circuit: &CircuitDefinition, spec: ProcessSpec) -> Result<Self, EngineError> {
        let base = ProcessSpec {
            oversampling: 1,
            ..spec
        };
        let two = ProcessSpec {
            oversampling: 2,
            ..spec
        };
        let four = ProcessSpec {
            oversampling: 4,
            ..spec
        };
        let engines_1x = [
            B18nEngine::new(circuit.clone(), base)?,
            B18nEngine::new(circuit.clone(), base)?,
        ];
        let engines_2x = [
            B18nEngine::new(circuit.clone(), two)?,
            B18nEngine::new(circuit.clone(), two)?,
        ];
        let engines_4x = [
            B18nEngine::new(circuit.clone(), four)?,
            B18nEngine::new(circuit.clone(), four)?,
        ];
        let latency_1x = engines_1x[0].latency_samples();
        let latency_2x = engines_2x[0].latency_samples();
        let latency_4x = engines_4x[0].latency_samples();
        assert!(
            latency_4x as usize <= MAX_LATENCY_SAMPLES,
            "latencia 4× fuera de la línea de retardo"
        );

        let rate = spec.sample_rate_hz as f32;
        Ok(Self {
            engines_1x,
            engines_2x,
            engines_4x,
            latency_1x,
            latency_2x,
            latency_4x,
            dry_delay: [
                vec![0.0; MAX_LATENCY_SAMPLES],
                vec![0.0; MAX_LATENCY_SAMPLES],
            ],
            dry_write: [0; MAX_CHANNELS],
            bypass_mix: [
                SmoothedValue::new(0.0, rate, BYPASS_SMOOTHING_MS),
                SmoothedValue::new(0.0, rate, BYPASS_SMOOTHING_MS),
            ],
        })
    }

    /// Latencia en muestras de la tasa base para el factor pedido.
    #[must_use]
    pub fn latency_samples(&self, oversampling: u8) -> u32 {
        if oversampling >= 4 {
            self.latency_4x
        } else if oversampling == 2 {
            self.latency_2x
        } else {
            self.latency_1x
        }
    }

    /// Fallos de convergencia acumulados por los motores del factor activo
    /// (diagnóstico; el motor conserva el último estado válido en cada fallo).
    #[must_use]
    pub fn convergence_failures(&self, oversampling: u8) -> u64 {
        if oversampling >= 4 {
            self.engines_4x[0].convergence_failures() + self.engines_4x[1].convergence_failures()
        } else if oversampling == 2 {
            self.engines_2x[0].convergence_failures() + self.engines_2x[1].convergence_failures()
        } else {
            self.engines_1x[0].convergence_failures() + self.engines_1x[1].convergence_failures()
        }
    }

    /// Procesa in-place hasta dos canales. Sin asignaciones ni locks.
    ///
    /// Los objetivos de los mandos se aplican a los tres factores para que el
    /// cambio de sobremuestreo no arrastre un estado de suavizado obsoleto.
    pub fn process(
        &mut self,
        channels: &mut [&mut [f32]],
        controls: RuntimeControls,
        oversampling: u8,
        bypass: bool,
    ) {
        let latency = self.latency_samples(oversampling) as usize;
        let target = if bypass { 1.0 } else { 0.0 };
        let four = oversampling >= 4;

        for (index, channel) in channels.iter_mut().enumerate().take(MAX_CHANNELS) {
            self.engines_1x[index].set_controls(controls);
            self.engines_2x[index].set_controls(controls);
            self.engines_4x[index].set_controls(controls);
            let engine = if four {
                &mut self.engines_4x[index]
            } else if oversampling == 2 {
                &mut self.engines_2x[index]
            } else {
                &mut self.engines_1x[index]
            };

            let mix = &mut self.bypass_mix[index];
            mix.set_target(target);
            let delay = &mut self.dry_delay[index];
            let mut write = self.dry_write[index];

            for sample in channel.iter_mut() {
                let input = *sample;
                let processed = engine.process_sample(input);

                delay[write] = input;
                let read = (write + MAX_LATENCY_SAMPLES - latency) % MAX_LATENCY_SAMPLES;
                let dry = delay[read];
                write = (write + 1) % MAX_LATENCY_SAMPLES;

                let mix_value = mix.next_value();
                *sample = if mix_value <= 0.0 {
                    processed
                } else if mix_value >= 1.0 {
                    dry
                } else {
                    processed * (1.0 - mix_value) + dry * mix_value
                };
            }
            self.dry_write[index] = write;
        }
    }

    /// Restablece motores, retardo y suavizados (apto para el hilo de audio).
    pub fn reset(&mut self) {
        for engine in self
            .engines_1x
            .iter_mut()
            .chain(self.engines_2x.iter_mut())
            .chain(self.engines_4x.iter_mut())
        {
            engine.reset();
        }
        for delay in &mut self.dry_delay {
            delay.fill(0.0);
        }
        self.dry_write = [0; MAX_CHANNELS];
        for mix in &mut self.bypass_mix {
            mix.snap();
        }
    }
}
