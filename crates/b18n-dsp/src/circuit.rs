//! Definición física e inmutable del circuito.
//!
//! Los valores provienen del JSON histórico a través de `b18n-config`. Los
//! campos sin caracterizar en la documentación llegan como `Option::None` y se
//! enumeran en [`CircuitDefinition::missing`]; `None` nunca significa cero ni
//! comportamiento ideal validado.

use std::collections::BTreeMap;

/// Circuito completo tal como lo consume el motor DSP.
#[derive(Debug, Clone, PartialEq)]
pub struct CircuitDefinition {
    pub identity: CircuitIdentity,
    pub input: InputNetworkValues,
    pub channel: ChannelStripValues,
    pub tone_stack: ToneStackValues,
    pub phase_inverter: PhaseInverterValues,
    pub power_stage: PowerStageValues,
    pub output_transformer: OutputTransformerValues,
    pub power_supply: PowerSupplyValues,
    pub speaker: SpeakerValues,
    /// Parámetros de modelo de válvula procedentes del overlay de calibración.
    pub tube_models: TubeModelValues,
    /// Nivel de fidelidad alcanzado por la topología/caracterización actual.
    pub fidelity: FidelityLevel,
    /// Datos que la documentación histórica deja sin caracterizar.
    pub missing: Vec<MissingValue>,
}

/// Parámetros de modelo que **no** están en el JSON canónico: llegan del
/// archivo de calibración, etiquetados como `estimated`, `measured` o
/// `datasheet`. `None` = sin modelo (el motor debe fallar explícitamente).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TubeModelValues {
    pub koren_6sl7: Option<KorenTriodeParameters>,
    /// Pentodo de potencia 7027A (modelo Koren con corriente de pantalla).
    pub koren_7027a: Option<KorenPentodeParameters>,
    /// Pentodo de la 7199 (etapa de entrada del inversor), ajustado al boletín RCA.
    pub koren_7199_pentode: Option<KorenPentodeParameters>,
    /// Triodo de la 7199 (inversor de carga dividida), ajustado al boletín RCA.
    pub koren_7199_triode: Option<KorenTriodeParameters>,
    /// Alimentación del previo (nodo de las placas 470 kΩ/220 kΩ).
    pub preamp_supply_v: Option<f64>,
}

/// Parámetros del modelo Koren de un pentodo/beam tetrode.
///
/// Proveniencia esperada: parámetros publicados de un tubo padre (p. ej. la
/// familia 6L6GC para la 7027A) o un ajuste documentado. `None` = sin modelo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KorenPentodeParameters {
    pub mu: f64,
    pub ex: f64,
    pub kg1: f64,
    pub kg2: f64,
    pub kp: f64,
    pub kvb: f64,
    pub vct: f64,
    pub grid_conduction_threshold_v: f64,
    pub grid_conduction_resistance_ohms: f64,
}

/// Parámetros del modelo Koren de un triodo (ecuaciones públicas) más una
/// aproximación lineal por tramos de la conducción de rejilla.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KorenTriodeParameters {
    pub mu: f64,
    pub ex: f64,
    pub kg1: f64,
    pub kp: f64,
    pub kvb: f64,
    pub vct: f64,
    /// Tensión de rejilla (Vgk) a partir de la cual conduce la rejilla.
    pub grid_conduction_threshold_v: f64,
    /// Resistencia equivalente de la rejilla en conducción.
    pub grid_conduction_resistance_ohms: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CircuitIdentity {
    pub manufacturer: String,
    pub model: String,
    pub schematic_revision: String,
}

/// Red de entrada: conectores y componentes visibles.
///
/// El JSON advierte que los valores abarcan entradas y canales distintos; su
/// interconexión se conserva en `docs/CIRCUIT_TRACEABILITY.md`, no en esta
/// lista.
#[derive(Debug, Clone, PartialEq)]
pub struct InputNetworkValues {
    pub channel_1_connectors: Vec<String>,
    pub channel_2_connectors: Vec<String>,
    pub visible_component_values_ohms: Vec<f64>,
}

