//! Motor del B-18N. Hito C (parte 1): triodos no lineales + sobremuestreo.
//!
//! Cadena por sample: entrada → 6SL7#1 (Newton) → acoplo 0.022 µF → volumen →
//! 6SL7#2 (Newton) → acoplo 0.022 µF → tono (MNA/TPT) → mezcla → inversor
//! ideal → OT ideal con DCR → 8 Ω. El camino no lineal puede correr a 4×.
//!
//! El punto de reposo se resuelve con Newton al inicializar y los capacitores
//! de acoplo se presetan a su tensión congruente: el motor arranca en reposo,
//! sin insertar tensiones nominales a mano. Si Newton falla durante el audio
//! se conserva el último estado válido y se incrementa un contador de fallos.

use std::f64::consts::PI;
use std::fmt;

use crate::audio_context::ProcessSpec;
use crate::circuit::CircuitDefinition;
use crate::controls::{volume_fraction, InputChannel, RuntimeControls};
use crate::numerics::oversampling::Oversampler;
use crate::stages::channel_mixer::ChannelMixer;
use crate::stages::input::InputStage;
use crate::stages::phase_splitter::{PhaseInverter7199, PhaseInverterOperatingPoint};
use crate::stages::power_7027a::{PowerStageConfig, PushPullStage};
use crate::stages::speaker::SpeakerCabinet;
use crate::stages::tone_stack::{
    ToneStackControls, ToneStackDesign, ToneStackError, ToneStackProcessor,
};
use crate::stages::triode_6sl7::{
    triode_output_impedance_ohms, NonlinearTriodeStage, TriodeStageConfig,
};
use crate::stages::tube_models::{KorenPentodeModel, KorenTriodeModel};
use crate::state::{EngineState, OperatingPoint, StageOperatingPoint};
use crate::utilities::db::db_to_gain;
use crate::utilities::filters::{OnePoleHighPass, OnePoleLowPass};
use crate::utilities::smoothing::SmoothedValue;

/// Compensación del lazo de NFB (aproximación declarada): el modelo no
/// incluye los parásitos de alta frecuencia del tubo ni del transformador,
/// que en el amplificador real limitan la ganancia de lazo. Se modela con un
/// polo en el camino de realimentación; el margen se mide en `nfb_stability`.
///
/// **B18-002:** el polo se construye con la **tasa de proceso efectiva**
/// (`sample_rate_hz × oversampling`), la misma a la que `LinearChain::process_sample`
/// hace avanzar la realimentación. La construcción anterior usaba la tasa base
/// mientras el filtro integraba `oversampling` veces por muestra del host, de
/// modo que el corte efectivo escalaba con el factor de sobremuestreo
/// (≈5 kHz a 2× y ≈10 kHz a 4×, en lugar de 2,5 kHz).
///
/// Además, la discretización de `OnePoleLowPass` pasó de Euler implícito (polo
/// apareado) a **TPT/bilineal con prewarp**: el corte físico es exacto a
/// cualquier tasa y la fase en HF se aproxima al RC analógico. El impacto de
/// ambos pasos se midió por separado con la comparación 1×/2×/4× de
/// `nfb_rate_invariance.rs` (dispersión final ≤0,64 dB/8,1° hasta 2,5 kHz;
/// antes 3,5 dB/20° a 48 kHz).
///
/// El valor de 2,5 kHz sigue siendo **provisional**: no es una característica
/// verificada del B-18N. Con el solver exacto del inversor 7199 el lazo
/// idealizado se acercaba a inestabilidad a Nyquist; 2,5 kHz da margen en
/// 44,1–192 kHz y mantiene la reducción de banda media (≈0,14×).
/// Cambio audible: `ALGORITHM_VERSION` actualizada.
const FEEDBACK_COMPENSATION_CUTOFF_HZ: f64 = 2_500.0;

/// Tiempo de suavizado de mandos (guía §6.4: 5–30 ms). Es un tiempo **físico**:
/// los suavizados avanzan una vez por muestra de la tasa de proceso, por lo que
/// no se multiplica por el factor de sobremuestreo (B18-004).
const CONTROL_SMOOTHING_MS: f32 = 15.0;

/// Serie de la red de entrada documentada (120 kΩ por jack).
const INPUT_SERIES_OHMS: f64 = 120_000.0;

