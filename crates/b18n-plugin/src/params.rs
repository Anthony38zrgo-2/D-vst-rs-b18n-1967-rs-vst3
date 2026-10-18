//! Parámetros del plugin: IDs persistentes y traducción a `RuntimeControls`.
//!
//! Los IDs (`#[id = "..."]`) y los identificadores de variante de los enums
//! (`#[id = "..."]` en cada variante) son parte del contrato con los hosts:
//! no se renombran ni se reordenan sin migración de presets. Los parámetros
//! modernos (trims, bypass, oversampling) van separados de los históricos.

use std::sync::Mutex;

use b18n_dsp::{InputChannel, RuntimeControls, SpeakerLoadSelection, SpeakerProfile};
use nih_plug::prelude::*;

/// Entrada histórica seleccionada según los conectores del esquema.
#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChannelParam {
    #[id = "bass"]
    #[default]
    Bass,
    #[id = "guitar"]
    Guitar,
    #[id = "instrument"]
    Instrument,
}

impl From<ChannelParam> for InputChannel {
    fn from(value: ChannelParam) -> Self {
        match value {
            ChannelParam::Bass => InputChannel::Bass,
            ChannelParam::Guitar => InputChannel::Guitar,
            ChannelParam::Instrument => InputChannel::Instrument,
        }
    }
}

/// Factor de sobremuestreo implementado por el motor (1×/2×/4×).
///
/// 8× no se expone: `Oversampler` no lo implementa y no se inventa.
#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OversamplingFactor {
    #[id = "1x"]
    #[name = "1x"]
    One,
    #[id = "2x"]
    #[name = "2x"]
    Two,
    #[id = "4x"]
    #[name = "4x"]
    #[default]
    Four,
}

impl OversamplingFactor {
    #[must_use]
    pub fn factor(self) -> u8 {
        match self {
            Self::One => 1,
            Self::Two => 2,
            Self::Four => 4,
        }
    }
}

/// Approximate cabinet voicings; Direct preserves the original amp output.
#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpeakerParam {
    #[id = "direct"]
    #[name = "Direct"]
    #[default]
    Direct,
    #[id = "18inch"]
    #[name = "18 inch (approx.)"]
    Eighteen,
    #[id = "15inch"]
    #[name = "15 inch (approx.)"]
    Fifteen,
}
impl From<SpeakerParam> for SpeakerProfile {
    fn from(value: SpeakerParam) -> Self {
        match value {
            SpeakerParam::Direct => Self::Direct,
            SpeakerParam::Eighteen => Self::Eighteen,
            SpeakerParam::Fifteen => Self::Fifteen,
        }
    }
}

/// Panel moderno (no histórico): solo opciones de calidad.
#[derive(Params)]
pub struct AdvancedParams {
    /// Calidad de sobremuestreo; cambiarla reconfigura la latencia reportada.
    #[id = "oversampling"]
    pub oversampling: EnumParam<OversamplingFactor>,

    /// Carga eléctrica del secundario del OT (B18-015; moderno, no histórico).
    /// El equivalente RLC es `estimated` (overlay de calibración), no medido.
    #[id = "load_model"]
    pub load_model: EnumParam<LoadModelParam>,
}

impl Default for AdvancedParams {
    fn default() -> Self {
        Self {
            oversampling: EnumParam::new("Oversampling", OversamplingFactor::default()),
            load_model: EnumParam::new("Load Model", LoadModelParam::default()),
        }
    }
}

/// Selección del modelo eléctrico de carga del secundario (B18-015).
#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LoadModelParam {
    /// Resistencia nominal del JSON histórico (8 Ω); fallback declarado.
    #[id = "nominal"]
    #[name = "Nominal resistance"]
    #[default]
    Nominal,
    /// Equivalente RLC estimado del altavoz de 18" (overlay, `estimated`).
    #[id = "rlc_estimated"]
    #[name = "Speaker RLC (estimated)"]
    RlcEstimated,
}

impl From<LoadModelParam> for SpeakerLoadSelection {
    fn from(value: LoadModelParam) -> Self {
        match value {
            LoadModelParam::Nominal => Self::NominalResistance,
            LoadModelParam::RlcEstimated => Self::EstimatedLumped,
        }
    }
}

