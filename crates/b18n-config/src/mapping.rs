//! Mapeo del JSON validado a los tipos físicos de `b18n-dsp`.

use b18n_dsp::circuit::{
    ChannelStripValues, CircuitDefinition, CircuitIdentity, FidelityLevel, InputNetworkValues,
    MissingValue, NegativeFeedbackValues, OutputTransformerValues, PhaseInverterValues, PotLaw,
    PotValues, PowerStageValues, PowerSupplyValues, SpeakerValues, ToneStackValues, TracedResistor,
    TriodeStageValues, Tube6sl7Reference, TubeModelValues,
};

use crate::schema::{HistoricalB18nJson, PotLawJson};
use crate::validation::{uncharacterized_fields, validate_source};
use crate::ConfigError;

/// Valida y traduce el documento histórico a `CircuitDefinition`.
pub fn convert_to_circuit(raw: &HistoricalB18nJson) -> Result<CircuitDefinition, ConfigError> {
    validate_source(raw)?;

    let channel = &raw.preamp.each_channel;
    let controls_common = &raw.controls.each_channel;

    let circuit = CircuitDefinition {
        identity: CircuitIdentity {
            manufacturer: raw.identity.manufacturer.clone(),
            model: raw.identity.model.clone(),
            schematic_revision: raw.identity.schematic_revision.clone(),
        },
        input: InputNetworkValues {
            channel_1_connectors: raw.controls.channel_1_inputs.clone(),
            channel_2_connectors: raw.controls.channel_2_inputs.clone(),
            visible_component_values_ohms: raw.preamp.input_network_values_visible_ohms.clone(),
        },
        channel: ChannelStripValues {
            triode_1: TriodeStageValues {
                plate_resistor_ohms: channel.triode_1_plate_resistor_ohms,
                cathode_resistor_ohms: channel.triode_1_cathode_resistor_ohms,
                cathode_bypass_farads: channel.triode_1_cathode_bypass_farads,
            },
            coupling_cap_1_farads: channel.coupling_cap_1_farads,
            triode_2: TriodeStageValues {
                plate_resistor_ohms: channel.triode_2_plate_resistor_ohms,
                cathode_resistor_ohms: channel.triode_2_cathode_resistor_ohms,
                cathode_bypass_farads: channel.triode_2_cathode_bypass_farads,
            },
            coupling_cap_2_farads: channel.coupling_cap_2_farads,
            channel_mixer_resistor_ohms: channel.channel_mixer_resistor_ohms,
            typical_dc_plate_1_v: channel.typical_dc_plate_1_v,
            typical_dc_plate_2_v: channel.typical_dc_plate_2_v,
            typical_dc_cathode_1_v: channel.typical_dc_cathode_1_v,
            typical_dc_cathode_2_v: channel.typical_dc_cathode_2_v,
            volume_pot: pot(&controls_common.volume),
            bass_pot: pot(&controls_common.bass),
            treble_pot: pot(&controls_common.treble),
            reference_6sl7: Tube6sl7Reference {
                mu_typ: raw.preamp.tube_6sl7gt_reference_at_250v.mu_typ,
                rp_ohms_typ: raw.preamp.tube_6sl7gt_reference_at_250v.rp_ohms_typ,
                gm_s_typ: raw.preamp.tube_6sl7gt_reference_at_250v.gm_s_typ,
                ia_a_typ: raw.preamp.tube_6sl7gt_reference_at_250v.ia_a_typ,
            },
        },
        tone_stack: ToneStackValues {
            circuit: raw.tone_stack.circuit.clone(),
            printed_board_resistors_ohms: raw.tone_stack.printed_board_resistors_ohms.clone(),
            printed_board_capacitors_farads: raw.tone_stack.printed_board_capacitors_farads.clone(),
            additional_resistors_visible_ohms: raw
                .tone_stack
                .additional_resistors_visible_in_full_schematic_ohms
                .clone(),
            connection_source: raw.tone_stack.connection_source.clone(),
        },
        phase_inverter: PhaseInverterValues {
            tube: raw.phase_inverter.tube.clone(),
            pentode_plate_resistor_ohms: raw.phase_inverter.pentode_plate_resistor_ohms,
            pentode_screen_feed_resistor_ohms: raw.phase_inverter.pentode_screen_feed_resistor_ohms,
            pentode_cathode_resistor_ohms: raw.phase_inverter.pentode_cathode_resistor_ohms,
            pentode_cathode_bypass_farads: raw.phase_inverter.pentode_cathode_bypass_farads,
            pentode_to_triode_coupling_farads: raw.phase_inverter.pentode_to_triode_coupling_farads,
            large_resistors_ohms: raw.phase_inverter.large_resistors_ohms.clone(),
            phase_splitter_resistors_ohms: raw.phase_inverter.phase_splitter_resistors_ohms.clone(),
            dc_pin_volts_no_signal: raw.phase_inverter.dc_pin_volts_no_signal.clone(),
            triode_supply_v: raw.phase_inverter.triode_supply_v,
            additional_traced_resistors: raw
                .phase_inverter
                .additional_traced_resistors
                .iter()
                .map(|resistor| TracedResistor {
                    ohms: resistor.ohms,
                    from_node: resistor.from_node.clone(),
                    to_node: resistor.to_node.clone(),
                    evidence: resistor.evidence.clone(),
                })
                .collect(),
        },
        power_stage: PowerStageValues {
            tube_types: raw.power_stage.tubes.clone(),
            topology: raw.power_stage.topology.clone(),
            plate_voltage_v_nominal: raw.power_stage.plate_voltage_v_nominal,
            screen_voltage_v_nominal: raw.power_stage.screen_voltage_v_nominal,
            control_grid_voltage_v_nominal: raw.power_stage.control_grid_voltage_v_nominal,
            cathode_voltage_v_nominal: raw.power_stage.cathode_voltage_v_nominal,
            grid_stopper_ohms_each: raw.power_stage.grid_stopper_ohms_each,
            grid_return_ohms_each: raw.power_stage.grid_return_ohms_each,
            inverter_to_grid_coupling_farads_each: raw
                .power_stage
                .inverter_to_grid_coupling_farads_each,
            idle_plate_current_a_each: raw.power_stage.plate_current_idle_a_each,
            idle_screen_current_a_each: raw.power_stage.screen_current_idle_a_each,
        },
        output_transformer: OutputTransformerValues {
            model: raw.output_transformer.model.clone(),
            primary_plate_to_plate_ohms_nominal: raw
                .output_transformer
                .primary_plate_to_plate_ohms_nominal,
            secondary_ohms: raw.output_transformer.secondary_ohms.clone(),
            dc_primary_winding_ohms: raw.output_transformer.dc_primary_winding_ohms,
            dc_secondary_8ohm_winding_ohms: raw.output_transformer.dc_secondary_8ohm_winding_ohms,
            dc_secondary_16ohm_winding_ohms: raw.output_transformer.dc_secondary_16ohm_winding_ohms,
            turns_ratio_primary_to_8ohm: raw.output_transformer.nominal_turns_ratio_primary_to_8ohm,
            turns_ratio_primary_to_16ohm: raw
                .output_transformer
                .nominal_turns_ratio_primary_to_16ohm,
            winding_inductance_h: raw.output_transformer.winding_inductance_h,
            leakage_inductance_h: raw.output_transformer.leakage_inductance_h,
            parasitic_capacitance_f: raw.output_transformer.parasitic_capacitance_f,
            // Parásitos del OT-471 (B18-013): no están en el JSON canónico;
            // llegan del overlay de calibración (`estimated`).
            core_loss_ohms: None,
            magnetizing_current_limit_a: None,
            negative_feedback: NegativeFeedbackValues {
                present: raw.output_transformer.negative_feedback.present,
                resistor_from_secondary_ohms_visible: raw
                    .output_transformer
                    .negative_feedback
                    .resistor_from_secondary_ohms_visible,
                topology_note: raw
                    .output_transformer
                    .negative_feedback
                    .other_components_or_topology
                    .clone(),
            },
        },
        power_supply: PowerSupplyValues {
            rectifier: raw.power_supply.rectifier.clone(),
            power_transformer_models: raw.power_supply.power_transformer_models.clone(),
            mains_primary_us_vac: raw.power_supply.mains_primary_us_vac,
            mains_primary_export_vac: raw.power_supply.mains_primary_export_vac,
            mains_frequencies_hz: raw.power_supply.mains_frequency_hz.clone(),
            hv_secondary_vac_center_tapped_total: raw
                .power_supply
                .high_voltage_secondary_vac_center_tapped_total,
            hv_dc_current_rating_a: raw.power_supply.high_voltage_dc_current_rating_a,
            bplus_idle_vdc: raw.power_supply.bplus_idle_vdc,
            fixed_bias_nominal_vdc: raw.power_supply.fixed_bias_nominal_vdc,
            hv_filter_capacitors_farads: raw
                .power_supply
                .high_voltage_filter_capacitors_farads
                .clone(),
            bias_filter_capacitor_farads: raw.power_supply.bias_filter_capacitor_farads,
            bias_filter_voltage_rating_v: raw.power_supply.bias_filter_voltage_rating_v,
            rectifier_diode_bias: raw.power_supply.rectifier_diode_bias.clone(),
            dropping_resistor_values_ohms: raw
                .power_supply
                .dropping_resistor_values_seen_ohms
                .clone(),
            // La dinámica B18-012 llega del overlay `supply.*` (estimated).
            rectifier_series_ohms: None,
            bias_series_ohms: None,
        },
        speaker: SpeakerValues {
            nominal_diameter_inches: raw.cabinet.nominal_speaker_diameter_inches,
            nominal_load_ohms: raw.cabinet.nominal_load_ohms,
            cabinet_type: raw.cabinet.cabinet_type.clone(),
            impedance_curve_available: raw.cabinet.speaker_impedance_curve.is_some(),
            measured_ir_available: raw.cabinet.measured_ir.is_some(),
            // El equivalente RLC estimado del altavoz (B18-015) llega del
            // overlay `speaker.reactive_load.*`; el JSON canónico no lo trae.
            lumped_equivalent: None,
        },
        // Los modelos de válvula llegan del overlay de calibración
        // (`apply_embedded_calibration` o `CalibrationFile::apply_to`).
        tube_models: TubeModelValues::default(),
        // La topología aún no se ha contrastado nodo a nodo contra el esquema.
        fidelity: FidelityLevel::TopologyUnverified,
        missing: missing_values(raw),
    };

    Ok(circuit)
}