/// Error de configuración del motor.
#[derive(Debug, Clone, PartialEq)]
pub enum EngineError {
    InvalidSampleRate {
        hz: f64,
    },
    InvalidBlockSize {
        frames: usize,
    },
    UnsupportedOversampling {
        factor: u8,
    },
    ToneStack(ToneStackError),
    /// Falta el modelo de válvula en la calibración (no se inventa).
    MissingTubeModel,
    /// Faltan datos trazados del inversor 7199 (errata no aplicada).
    MissingInverterData,
    /// El reposo no converge: no se activa audio con un punto inválido.
    OperatingPointNotConverged,
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSampleRate { hz } => write!(f, "sample rate no válido: {hz} Hz"),
            Self::InvalidBlockSize { frames } => {
                write!(f, "tamaño máximo de bloque no válido: {frames} frames")
            }
            Self::UnsupportedOversampling { factor } => {
                write!(f, "factor de sobremuestreo no soportado: {factor}×")
            }
            Self::ToneStack(source) => write!(f, "{source}"),
            Self::MissingTubeModel => write!(
                f,
                "falta el modelo Koren del 6SL7 en la calibración (no se inventan curvas)"
            ),
            Self::MissingInverterData => write!(
                f,
                "faltan datos trazados del inversor 7199 (150 kΩ/10 M/3.3 M y carril del triodo)"
            ),
            Self::OperatingPointNotConverged => {
                write!(f, "el punto de reposo no converge; no se activa el audio")
            }
        }
    }
}

impl std::error::Error for EngineError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ToneStack(source) => Some(source),
            _ => None,
        }
    }
}

impl From<ToneStackError> for EngineError {
    fn from(source: ToneStackError) -> Self {
        Self::ToneStack(source)
    }
}

/// Motor de procesamiento mono. En estéreo se usan dos instancias.
pub struct B18nEngine {
    circuit: CircuitDefinition,
    spec: ProcessSpec,
    controls: RuntimeControls,
    /// `false` hasta la primera `set_controls` (arranque directo a los
    /// objetivos, sin rampa; véase `set_controls`).
    controls_synchronized: bool,
    state: EngineState,
    chain: LinearChain,
    settled_chain: LinearChain,
    oversampler: Oversampler,
    settled_oversampler: Oversampler,
    speaker: SpeakerCabinet,
}

impl B18nEngine {
    /// Valida la especificación, resuelve el reposo y prepara el motor.
    pub fn new(circuit: CircuitDefinition, spec: ProcessSpec) -> Result<Self, EngineError> {
        spec.validate()?;
        let oversampler =
            Oversampler::new(spec.oversampling).ok_or(EngineError::UnsupportedOversampling {
                factor: spec.oversampling,
            })?;
        let chain = LinearChain::new(&circuit, &spec)?;

        let state = EngineState {
            operating_point_solved: true,
            operating_point: Some(chain.operating_point()),
            convergence_failures: 0,
        };
        let settled_chain = chain.clone();
        let settled_oversampler = oversampler.clone();

        Ok(Self {
            circuit,
            spec,
            controls: RuntimeControls::default(),
            controls_synchronized: false,
            state,
            chain,
            settled_chain,
            oversampler,
            settled_oversampler,
            speaker: SpeakerCabinet::new(spec.sample_rate_hz),
        })
    }

    /// Actualiza los objetivos de los mandos (suavizados por sample).
    ///
    /// La **primera** llamada de una instancia recién construida salta directo
    /// a los objetivos: sin rampa de 15 ms a ganancia distinta de la final en
    /// el primer bloque (la respuesta impulso del motor directo y la del
    /// adaptador quedan idénticas entre sí y dentro de la escala de la
    /// instancia por defecto).
    pub fn set_controls(&mut self, controls: RuntimeControls) {
        self.controls = controls.sanitized();
        self.speaker.set_profile(self.controls.speaker_profile);
        self.chain.set_targets(&self.controls);
        if !self.controls_synchronized {
            self.controls_synchronized = true;
            sync_chain_smoothing(&mut self.chain);
            self.speaker.snap();
        }
    }

    /// Restablece al estado de reposo resuelto al inicializar.
    ///
    /// Sin asignaciones: los buffers del MNA y del sobremuestreador se
    /// reutilizan (`restore_from`); es apto para el hilo de audio. Los mandos
    /// del usuario continúan saliendo directo a sus objetivos (sin rampa
    /// desde los valores de construcción tras un reset del host).
    pub fn reset(&mut self) {
        self.chain.restore_from(&self.settled_chain);
        self.oversampler.restore_from(&self.settled_oversampler);
        self.state.reset();
        self.speaker.reset();
        if self.controls_synchronized {
            self.chain.set_targets(&self.controls);
            sync_chain_smoothing(&mut self.chain);
            self.speaker.set_profile(self.controls.speaker_profile);
            self.speaker.snap();
        }
    }

    #[must_use]
    pub fn spec(&self) -> ProcessSpec {
        self.spec
    }

    #[must_use]
    pub fn circuit(&self) -> &CircuitDefinition {
        &self.circuit
    }

    #[must_use]
    pub fn controls(&self) -> RuntimeControls {
        self.controls
    }

    #[must_use]
    pub fn state(&self) -> &EngineState {
        &self.state
    }