/// Parámetros del B-18N '67.
#[derive(Params)]
pub struct B18nParams {
    #[persist = "editor-state-layout-v3"]
    pub editor_state: std::sync::Arc<nih_plug_egui::EguiState>,
    #[persist = "ui-language"]
    pub ui_language: Mutex<String>,
    /// Conector de entrada (Bass/Guitar comparten el canal 1; Instrument el 2).
    #[id = "channel"]
    pub channel: EnumParam<ChannelParam>,

    #[id = "speaker"]
    pub speaker: EnumParam<SpeakerParam>,

    /// Adaptación moderna DAW↔voltios (no histórica).
    #[id = "input_trim"]
    pub input_trim_db: FloatParam,

    /// Canal 1: volumen (potenciómetro 1 MΩ log, posición 0..1).
    #[id = "volume_1"]
    pub volume_1: FloatParam,
    /// Canal 1: graves (1 MΩ lineal, posición 0..1).
    #[id = "bass_1"]
    pub bass_1: FloatParam,
    /// Canal 1: agudos (1 MΩ lineal, posición 0..1).
    #[id = "treble_1"]
    pub treble_1: FloatParam,

    /// Canal 2: volumen (1 MΩ log, posición 0..1).
    #[id = "volume_2"]
    pub volume_2: FloatParam,
    /// Canal 2: graves (1 MΩ lineal, posición 0..1).
    #[id = "bass_2"]
    pub bass_2: FloatParam,
    /// Canal 2: agudos (1 MΩ lineal, posición 0..1).
    #[id = "treble_2"]
    pub treble_2: FloatParam,

    /// Normalización moderna de salida (no histórica).
    #[id = "output_trim"]
    pub output_trim_db: FloatParam,

    /// Bypass del host con compensación de latencia en el motor.
    #[id = "bypass"]
    pub bypass: BoolParam,

    #[nested(group = "Advanced")]
    pub advanced: AdvancedParams,

    /// Huellas del modelo con el que se guardó el preset (trazabilidad).
    #[persist = "model_sha256"]
    pub model_sha256: Mutex<String>,
    #[persist = "errata_sha256"]
    pub errata_sha256: Mutex<String>,
    #[persist = "effective_sha256"]
    pub effective_sha256: Mutex<String>,
    #[persist = "schematic_revision"]
    pub schematic_revision: Mutex<String>,
    #[persist = "algorithm_version"]
    pub algorithm_version: Mutex<String>,
}

fn position(name: &str, default: f32) -> FloatParam {
    FloatParam::new(name, default, FloatRange::Linear { min: 0.0, max: 1.0 })
        .with_value_to_string(formatters::v2s_f32_percentage(0))
        .with_string_to_value(formatters::s2v_f32_percentage())
}

fn trim(name: &str, default_db: f32) -> FloatParam {
    FloatParam::new(
        name,
        default_db,
        FloatRange::Linear {
            min: b18n_dsp::controls::TRIM_MIN_DB,
            max: b18n_dsp::controls::TRIM_MAX_DB,
        },
    )
    .with_unit(" dB")
    .with_value_to_string(formatters::v2s_f32_rounded(1))
}

/// Margen por defecto del trim de salida (no histórico).
///
/// Medición Ola 8: una instancia nueva (volumen/tone a 0,5, trims a este
/// valor) con senos de graves a escala completa del host producía un pico de
/// 5,57 V en la etapa y rebasaba la escala digital del host (`daw_levels`).
/// El valor se recalibró en B18-003 (corrección del doble atenuador del tono,
/// que sube la cadena ~3,4 dB a 110/220 Hz): con **−20,0 dB** el pico vuelve a
/// ≈0,82 (−1,7 dBFS); el usuario puede subir el trim si el material es más
/// bajo. Adaptación moderna de nivel, no una característica histórica.
const OUTPUT_TRIM_DEFAULT_DB: f32 = -20.0;