fn pot(spec: &crate::schema::PotSpec) -> PotValues {
    PotValues {
        max_ohms: spec.ohms,
        law: match spec.law {
            PotLawJson::Linear => PotLaw::Linear,
            PotLawJson::Logarithmic => PotLaw::Logarithmic,
        },
    }
}

fn missing_values(raw: &HistoricalB18nJson) -> Vec<MissingValue> {
    let fields = uncharacterized_fields(raw);
    fields
        .iter()
        .filter_map(|field| note_for(field).map(|note| MissingValue { field, note }))
        .collect()
}

fn note_for(field: &'static str) -> Option<&'static str> {
    Some(match field {
        "preamp.tube_6sl7gt_characteristic_curves" => {
            "curvas de válvula no documentadas; usar modelo simplificado con limitación explícita"
        }
        "power_stage.plate_current_idle_a_each" => {
            "corriente de reposo de placa sin caracterizar; calibrar con datos externos"
        }
        "power_stage.screen_current_idle_a_each" => {
            "corriente de reposo de pantalla sin caracterizar"
        }
        "power_stage.grid_conduction_model" => {
            "modelo de conducción de rejilla sin datos; blocking distortion no validada"
        }
        "power_stage.tube_characteristic_curves" => "curvas de la 7027A no documentadas",
        "output_transformer.winding_inductance_h" => {
            "inductancia de magnetización no medida; OT ideal en la primera etapa"
        }
        "output_transformer.leakage_inductance_h" => "inductancia de fuga no medida",
        "output_transformer.parasitic_capacitance_f" => "capacitancias parásitas no medidas",
        "output_transformer.core_saturation_curve" => "curva de saturación del núcleo no medida",
        "cabinet.speaker_impedance_curve" => {
            "impedancia compleja del altavoz no medida; afecta potencia y NFB"
        }
        "cabinet.speaker_thiele_small" => "parámetros Thiele-Small no medidos",
        "cabinet.cabinet_3d_geometry" => "geometría del gabinete no digitalizada",
        "cabinet.measured_ir" => "IR no medida; gabinete opcional y externo",
        "cabinet.measured_frequency_response" => "respuesta en frecuencia no medida",
        _ => return None,
    })
}