    /// Fallos de convergencia acumulados (bandera de diagnóstico).
    #[must_use]
    pub fn convergence_failures(&self) -> u64 {
        self.chain.convergence_failures()
    }

    /// Desglose de fallos por etapa (diagnóstico): triodos, inversor 7199 y
    /// etapa de potencia.
    #[must_use]
    pub fn failure_breakdown(&self) -> (u64, u64, u64) {
        (
            self.chain.channel_1.triode_1.convergence_failures()
                + self.chain.channel_1.triode_2.convergence_failures()
                + self.chain.channel_2.triode_1.convergence_failures()
                + self.chain.channel_2.triode_2.convergence_failures(),
            self.chain.inverter.convergence_failures(),
            self.chain.power.convergence_failures(),
        )
    }

    /// Latencia en muestras de la tasa base (filtros de sobremuestreo).
    #[must_use]
    pub fn latency_samples(&self) -> u32 {
        self.oversampler.latency_samples()
    }

    /// Procesa un bloque mono `input` hacia `output`.
    pub fn process_mono(&mut self, input: &[f32], output: &mut [f32]) {
        let frames = input.len().min(output.len());
        for index in 0..frames {
            output[index] = self.process_sample(input[index]);
        }
        for sample in &mut output[frames..] {
            *sample = 0.0;
        }
        self.state.convergence_failures = self.chain.convergence_failures();
    }

    /// Procesa un sample base (aplica sobremuestreo si está configurado).
    pub fn process_sample(&mut self, sample: f32) -> f32 {
        let chain = &mut self.chain;
        let oversampler = &mut self.oversampler;
        let output = oversampler.process(sample, |value| chain.process_sample(value));
        self.speaker.process(output)
    }
}

/// Salto directo al objetivo en los suavizados de la cadena (instancia nueva
/// y `reset`: continuidad de los mandos sin rampa desde los valores de
/// construcción).
fn sync_chain_smoothing(chain: &mut LinearChain) {
    chain.input_trim_db.snap();
    chain.output_trim_db.snap();
    chain.channel_1.snap_smoothing();
    chain.channel_2.snap_smoothing();
}

/// Cache a control mapping without changing its smoothing trajectory.
#[derive(Clone, Copy)]
struct MappedControl {
    position: f32,
    value: f64,
}
impl MappedControl {
    fn new() -> Self {
        Self {
            position: f32::NAN,
            value: 0.0,
        }
    }
    fn get(&mut self, position: f32, map: impl FnOnce(f32) -> f64) -> f64 {
        if self.position != position {
            self.position = position;
            self.value = map(position);
        }
        self.value
    }
}

/// Cadena lineal/no lineal completa con un previo por canal.
#[derive(Clone)]
struct LinearChain {
    input: InputStage,
    channel_1: ChannelChain,
    channel_2: ChannelChain,
    mixer: ChannelMixer,
    inverter: PhaseInverter7199,
    power: PushPullStage,
    input_trim_db: SmoothedValue,
    output_trim_db: SmoothedValue,
    input_gain: MappedControl,
    output_gain: MappedControl,
    channel: InputChannel,
    /// Realimentación negativa (separación explícita `z^-1`, aproximación
    /// declarada): la muestra de la toma de 16 Ω se usa en el sample siguiente
    /// de la tasa de proceso efectiva (no del host); el `z^-1` y el polo de
    /// compensación avanzan al mismo ritmo.
    feedback_enabled: bool,
    feedback_tap_gain: f64,
    feedback_polarity: f64,
    feedback_state: f64,
    feedback_lowpass: OnePoleLowPass,
}