impl Default for B18nParams {
    fn default() -> Self {
        Self {
            editor_state: nih_plug_egui::EguiState::from_size(
                crate::editor::WIDTH,
                crate::editor::HEIGHT,
            ),
            ui_language: Mutex::new("es".to_string()),
            channel: EnumParam::new("Channel", ChannelParam::default()),
            speaker: EnumParam::new("Speaker", SpeakerParam::default()),
            input_trim_db: trim("Input Trim", 0.0),
            volume_1: position("Volume 1", 0.5),
            bass_1: position("Bass 1", 0.5),
            treble_1: position("Treble 1", 0.5),
            volume_2: position("Volume 2", 0.5),
            bass_2: position("Bass 2", 0.5),
            treble_2: position("Treble 2", 0.5),
            output_trim_db: trim("Output Trim", OUTPUT_TRIM_DEFAULT_DB),
            bypass: BoolParam::new("Bypass", false)
                .make_bypass()
                .with_value_to_string(formatters::v2s_bool_bypass())
                .with_string_to_value(formatters::s2v_bool_bypass()),
            advanced: AdvancedParams::default(),
            model_sha256: Mutex::new(String::new()),
            errata_sha256: Mutex::new(String::new()),
            effective_sha256: Mutex::new(String::new()),
            schematic_revision: Mutex::new(String::new()),
            algorithm_version: Mutex::new(String::new()),
        }
    }
}

impl B18nParams {
    /// Posiciones y trims actuales como controles del motor (sin locks).
    #[must_use]
    pub fn runtime_controls(&self) -> RuntimeControls {
        RuntimeControls {
            input_trim_db: self.input_trim_db.value(),
            channel: self.channel.value().into(),
            volume_1: self.volume_1.value(),
            bass_1: self.bass_1.value(),
            treble_1: self.treble_1.value(),
            volume_2: self.volume_2.value(),
            bass_2: self.bass_2.value(),
            treble_2: self.treble_2.value(),
            output_trim_db: self.output_trim_db.value(),
            // No measured IR; acoustic profiles are separate from the
            // electrical load (B18-015).
            cabinet_enabled: false,
            speaker_profile: self.speaker.value().into(),
            speaker_load: self.advanced.load_model.value().into(),
        }
    }

    /// Factor de sobremuestreo activo.
    #[must_use]
    pub fn oversampling(&self) -> u8 {
        self.advanced.oversampling.value().factor()
    }

    /// Estado de bypass solicitado por el host.
    #[must_use]
    pub fn bypassed(&self) -> bool {
        self.bypass.value()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn parameter_ids_are_frozen_and_unique() {
        let params = B18nParams::default();
        let map = params.param_map();
        let ids: BTreeSet<String> = map.iter().map(|(id, _, _)| id.clone()).collect();
        let expected: BTreeSet<String> = [
            "bass_1",
            "bass_2",
            "bypass",
            "channel",
            "input_trim",
            "load_model",
            "output_trim",
            "oversampling",
            "speaker",
            "treble_1",
            "treble_2",
            "volume_1",
            "volume_2",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();
        assert_eq!(ids, expected, "los IDs de automatización son un contrato");
        assert_eq!(map.len(), 13, "sin IDs duplicados");
    }

    #[test]
    fn defaults_match_the_historical_controls() {
        let params = B18nParams::default();
        let controls = params.runtime_controls();
        assert_eq!(controls.channel, InputChannel::Bass);
        assert!((controls.volume_1 - 0.5).abs() < 1e-6);
        assert!((controls.bass_2 - 0.5).abs() < 1e-6);
        assert_eq!(params.oversampling(), 4);
        assert!(!params.bypassed());
        // El trim de salida por defecto normaliza la instancia nueva dentro
        // de la escala del host (no histórico; ver `OUTPUT_TRIM_DEFAULT_DB`).
        assert!((controls.input_trim_db - 0.0).abs() < 1e-6);
        assert!((controls.output_trim_db - OUTPUT_TRIM_DEFAULT_DB).abs() < 1e-6);
    }

    #[test]
    fn oversampling_ids_are_stable() {
        assert_eq!(OversamplingFactor::One.to_index(), 0);
        assert_eq!(OversamplingFactor::Four.to_index(), 2);
        assert_eq!(OversamplingFactor::ids(), Some(&["1x", "2x", "4x"][..]));
        assert_eq!(
            ChannelParam::ids(),
            Some(&["bass", "guitar", "instrument"][..])
        );
    }
}
