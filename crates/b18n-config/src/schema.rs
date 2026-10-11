//! Esquema Serde que refleja las claves reales del JSON histórico.
//!
//! Reglas:
//! - `deny_unknown_fields`: cualquier cambio silencioso del JSON canónico se
//!   detecta al cargar.
//! - Los campos `null` del documento se modelan como `Option<T>`; `None` no
//!   significa cero ni ideal.
//! - No se usa `serde_json::Value` para datos que el DSP necesita; solo para
//!   bloques opacos explícitamente sin caracterizar (curvas, IR, geometría).

use std::collections::BTreeMap;

use serde::Deserialize;

/// Bloque opaco sin caracterizar (curva, IR, geometría, etc.).
pub type Opaque = serde_json::Value;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoricalB18nJson {
    pub identity: Identity,
    pub controls: Controls,
    pub preamp: Preamp,
    pub tone_stack: ToneStack,
    pub phase_inverter: PhaseInverter,
    pub power_stage: PowerStage,
    pub output_transformer: OutputTransformer,
    pub power_supply: PowerSupply,
    pub cabinet: Cabinet,
    pub unknowns_requiring_characterization: Vec<String>,
    pub validation: Validation,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    pub manufacturer: String,
    pub model: String,
    pub schematic_revision: String,
    pub schematic_shared_with: String,
    pub documented_nominal_output_w: f64,
    pub other_period_reported_output_w: f64,
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Controls {
    pub channels: u32,
    pub channel_1_inputs: Vec<String>,
    pub channel_2_inputs: Vec<String>,
    pub each_channel: ControlsEachChannel,
    pub hum_balance: HumBalance,
    pub potentiometer_tolerance_percent: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ControlsEachChannel {
    pub volume: PotSpec,
    pub bass: PotSpec,
    pub treble: PotSpec,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PotSpec {
    pub ohms: f64,
    pub law: PotLawJson,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PotLawJson {
    Linear,
    Logarithmic,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HumBalance {
    pub ohms: f64,
    pub law: PotLawJson,
    pub rating_w: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preamp {
    pub v1: String,
    pub v2: String,
    pub each_channel: PreampEachChannel,
    pub input_network_values_visible_ohms: Vec<f64>,
    pub input_network_note: String,
    pub tube_6sl7gt_reference_at_250v: Tube6sl7ReferenceJson,
    pub tube_6sl7gt_characteristic_curves: Option<Opaque>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreampEachChannel {
    pub triode_1_plate_resistor_ohms: f64,
    pub triode_1_cathode_resistor_ohms: f64,
    pub triode_1_cathode_bypass_farads: Option<f64>,
    pub coupling_cap_1_farads: f64,
    pub triode_2_plate_resistor_ohms: f64,
    pub triode_2_cathode_resistor_ohms: f64,
    pub triode_2_cathode_bypass_farads: Option<f64>,
    pub coupling_cap_2_farads: f64,
    pub channel_mixer_resistor_ohms: f64,
    pub typical_dc_plate_1_v: f64,
    pub typical_dc_plate_2_v: f64,
    pub typical_dc_cathode_1_v: f64,
    pub typical_dc_cathode_2_v: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tube6sl7ReferenceJson {
    pub mu_typ: f64,
    pub rp_ohms_typ: f64,
    pub gm_s_typ: f64,
    pub ia_a_typ: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToneStack {
    pub circuit: String,
    pub printed_board_resistors_ohms: Vec<f64>,
    pub printed_board_capacitors_farads: Vec<f64>,
    pub additional_resistors_visible_in_full_schematic_ohms: Vec<f64>,
    pub connection_source: String,
    pub fixed_bass_frequency_hz: Option<f64>,
    pub fixed_treble_frequency_hz: Option<f64>,
    pub note: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhaseInverter {
    pub tube: String,
    pub pentode_plate_resistor_ohms: f64,
    pub pentode_screen_feed_resistor_ohms: f64,
    pub pentode_cathode_resistor_ohms: f64,
    pub pentode_cathode_bypass_farads: Option<f64>,
    pub pentode_to_triode_coupling_farads: f64,
    pub large_resistors_ohms: Vec<f64>,
    pub phase_splitter_resistors_ohms: Vec<f64>,
    pub dc_pin_volts_no_signal: BTreeMap<u8, f64>,
    /// Tensión del carril de placa del triodo inversor; la aporta la errata
    /// histórica (`phase_inverter.triode_supply_v`) por ser un dato trazado.
    #[serde(default)]
    pub triode_supply_v: Option<f64>,
    /// Componentes trazados que el JSON inicial no listaba; los aporta la
    /// errata histórica (`phase_inverter.additional_traced_resistors`).
    #[serde(default)]
    pub additional_traced_resistors: Vec<TracedResistorJson>,
    pub measurement_note: String,
}

/// Componente trazado en el esquema que el JSON canónico no lista.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TracedResistorJson {
    pub ohms: f64,
    pub from_node: String,
    pub to_node: String,
    pub evidence: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PowerStage {
    pub tubes: Vec<String>,
    pub topology: String,
    pub plate_voltage_v_nominal: f64,
    pub screen_voltage_v_nominal: f64,
    pub control_grid_voltage_v_nominal: f64,
    pub cathode_voltage_v_nominal: f64,
    pub grid_stopper_ohms_each: f64,
    pub grid_return_ohms_each: f64,
    pub inverter_to_grid_coupling_farads_each: f64,
    pub plate_current_idle_a_each: Option<f64>,
    pub screen_current_idle_a_each: Option<f64>,
    pub grid_conduction_model: Option<Opaque>,
    pub tube_characteristic_curves: Option<Opaque>,
    pub tube_datasheet: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputTransformer {
    pub model: String,
    pub primary_plate_to_plate_ohms_nominal: f64,
    pub secondary_ohms: Vec<f64>,
    pub dc_primary_winding_ohms: f64,
    pub dc_secondary_8ohm_winding_ohms: f64,
    pub dc_secondary_16ohm_winding_ohms: f64,
    pub nominal_turns_ratio_primary_to_8ohm: f64,
    pub nominal_turns_ratio_primary_to_16ohm: f64,
    pub winding_inductance_h: Option<f64>,
    pub leakage_inductance_h: Option<f64>,
    pub parasitic_capacitance_f: Option<f64>,
    pub core_saturation_curve: Option<Opaque>,
    pub negative_feedback: NegativeFeedback,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NegativeFeedback {
    pub present: bool,
    pub resistor_from_secondary_ohms_visible: f64,
    pub other_components_or_topology: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PowerSupply {
    pub rectifier: String,
    pub power_transformer_models: Vec<String>,
    pub mains_primary_us_vac: f64,
    pub mains_primary_export_vac: f64,
    pub mains_frequency_hz: Vec<f64>,
    pub high_voltage_secondary_vac_center_tapped_total: f64,
    pub high_voltage_dc_current_rating_a: f64,
    pub rectifier_heater_vac: f64,
    pub rectifier_heater_max_a: f64,
    pub other_heaters_vac: f64,
    pub other_heaters_max_a: f64,
    pub bplus_idle_vdc: f64,
    pub fixed_bias_nominal_vdc: f64,
    pub high_voltage_filter_capacitors_farads: Vec<f64>,
    pub bias_filter_capacitor_farads: f64,
    pub bias_filter_voltage_rating_v: f64,
    pub rectifier_diode_bias: String,
    pub dropping_resistor_values_seen_ohms: Vec<f64>,
    pub note: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cabinet {
    pub nominal_speaker_diameter_inches: f64,
    pub nominal_load_ohms: f64,
    pub historical_speaker_example: String,
    pub other_historical_speaker_makers_reported: Vec<String>,
    pub cabinet_type: String,
    pub historical_example_outer_inches_h_w_d: Vec<f64>,
    pub speaker_impedance_curve: Option<Opaque>,
    pub speaker_thiele_small: Option<Opaque>,
    pub cabinet_3d_geometry: Option<Opaque>,
    pub measured_ir: Option<Opaque>,
    pub measured_frequency_response: Option<Opaque>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Validation {
    pub reported_dc_meter_loading_ohms_per_volt: f64,
    pub dc_voltage_tolerance_percent_reported: f64,
    pub resistor_tolerance_percent_general: f64,
    pub validate_at: Vec<String>,
}