impl LinearChain {
    fn new(circuit: &CircuitDefinition, spec: &ProcessSpec) -> Result<Self, EngineError> {
        let processing_rate = spec.sample_rate_hz * f64::from(spec.oversampling);
        let tone_design = ToneStackDesign::from_circuit(circuit)?;

        // Fugas de rejilla documentadas: CH1 = 5.6 MΩ, CH2 = 2.2 MΩ.
        let channel_1 = ChannelChain::new(circuit, &tone_design, processing_rate, 5_600_000.0)?;
        let channel_2 = ChannelChain::new(circuit, &tone_design, processing_rate, 2_200_000.0)?;

        let mixer = ChannelMixer::new(
            circuit.channel.channel_mixer_resistor_ohms,
            circuit.power_stage.inverter_to_grid_coupling_farads_each,
            processing_rate,
        );

        let inverter = PhaseInverter7199::from_circuit(circuit, processing_rate)
            .ok_or(EngineError::MissingInverterData)?;
        if !inverter.operating_point_solved() {
            return Err(EngineError::OperatingPointNotConverged);
        }

        let pentode = circuit
            .tube_models
            .koren_7027a
            .ok_or(EngineError::MissingTubeModel)?;
        let mut power = PushPullStage::new(
            KorenPentodeModel::new(pentode),
            PowerStageConfig {
                supply_v: circuit.power_supply.bplus_idle_vdc,
                screen_supply_v: circuit.power_stage.screen_voltage_v_nominal,
                bias_v: circuit.power_stage.control_grid_voltage_v_nominal,
                plate_to_plate_ohms: circuit
                    .output_transformer
                    .primary_plate_to_plate_ohms_nominal,
                dc_primary_half_ohms: circuit.output_transformer.dc_primary_winding_ohms / 2.0,
                dc_secondary_ohms: circuit.output_transformer.dc_secondary_8ohm_winding_ohms,
                turns_ratio: circuit.output_transformer.turns_ratio_primary_to_8ohm,
                load_ohms: circuit.speaker.nominal_load_ohms,
                grid_stopper_ohms: circuit.power_stage.grid_stopper_ohms_each,
                grid_return_ohms: circuit.power_stage.grid_return_ohms_each,
                // Impedancias de fuente reales del inversor (placa y cátodo).
                grid_source_upper_ohms: inverter.output_impedance_upper_ohms(),
                grid_source_lower_ohms: inverter.output_impedance_lower_ohms(),
                coupling_farads: circuit.power_stage.inverter_to_grid_coupling_farads_each,
                sample_rate_hz: processing_rate,
            },
        );
        if !power.solve_operating_point() {
            return Err(EngineError::OperatingPointNotConverged);
        }

        let feedback_enabled = circuit.output_transformer.negative_feedback.present
            && circuit
                .output_transformer
                .negative_feedback
                .resistor_from_secondary_ohms_visible
                > 0.0;
        // La toma de 16 Ω entrega `N8/N16` veces la tensión de la carga de 8 Ω.
        let feedback_tap_gain = circuit.output_transformer.turns_ratio_primary_to_8ohm
            / circuit.output_transformer.turns_ratio_primary_to_16ohm;

        Ok(Self {
            input: InputStage::new(),
            channel_1,
            channel_2,
            mixer,
            inverter,
            power,
            // 15 ms **físicos** a cualquier factor: `next_value()` avanza una
            // vez por muestra de proceso, de modo que escalar el tiempo por el
            // sobremuestreo lo alargaba a 30/60 ms (B18-004). Mismo criterio
            // que los suavizados de `ChannelChain`.
            input_trim_db: SmoothedValue::new(0.0, processing_rate as f32, CONTROL_SMOOTHING_MS),
            output_trim_db: SmoothedValue::new(0.0, processing_rate as f32, CONTROL_SMOOTHING_MS),
            input_gain: MappedControl::new(),
            output_gain: MappedControl::new(),
            channel: InputChannel::default(),
            feedback_enabled,
            feedback_tap_gain,
            // La fase del arrollamiento de 16 Ω no está documentada; se asume
            // la que produce realimentación **negativa** (el amplificador real
            // la tiene). El test de lazo lo verifica.
            feedback_polarity: -1.0,
            feedback_state: 0.0,
            // El polo se construye con la **tasa de proceso efectiva**: este es
            // el ritmo al que `process_sample` avanza la realimentación, así que
            // el corte físico es el mismo a 1×/2×/4× (B18-002; antes se construía
            // con la tasa base y el corte efectivo escalaba con el sobremuestreo).
            feedback_lowpass: OnePoleLowPass::new(FEEDBACK_COMPENSATION_CUTOFF_HZ, processing_rate),
        })
    }

    fn operating_point(&self) -> OperatingPoint {
        let mut point = self.channel_1.operating_point();
        point.phase_inverter = self.inverter.operating_point();
        point.power_plate_v = self.power.plate_voltage(0);
        point.power_idle_plate_current_a = self.power.idle_plate_current_a();
        point.power_idle_screen_current_a = self.power.idle_screen_current_a();
        point
    }

    fn convergence_failures(&self) -> u64 {
        self.channel_1.convergence_failures()
            + self.channel_2.convergence_failures()
            + self.inverter.convergence_failures()
            + self.power.convergence_failures()
    }

    fn set_targets(&mut self, controls: &RuntimeControls) {
        self.input_trim_db.set_target(controls.input_trim_db);
        self.output_trim_db.set_target(controls.output_trim_db);
        self.channel_1.set_targets(controls, true);
        self.channel_2.set_targets(controls, false);
        self.channel = controls.channel;
    }

    /// Restaura el estado desde la cadena de reposo **sin asignar memoria**
    /// (los `Vec` del MNA reutilizan su capacidad).
    fn restore_from(&mut self, other: &Self) {
        self.input = other.input;
        self.channel_1.restore_from(&other.channel_1);
        self.channel_2.restore_from(&other.channel_2);
        self.mixer = other.mixer.clone();
        self.inverter = other.inverter.clone();
        self.power = other.power.clone();
        self.input_trim_db = other.input_trim_db;
        self.output_trim_db = other.output_trim_db;
        self.input_gain = other.input_gain;
        self.output_gain = other.output_gain;
        self.channel = other.channel;
        self.feedback_enabled = other.feedback_enabled;
        self.feedback_tap_gain = other.feedback_tap_gain;
        self.feedback_polarity = other.feedback_polarity;
        self.feedback_state = other.feedback_state;
        self.feedback_lowpass = other.feedback_lowpass;
    }

