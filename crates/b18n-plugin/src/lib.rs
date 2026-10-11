//! Adaptador VST3 (NIH-plug) del B-18N/B-15ND, revisión de esquema `1967-05`.
//!
//! El plugin no contiene DSP: carga el JSON canónico embebido con su errata y
//! calibración al inicializar y delega en [`B18nProcessor`] (un motor por
//! canal). El callback de audio no asigna memoria, no bloquea, no registra y
//! no usa `unwrap`/`panic`.
//!
//! Versión de algoritmo audible: la misma que reporta `b18n-render`
//! (`0.1.0-nonlinear-triodes-power-7199-inverter-nfb-4x`). Si cambia el DSP,
//! subir ambas (regla 8 del handoff).

#![deny(unsafe_op_in_unsafe_fn)]

pub mod params;
pub mod processor;

use std::fmt;
use std::sync::Arc;

use nih_plug::prelude::*;

use params::B18nParams;
use processor::B18nProcessor;

/// Identificador VST3 de 16 bytes, propio y estable (no reutilizar de terceros).
const VST3_CLASS_ID: [u8; 16] = *b"B18N1967VST3DSP!";

/// Versión de algoritmo (debe coincidir con `b18n-render`).
pub const ALGORITHM_VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    "-nonlinear-triodes-power-7199-inverter-nfb-25k-comp-4x"
);

/// Error de inicialización del plugin (fuera del hilo de audio).
#[derive(Debug)]
enum InitError {
    Config(b18n_config::ConfigError),
    Engine(b18n_dsp::EngineError),
}

impl fmt::Display for InitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(source) => write!(f, "modelo histórico: {source}"),
            Self::Engine(source) => write!(f, "motor DSP: {source}"),
        }
    }
}

impl std::error::Error for InitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Config(source) => Some(source),
            Self::Engine(source) => Some(source),
        }
    }
}

impl From<b18n_config::ConfigError> for InitError {
    fn from(source: b18n_config::ConfigError) -> Self {
        Self::Config(source)
    }
}

impl From<b18n_dsp::EngineError> for InitError {
    fn from(source: b18n_dsp::EngineError) -> Self {
        Self::Engine(source)
    }
}

/// Plugin B-18N '67.
pub struct B18nPlugin {
    params: Arc<B18nParams>,
    processor: Option<B18nProcessor>,
    /// Última latencia reportada al host (en muestras de la tasa base).
    reported_latency: Option<u32>,
}

impl Default for B18nPlugin {
    fn default() -> Self {
        Self {
            params: Arc::new(B18nParams::default()),
            processor: None,
            reported_latency: None,
        }
    }
}

impl B18nPlugin {
    /// Carga el modelo, construye el procesador y registra la trazabilidad.
    fn build_processor(&mut self, buffer_config: &BufferConfig) -> Result<(), InitError> {
        let circuit = b18n_config::load_reference()?;
        let spec = b18n_dsp::ProcessSpec {
            sample_rate_hz: f64::from(buffer_config.sample_rate),
            max_block_frames: buffer_config.max_buffer_size.max(1) as usize,
            // El adaptador construye 1× y 4×; este campo no selecciona factor.
            oversampling: 1,
        };
        let processor = B18nProcessor::new(&circuit, spec)?;

        let effective = b18n_config::apply_embedded_errata(b18n_config::EMBEDDED_REFERENCE_JSON)?;
        store(
            &self.params.model_sha256,
            b18n_config::EXPECTED_REFERENCE_SHA256,
        );
        store(
            &self.params.errata_sha256,
            &b18n_config::sha256_hex(b18n_config::EMBEDDED_ERRATA_JSON.as_bytes()),
        );
        store(
            &self.params.effective_sha256,
            &b18n_config::sha256_hex(effective.as_bytes()),
        );
        store(
            &self.params.schematic_revision,
            &circuit.identity.schematic_revision,
        );
        store(&self.params.algorithm_version, ALGORITHM_VERSION);

        self.processor = Some(processor);
        self.reported_latency = None;
        Ok(())
    }
}

/// Guarda un valor de trazabilidad en un campo persistente (no hilo de audio).
fn store(field: &std::sync::Mutex<String>, value: &str) {
    if let Ok(mut guard) = field.lock() {
        *guard = value.to_owned();
    }
}

impl Plugin for B18nPlugin {
    const NAME: &'static str = "B18N '67";
    const VENDOR: &'static str = "B18N 1967";
    const URL: &'static str = "";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(2),
            main_output_channels: NonZeroU32::new(2),
            aux_input_ports: &[],
            aux_output_ports: &[],
            names: PortNames::const_default(),
        },
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(1),
            main_output_channels: NonZeroU32::new(1),
            ..AudioIOLayout::const_default()
        },
    ];

    const MIDI_INPUT: MidiConfig = MidiConfig::None;
    const MIDI_OUTPUT: MidiConfig = MidiConfig::None;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        context: &mut impl InitContext<Self>,
    ) -> bool {
        match self.build_processor(buffer_config) {
            Ok(()) => {
                let latency = self.processor.as_ref().map_or(0, |processor| {
                    processor.latency_samples(self.params.oversampling())
                });
                context.set_latency_samples(latency);
                self.reported_latency = Some(latency);
                true
            }
            Err(error) => {
                nih_error!("B18N '67: inicialización fallida: {error}");
                self.processor = None;
                self.reported_latency = None;
                false
            }
        }
    }

    fn reset(&mut self) {
        if let Some(processor) = self.processor.as_mut() {
            processor.reset();
        }
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let Some(processor) = self.processor.as_mut() else {
            return ProcessStatus::Error("B18N '67: motor no inicializado");
        };
        let frames = buffer.samples();
        let channels = buffer.channels();
        if frames == 0 || channels == 0 {
            return ProcessStatus::Normal;
        }

        let controls = self.params.runtime_controls();
        let oversampling = self.params.oversampling();
        let bypass = self.params.bypassed();

        let latency = processor.latency_samples(oversampling);
        if self.reported_latency != Some(latency) {
            context.set_latency_samples(latency);
            self.reported_latency = Some(latency);
        }

        processor.process(buffer.as_slice(), controls, oversampling, bypass);
        ProcessStatus::Normal
    }

    fn deactivate(&mut self) {
        self.processor = None;
        self.reported_latency = None;
    }
}

impl Vst3Plugin for B18nPlugin {
    const VST3_CLASS_ID: [u8; 16] = VST3_CLASS_ID;
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Distortion];
}

nih_export_vst3!(B18nPlugin);