/// Un canal de previo 6SL7GT (dos triodos en cascada).
#[derive(Debug, Clone, PartialEq)]
pub struct ChannelStripValues {
    pub triode_1: TriodeStageValues,
    pub coupling_cap_1_farads: f64,
    pub triode_2: TriodeStageValues,
    pub coupling_cap_2_farads: f64,
    pub channel_mixer_resistor_ohms: f64,
    pub typical_dc_plate_1_v: f64,
    pub typical_dc_plate_2_v: f64,
    pub typical_dc_cathode_1_v: f64,
    pub typical_dc_cathode_2_v: f64,
    pub volume_pot: PotValues,
    pub bass_pot: PotValues,
    pub treble_pot: PotValues,
    pub reference_6sl7: Tube6sl7Reference,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TriodeStageValues {
    pub plate_resistor_ohms: f64,
    pub cathode_resistor_ohms: f64,
    /// `None` = sin bypass documentado (no se asume ninguno).
    pub cathode_bypass_farads: Option<f64>,
}

/// Referencia de pequeña señal a 250 V; no caracteriza saturación.
#[derive(Debug, Clone, PartialEq)]
pub struct Tube6sl7Reference {
    pub mu_typ: f64,
    pub rp_ohms_typ: f64,
    pub gm_s_typ: f64,
    pub ia_a_typ: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PotLaw {
    Linear,
    Logarithmic,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PotValues {
    pub max_ohms: f64,
    pub law: PotLaw,
}

/// Valores de la red de tono. Las conexiones se digitalizan aparte: el orden de
/// los arrays no las codifica.
#[derive(Debug, Clone, PartialEq)]
pub struct ToneStackValues {
    pub circuit: String,
    pub printed_board_resistors_ohms: Vec<f64>,
    pub printed_board_capacitors_farads: Vec<f64>,
    pub additional_resistors_visible_ohms: Vec<f64>,
    pub connection_source: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PhaseInverterValues {
    pub tube: String,
    pub pentode_plate_resistor_ohms: f64,
    pub pentode_screen_feed_resistor_ohms: f64,
    pub pentode_cathode_resistor_ohms: f64,
    pub pentode_cathode_bypass_farads: Option<f64>,
    pub pentode_to_triode_coupling_farads: f64,
    pub large_resistors_ohms: Vec<f64>,
    pub phase_splitter_resistors_ohms: Vec<f64>,
    /// Tensiones DC por pin sin señal; referencia de contraste, no solución.
    pub dc_pin_volts_no_signal: BTreeMap<u8, f64>,
    /// Carril de placa del triodo inversor (dato trazado por la errata).
    pub triode_supply_v: Option<f64>,
    /// Componentes trazados que el JSON inicial no listaba (errata histórica).
    pub additional_traced_resistors: Vec<TracedResistor>,
}

/// Componente trazado en el esquema que el JSON canónico no lista.
#[derive(Debug, Clone, PartialEq)]
pub struct TracedResistor {
    pub ohms: f64,
    pub from_node: String,
    pub to_node: String,
    pub evidence: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PowerStageValues {
    pub tube_types: Vec<String>,
    pub topology: String,
    pub plate_voltage_v_nominal: f64,
    pub screen_voltage_v_nominal: f64,
    pub control_grid_voltage_v_nominal: f64,
    pub cathode_voltage_v_nominal: f64,
    pub grid_stopper_ohms_each: f64,
    pub grid_return_ohms_each: f64,
    pub inverter_to_grid_coupling_farads_each: f64,
    /// `None` = corriente de reposo sin caracterizar (no inventar).
    pub idle_plate_current_a_each: Option<f64>,
    pub idle_screen_current_a_each: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OutputTransformerValues {
    pub model: String,
    pub primary_plate_to_plate_ohms_nominal: f64,
    pub secondary_ohms: Vec<f64>,
    pub dc_primary_winding_ohms: f64,
    pub dc_secondary_8ohm_winding_ohms: f64,
    pub dc_secondary_16ohm_winding_ohms: f64,
    pub turns_ratio_primary_to_8ohm: f64,
    pub turns_ratio_primary_to_16ohm: f64,
    pub winding_inductance_h: Option<f64>,
    pub leakage_inductance_h: Option<f64>,
    pub parasitic_capacitance_f: Option<f64>,
    pub negative_feedback: NegativeFeedbackValues,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NegativeFeedbackValues {
    pub present: bool,
    pub resistor_from_secondary_ohms_visible: f64,
    /// El nodo y la polaridad exactos se toman del esquema 5/67.
    pub topology_note: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PowerSupplyValues {
    pub rectifier: String,
    pub power_transformer_models: Vec<String>,
    pub mains_primary_us_vac: f64,
    pub mains_primary_export_vac: f64,
    pub mains_frequencies_hz: Vec<f64>,
    pub hv_secondary_vac_center_tapped_total: f64,
    pub hv_dc_current_rating_a: f64,
    pub bplus_idle_vdc: f64,
    pub fixed_bias_nominal_vdc: f64,
    pub hv_filter_capacitors_farads: Vec<f64>,
    pub bias_filter_capacitor_farads: f64,
    pub bias_filter_voltage_rating_v: f64,
    pub rectifier_diode_bias: String,
    pub dropping_resistor_values_ohms: Vec<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpeakerValues {
    pub nominal_diameter_inches: f64,
    pub nominal_load_ohms: f64,
    pub cabinet_type: String,
    pub impedance_curve_available: bool,
    pub measured_ir_available: bool,
}

/// Nivel de fidelidad de la caracterización (véase `MODEL_ASSUMPTIONS.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FidelityLevel {
    TopologyUnverified,
    Approximate,
    Calibrated,
    Measured,
}

impl FidelityLevel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TopologyUnverified => "topology_unverified",
            Self::Approximate => "approximate",
            Self::Calibrated => "calibrated",
            Self::Measured => "measured",
        }
    }
}

/// Dato sin caracterizar que el modelo no puede completar por sí solo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingValue {
    /// Ruta del campo, con la nomenclatura del JSON.
    pub field: &'static str,
    /// Qué falta y consecuencia de no tenerlo.
    pub note: &'static str,
}

#[cfg(test)]
pub(crate) fn test_circuit() -> CircuitDefinition {
    CircuitDefinition {
        identity: CircuitIdentity {
            manufacturer: "Ampeg".to_owned(),
            model: "B-18N Portaflex".to_owned(),
            schematic_revision: "1967-05".to_owned(),
        },
        input: InputNetworkValues {
            channel_1_connectors: vec!["bass".to_owned(), "guitar".to_owned()],
            channel_2_connectors: vec!["instrument".to_owned()],
            visible_component_values_ohms: vec![
                120_000.0,
                120_000.0,
                100_000.0,
                5_600_000.0,
                2_200_000.0,
            ],
        },
        channel: ChannelStripValues {
            triode_1: TriodeStageValues {
                plate_resistor_ohms: 470_000.0,
                cathode_resistor_ohms: 5_600.0,
                cathode_bypass_farads: None,
            },
            coupling_cap_1_farads: 2.2e-8,
            triode_2: TriodeStageValues {
                plate_resistor_ohms: 220_000.0,
                cathode_resistor_ohms: 2_200.0,
                cathode_bypass_farads: None,
            },
            coupling_cap_2_farads: 2.2e-8,
            channel_mixer_resistor_ohms: 270_000.0,
            typical_dc_plate_1_v: 175.0,
            typical_dc_plate_2_v: 185.0,
            typical_dc_cathode_1_v: 2.4,
            typical_dc_cathode_2_v: 2.0,
            volume_pot: PotValues {
                max_ohms: 1_000_000.0,
                law: PotLaw::Logarithmic,
            },
            bass_pot: PotValues {
                max_ohms: 1_000_000.0,
                law: PotLaw::Linear,
            },
            treble_pot: PotValues {
                max_ohms: 1_000_000.0,
                law: PotLaw::Linear,
            },
            reference_6sl7: Tube6sl7Reference {
                mu_typ: 70.0,
                rp_ohms_typ: 44_000.0,
                gm_s_typ: 0.0016,
                ia_a_typ: 0.0023,
            },
        },
        tone_stack: ToneStackValues {
            circuit: "original coupled two-knob passive RC network".to_owned(),
            printed_board_resistors_ohms: vec![220_000.0, 22_000.0],
            printed_board_capacitors_farads: vec![1e-9, 1e-8, 4.7e-10, 4.7e-9],
            additional_resistors_visible_ohms: vec![120_000.0, 270_000.0],
            connection_source: "Service manual printed CKT and 5/67 schematic".to_owned(),
        },
        phase_inverter: PhaseInverterValues {
            tube: "7199".to_owned(),
            // Valores corregidos por la errata del proyecto (placa pin 2,
            // pantalla pin 3); el canónico conserva los históricos.
            pentode_plate_resistor_ohms: 270_000.0,
            pentode_screen_feed_resistor_ohms: 470_000.0,
            pentode_cathode_resistor_ohms: 1_500.0,
            pentode_cathode_bypass_farads: Some(1e-7),
            pentode_to_triode_coupling_farads: 2e-8,
            large_resistors_ohms: vec![10_000_000.0, 3_300_000.0],
            phase_splitter_resistors_ohms: vec![47_000.0, 47_000.0],
            dc_pin_volts_no_signal: BTreeMap::from([
                (1, 360.0),
                (2, 105.0),
                (3, 60.0),
                (6, 1.8),
                (7, 0.0),
                (8, 100.0),
                (9, 65.0),
            ]),
            triode_supply_v: Some(460.0),
            additional_traced_resistors: vec![TracedResistor {
                ohms: 150_000.0,
                from_node: "pin 3 (nodo de pantalla G2 del pentodo 7199)".to_owned(),
                to_node: "bus de masa".to_owned(),
                evidence: "fixture: adición trazada por la errata del proyecto".to_owned(),
            }],
        },
        power_stage: PowerStageValues {
            tube_types: vec!["7027A".to_owned(), "7027A".to_owned()],
            topology: "fixed-bias push-pull, nominal class AB1".to_owned(),
            plate_voltage_v_nominal: 505.0,
            screen_voltage_v_nominal: 500.0,
            control_grid_voltage_v_nominal: -55.0,
            cathode_voltage_v_nominal: 0.0,
            grid_stopper_ohms_each: 1_000.0,
            grid_return_ohms_each: 100_000.0,
            inverter_to_grid_coupling_farads_each: 5e-8,
            idle_plate_current_a_each: None,
            idle_screen_current_a_each: None,
        },
        output_transformer: OutputTransformerValues {
            model: "OT-471".to_owned(),
            primary_plate_to_plate_ohms_nominal: 4_300.0,
            secondary_ohms: vec![8.0, 16.0],
            dc_primary_winding_ohms: 300.0,
            dc_secondary_8ohm_winding_ohms: 0.4,
            dc_secondary_16ohm_winding_ohms: 0.7,
            turns_ratio_primary_to_8ohm: 23.18,
            turns_ratio_primary_to_16ohm: 16.39,
            winding_inductance_h: None,
            leakage_inductance_h: None,
            parasitic_capacitance_f: None,
            negative_feedback: NegativeFeedbackValues {
                present: true,
                resistor_from_secondary_ohms_visible: 22_000.0,
                topology_note: "Reference 5/67 schematic directly".to_owned(),
            },
        },
        power_supply: PowerSupplyValues {
            rectifier: "5AR4/GZ34".to_owned(),
            power_transformer_models: vec!["PT-109".to_owned(), "PT-12-XA".to_owned()],
            mains_primary_us_vac: 117.0,
            mains_primary_export_vac: 240.0,
            mains_frequencies_hz: vec![50.0, 60.0],
            hv_secondary_vac_center_tapped_total: 800.0,
            hv_dc_current_rating_a: 0.2,
            bplus_idle_vdc: 505.0,
            fixed_bias_nominal_vdc: -55.0,
            hv_filter_capacitors_farads: vec![3e-5, 4e-5, 4e-5, 4e-5],
            bias_filter_capacitor_farads: 1e-4,
            bias_filter_voltage_rating_v: 100.0,
            rectifier_diode_bias: "10D6".to_owned(),
            dropping_resistor_values_ohms: vec![1_000.0, 10_000.0, 22_000.0],
        },
        speaker: SpeakerValues {
            nominal_diameter_inches: 18.0,
            nominal_load_ohms: 8.0,
            cabinet_type: "Portaflex double-baffle; slotted ports".to_owned(),
            impedance_curve_available: false,
            measured_ir_available: false,
        },
        tube_models: TubeModelValues {
            koren_6sl7: Some(KorenTriodeParameters {
                mu: 66.0811,
                ex: 1.098449,
                kg1: 816.8934,
                kp: 592.0055,
                kvb: 300.3225,
                vct: 0.0,
                grid_conduction_threshold_v: -0.3,
                grid_conduction_resistance_ohms: 1_000.0,
            }),
            koren_7027a: Some(KorenPentodeParameters {
                // Parámetros publicados de la 6L6GC (tubo padre de la 7027A).
                mu: 8.7,
                ex: 1.35,
                kg1: 1460.0,
                kg2: 4500.0,
                kp: 48.0,
                kvb: 12.0,
                vct: 0.0,
                grid_conduction_threshold_v: 0.0,
                grid_conduction_resistance_ohms: 500.0,
            }),
            koren_7199_pentode: Some(KorenPentodeParameters {
                // Ajuste a las anclas del boletín RCA 7199 (scripts/fit_7199_model.py).
                mu: 22.279_549,
                ex: 1.638_27,
                kg1: 3_712.065_38,
                kg2: 3_807.560_112,
                kp: 66.513_832,
                kvb: 12.522_225,
                vct: 0.0,
                grid_conduction_threshold_v: 0.0,
                grid_conduction_resistance_ohms: 500.0,
            }),
            koren_7199_triode: Some(KorenTriodeParameters {
                // Ajuste a las anclas del boletín RCA 7199 (scripts/fit_7199_model.py).
                mu: 17.0,
                ex: 0.978_282,
                kg1: 448.917_294,
                kp: 272.472_003,
                kvb: 160.0,
                vct: 0.0,
                grid_conduction_threshold_v: -0.3,
                grid_conduction_resistance_ohms: 1_000.0,
            }),
            preamp_supply_v: Some(380.0),
        },
        fidelity: FidelityLevel::TopologyUnverified,
        missing: Vec::new(),
    }
}