    fn process_sample(&mut self, sample: f32) -> f32 {
        let input_gain = self.input_gain.get(self.input_trim_db.next_value(), |db| {
            f64::from(db_to_gain(db))
        });
        let volts = self.input.process_sample(sample) * input_gain;

        let (channel_1_input, channel_2_input) = match self.channel {
            InputChannel::Bass | InputChannel::Guitar => (volts, 0.0),
            InputChannel::Instrument => (0.0, volts),
        };

        let channel_1 = self.channel_1.process(channel_1_input);
        let channel_2 = self.channel_2.process(channel_2_input);
        let mixed = self.mixer.process(channel_1, channel_2);
        // Inversor real de la 7199 con realimentación negativa al cátodo del
        // pentodo. La separación es explícita `z^-1`: la muestra de la toma de
        // 16 Ω (del sample anterior de la tasa de proceso) se inyecta en el
        // cátodo. Este `z^-1` no es un retardo de muestra del host.
        let feedback = if self.feedback_enabled {
            self.feedback_lowpass.process(self.feedback_state) * self.feedback_polarity
        } else {
            0.0
        };
        let (upper, lower) = self.inverter.process(mixed, feedback);
        let secondary = self.power.process_grids(upper, lower);
        if self.feedback_enabled {
            self.feedback_state = secondary * self.feedback_tap_gain;
        }
        let output_gain = self
            .output_gain
            .get(self.output_trim_db.next_value(), |db| {
                f64::from(db_to_gain(db))
            });
        (secondary * output_gain) as f32
    }
}

/// Cadena de un canal: 6SL7#1, acoplo, volumen, 6SL7#2, acoplo y tono.
#[derive(Clone)]
struct ChannelChain {
    triode_1: NonlinearTriodeStage,
    coupling_1: OnePoleHighPass,
    coupling_1_gain: f64,
    triode_2: NonlinearTriodeStage,
    coupling_2: OnePoleHighPass,
    coupling_2_gain: f64,
    tone: ToneStackProcessor,
    volume: SmoothedValue,
    bass: SmoothedValue,
    treble: SmoothedValue,
    volume_gain: MappedControl,
}

impl ChannelChain {
    fn new(
        circuit: &CircuitDefinition,
        tone_design: &ToneStackDesign,
        processing_rate_hz: f64,
        grid_leak_ohms: f64,
    ) -> Result<Self, EngineError> {
        let params = circuit
            .tube_models
            .koren_6sl7
            .ok_or(EngineError::MissingTubeModel)?;
        let supply_v = circuit
            .tube_models
            .preamp_supply_v
            .ok_or(EngineError::MissingTubeModel)?;
        let model = KorenTriodeModel::new(params);
        let volume_pot_ohms = circuit.channel.volume_pot.max_ohms;

        // --- Triodo 1 (conexión directa desde la red de entrada) ---
        let triode_1_config = TriodeStageConfig {
            plate_resistor_ohms: circuit.channel.triode_1.plate_resistor_ohms,
            cathode_resistor_ohms: circuit.channel.triode_1.cathode_resistor_ohms,
            grid_leak_ohms,
            supply_v,
            coupling_farads: None,
            series_resistance_ohms: INPUT_SERIES_OHMS,
            sample_rate_hz: processing_rate_hz,
        };
        let mut triode_1 = NonlinearTriodeStage::new(model, triode_1_config);
        if !triode_1.solve_operating_point().converged {
            return Err(EngineError::OperatingPointNotConverged);
        }

        // --- Triodo 2 (rejilla desde el cursor del volumen) ---
        let triode_2_config = TriodeStageConfig {
            plate_resistor_ohms: circuit.channel.triode_2.plate_resistor_ohms,
            cathode_resistor_ohms: circuit.channel.triode_2.cathode_resistor_ohms,
            grid_leak_ohms: volume_pot_ohms,
            supply_v,
            coupling_farads: None,
            // Cota superior de la impedancia de Thevenin del potenciómetro.
            series_resistance_ohms: volume_pot_ohms * 0.25,
            sample_rate_hz: processing_rate_hz,
        };
        let mut triode_2 = NonlinearTriodeStage::new(model, triode_2_config);
        if !triode_2.solve_operating_point().converged {
            return Err(EngineError::OperatingPointNotConverged);
        }

        // --- Impedancias de salida y acoplos en el reposo ---
        let rp_1 = triode_1.model().plate_resistance(
            triode_1.plate_voltage(),
            triode_1.grid_voltage() - triode_1.cathode_voltage(),
        );
        let rp_2 = triode_2.model().plate_resistance(
            triode_2.plate_voltage(),
            triode_2.grid_voltage() - triode_2.cathode_voltage(),
        );
        let output_impedance_1 = triode_output_impedance_ohms(
            params.mu,
            rp_1,
            circuit.channel.triode_1.plate_resistor_ohms,
            circuit.channel.triode_1.cathode_resistor_ohms,
        );
        let output_impedance_2 = triode_output_impedance_ohms(
            params.mu,
            rp_2,
            circuit.channel.triode_2.plate_resistor_ohms,
            circuit.channel.triode_2.cathode_resistor_ohms,
        );

        let coupling_1_total = output_impedance_1 + volume_pot_ohms;
        let mut coupling_1 = OnePoleHighPass::new(
            1.0 / (2.0 * PI * circuit.channel.coupling_cap_1_farads * coupling_1_total),
            processing_rate_hz,
        );
        coupling_1.preset(triode_1.plate_voltage());
        let coupling_1_gain = volume_pot_ohms / coupling_1_total;

        let tone_series_ohms = tone_design.series_resistor_ohms;
        let coupling_2_total = output_impedance_2 + tone_series_ohms;
        let mut coupling_2 = OnePoleHighPass::new(
            1.0 / (2.0 * PI * circuit.channel.coupling_cap_2_farads * coupling_2_total),
            processing_rate_hz,
        );
        coupling_2.preset(triode_2.plate_voltage());
        let coupling_2_gain = tone_series_ohms / coupling_2_total;

        // La red de tono corre a la tasa de proceso (ya sobremuestreada).
        let tone = ToneStackProcessor::new(
            *tone_design,
            output_impedance_2,
            circuit.channel.channel_mixer_resistor_ohms,
            processing_rate_hz,
            1,
        );

        let smoother = |initial: f32| {
            SmoothedValue::new(initial, processing_rate_hz as f32, CONTROL_SMOOTHING_MS)
        };

        Ok(Self {
            triode_1,
            coupling_1,
            coupling_1_gain,
            triode_2,
            coupling_2,
            coupling_2_gain,
            tone,
            volume: smoother(0.5),
            bass: smoother(0.5),
            treble: smoother(0.5),
            volume_gain: MappedControl::new(),
        })
    }

    fn operating_point(&self) -> OperatingPoint {
        OperatingPoint {
            triode_1: StageOperatingPoint {
                plate_v: self.triode_1.plate_voltage(),
                cathode_v: self.triode_1.cathode_voltage(),
            },
            triode_2: StageOperatingPoint {
                plate_v: self.triode_2.plate_voltage(),
                cathode_v: self.triode_2.cathode_voltage(),
            },
            // Lo completa `LinearChain::operating_point` con el inversor y la
            // etapa de potencia.
            phase_inverter: PhaseInverterOperatingPoint::default(),
            power_plate_v: 0.0,
            power_idle_plate_current_a: 0.0,
            power_idle_screen_current_a: 0.0,
        }
    }

    fn convergence_failures(&self) -> u64 {
        self.triode_1.convergence_failures() + self.triode_2.convergence_failures()
    }

    fn set_targets(&mut self, controls: &RuntimeControls, channel_one: bool) {
        if channel_one {
            self.volume.set_target(controls.volume_1);
            self.bass.set_target(controls.bass_1);
            self.treble.set_target(controls.treble_1);
        } else {
            self.volume.set_target(controls.volume_2);
            self.bass.set_target(controls.bass_2);
            self.treble.set_target(controls.treble_2);
        }
    }

    /// Salta directo al objetivo de los mandos (instancia recién construida:
    /// sin rampa de 15 ms a ganancia completa en el primer bloque del host).
    fn snap_smoothing(&mut self) {
        self.volume.snap();
        self.bass.snap();
        self.treble.snap();
    }

    /// Restaura el estado desde el canal de reposo sin asignar memoria.
    fn restore_from(&mut self, other: &Self) {
        self.triode_1 = other.triode_1.clone();
        self.coupling_1 = other.coupling_1;
        self.coupling_1_gain = other.coupling_1_gain;
        self.triode_2 = other.triode_2.clone();
        self.coupling_2 = other.coupling_2;
        self.coupling_2_gain = other.coupling_2_gain;
        self.tone.restore_from(&other.tone);
        self.volume = other.volume;
        self.bass = other.bass;
        self.treble = other.treble;
        self.volume_gain = other.volume_gain;
    }

    fn process(&mut self, input_volts: f64) -> f64 {
        let volume_position = self.volume.next_value();
        let controls = [
            volume_position,
            self.bass.next_value(),
            self.treble.next_value(),
        ];
        self.tone.set_controls(ToneStackControls {
            bass: controls[1],
            treble: controls[2],
        });
        let plate_1 = self.triode_1.process(input_volts);
        let drive_1 = self.coupling_1.process(plate_1) * self.coupling_1_gain;
        let divided = drive_1 * self.volume_gain.get(volume_position, volume_fraction);
        let plate_2 = self.triode_2.process(divided);
        let tone_input = self.coupling_2.process(plate_2) * self.coupling_2_gain;
        self.tone.process_sample(tone_input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::circuit::test_circuit;

    fn engine() -> B18nEngine {
        B18nEngine::new(
            test_circuit(),
            ProcessSpec {
                sample_rate_hz: 48_000.0,
                max_block_frames: 1024,
                oversampling: 4,
            },
        )
        .expect("especificación válida")
    }

    fn sine_input(frames: usize) -> Vec<f32> {
        (0..frames)
            .map(|index| (index as f32 * 0.05).sin() * 0.3)
            .collect()
    }

    #[test]
    fn mapped_controls_preserve_sample_by_sample_smoothing() {
        let mut smooth = SmoothedValue::new(-16.5, 48000.0, 15.0);
        let mut cache = MappedControl::new();
        for i in 0..10000 {
            if i % 2000 == 0 {
                smooth.set_target(-30.0 + i as f32 * 0.004);
            }
            let position = smooth.next_value();
            assert_eq!(
                cache.get(position, |db| f64::from(db_to_gain(db))),
                f64::from(db_to_gain(position))
            );
        }
        let mut calls = 0;
        cache.get(0.5, |p| {
            calls += 1;
            volume_fraction(p)
        });
        cache.get(0.5, |p| {
            calls += 1;
            volume_fraction(p)
        });
        assert_eq!(calls, 1);
    }

    #[test]
    fn new_rejects_invalid_spec() {
        let result = B18nEngine::new(
            test_circuit(),
            ProcessSpec {
                sample_rate_hz: 48_000.0,
                max_block_frames: 1024,
                oversampling: 3,
            },
        );
        assert!(matches!(
            result,
            Err(EngineError::UnsupportedOversampling { factor: 3 })
        ));
    }

    #[test]
    fn operating_point_is_solved_at_init() {
        let engine = engine();
        assert!(engine.state().operating_point_solved);
        let op = engine.state().operating_point.expect("reposo registrado");
        assert!(
            (op.triode_1.plate_v - 175.0).abs() / 175.0 < 0.15,
            "triodo 1 placa {} V",
            op.triode_1.plate_v
        );
        assert!(
            (op.triode_1.cathode_v - 2.4).abs() / 2.4 < 0.30,
            "triodo 1 cátodo {} V",
            op.triode_1.cathode_v
        );
        assert!(
            (op.triode_2.plate_v - 185.0).abs() / 185.0 < 0.20,
            "triodo 2 placa {} V",
            op.triode_2.plate_v
        );
        // Inversor 7199 real: contraste con el manual (pin 1 = 360 V,
        // pin 8 = 100 V) y con el golden SPICE (`phase_7199/`).
        let inverter = op.phase_inverter;
        assert!(
            (inverter.triode_plate_v - 356.0).abs() / 356.0 < 0.05,
            "placa del triodo inversor {} V",
            inverter.triode_plate_v
        );
        assert!(
            (inverter.triode_cathode_v - 102.7).abs() / 102.7 < 0.08,
            "cátodo del triodo inversor {} V",
            inverter.triode_cathode_v
        );
        assert!(
            (inverter.pentode_plate_v - 88.3).abs() / 88.3 < 0.20,
            "placa del pentodo 7199 {} V",
            inverter.pentode_plate_v
        );
        assert!(
            (inverter.pentode_screen_v - 66.1).abs() / 66.1 < 0.15,
            "pantalla del pentodo 7199 {} V",
            inverter.pentode_screen_v
        );
        // Etapa de potencia: el JSON deja la corriente de reposo en `null`;
        // solo se comprueba coherencia del modelo (rango y disipación).
        assert!(
            op.power_idle_plate_current_a > 0.005 && op.power_idle_plate_current_a < 0.06,
            "reposo de la 7027A {} A",
            op.power_idle_plate_current_a
        );
        assert!(
            op.power_plate_v * op.power_idle_plate_current_a < 30.0,
            "disipación excesiva: {} W",
            op.power_plate_v * op.power_idle_plate_current_a
        );
        assert_eq!(engine.convergence_failures(), 0);
    }

    #[test]
    fn chain_is_deterministic_and_finite() {
        let input = sine_input(257);

        let mut first = engine();
        let mut out_first = vec![0.0_f32; input.len()];
        first.process_mono(&input, &mut out_first);

        let mut second = engine();
        let mut out_second = vec![0.0_f32; input.len()];
        second.process_mono(&input, &mut out_second);

        assert_eq!(out_first, out_second, "misma entrada debe dar misma salida");
        assert!(out_first.iter().all(|sample| sample.is_finite()));
        assert!(
            out_first.iter().any(|sample| sample.abs() > 0.0),
            "la cadena debe dejar pasar señal"
        );
    }

    #[test]
    fn silence_stays_silent_from_operating_point() {
        let mut engine = engine();
        let input = vec![0.0_f32; 1024];
        let mut output = vec![1.0_f32; 1024];
        engine.process_mono(&input, &mut output);
        let residual = output
            .iter()
            .fold(0.0_f32, |acc, sample| acc.max(sample.abs()));
        assert!(
            residual < 1e-6,
            "residuo de silencio demasiado alto: {residual}"
        );
    }

    #[test]
    fn output_is_block_size_invariant() {
        let input = sine_input(4096);

        let mut whole = engine();
        let mut out_whole = vec![0.0_f32; input.len()];
        whole.process_mono(&input, &mut out_whole);

        let mut chunked = engine();
        let mut out_chunked = Vec::with_capacity(input.len());
        for chunk in input.chunks(257) {
            let mut block = vec![0.0_f32; chunk.len()];
            chunked.process_mono(chunk, &mut block);
            out_chunked.extend_from_slice(&block);
        }

        assert_eq!(out_whole, out_chunked, "sin glitches por tamaño de bloque");
    }

    #[test]
    fn strong_signal_stays_finite() {
        let mut engine = engine();
        let input = sine_input(24_000);
        let mut output = vec![0.0_f32; input.len()];
        engine.process_mono(&input, &mut output);
        assert!(output.iter().all(|sample| sample.is_finite()));
        assert!(output.iter().fold(0.0_f32, |acc, s| acc.max(s.abs())) < 100.0);
        assert!(
            engine.convergence_failures() < input.len() as u64 / 10,
            "fallos de convergencia: {}",
            engine.convergence_failures()
        );
    }

    #[test]
    fn oversampling_reports_latency() {
        let engine_4x = engine();
        assert_eq!(engine_4x.latency_samples(), 47);

        let engine_1x = B18nEngine::new(
            test_circuit(),
            ProcessSpec {
                sample_rate_hz: 48_000.0,
                max_block_frames: 1024,
                oversampling: 1,
            },
        )
        .expect("especificación válida");
        assert_eq!(engine_1x.latency_samples(), 0);
    }

    #[test]
    fn zero_fills_output_beyond_input() {
        let mut engine = engine();
        let input = [0.25_f32, -0.5];
        let mut output = [1.0_f32; 4];
        engine.process_mono(&input, &mut output);
        assert_eq!(output[2], 0.0);
        assert_eq!(output[3], 0.0);
    }

    #[test]
    fn empty_and_single_frame_are_safe() {
        let mut engine = engine();
        let mut empty: [f32; 0] = [];
        engine.process_mono(&[], &mut empty);

        let mut single = [0.0_f32; 1];
        engine.process_mono(&[0.75], &mut single);
        assert!(single[0].is_finite());
    }

    #[test]
    fn reset_returns_to_settled_state() {
        let input = sine_input(512);
        let mut engine = engine();
        let mut first = vec![0.0_f32; input.len()];
        engine.process_mono(&input, &mut first);
        engine.reset();
        let mut second = vec![0.0_f32; input.len()];
        engine.process_mono(&input, &mut second);
        assert_eq!(first, second, "reset debe reproducir la misma respuesta");
        assert!(engine.state().operating_point_solved);
    }

    #[test]
    fn input_selection_routes_to_the_matching_channel() {
        let input = sine_input(4096);

        let controls = |channel| RuntimeControls {
            channel,
            volume_1: 0.8,
            volume_2: 0.05,
            ..RuntimeControls::default()
        };

        let mut bass_engine = engine();
        bass_engine.set_controls(controls(InputChannel::Bass));
        let mut bass_out = vec![0.0_f32; input.len()];
        bass_engine.process_mono(&input, &mut bass_out);

        let mut instrument_engine = engine();
        instrument_engine.set_controls(controls(InputChannel::Instrument));
        let mut instrument_out = vec![0.0_f32; input.len()];
        instrument_engine.process_mono(&input, &mut instrument_out);

        // Se mide la cola para no incluir el transitorio del suavizado.
        let tail_peak = |samples: &[f32]| {
            samples[samples.len() * 3 / 4..]
                .iter()
                .fold(0.0_f32, |acc, s| acc.max(s.abs()))
        };
        let bass_peak = tail_peak(&bass_out);
        let instrument_peak = tail_peak(&instrument_out);
        assert!(bass_peak > 0.0 && instrument_peak > 0.0);
        assert!(
            bass_peak > 5.0 * instrument_peak,
            "bass {bass_peak} instrument {instrument_peak}"
        );
    }
}
