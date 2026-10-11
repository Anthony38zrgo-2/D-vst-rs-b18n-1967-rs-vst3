//! Validación estricta del JSON histórico.
//!
//! Comprueba coherencia y unidades sin inventar valores: si el documento no
//! cumple lo esperado para esta revisión, el loader falla con un error tipado.

use crate::schema::{
    Controls, HistoricalB18nJson, PhaseInverter, PotLawJson, PowerStage, PowerSupply, Preamp,
    ToneStack,
};

/// Revisión exigida por este proyecto (B-18N/B-15ND, mayo de 1967).
pub const EXPECTED_SCHEMATIC_REVISION: &str = "1967-05";

/// Valida el documento completo. Devuelve el primer problema encontrado.
pub fn validate_source(raw: &HistoricalB18nJson) -> Result<(), crate::ConfigError> {
    if raw.identity.schematic_revision != EXPECTED_SCHEMATIC_REVISION {
        return Err(crate::ConfigError::UnexpectedRevision {
            expected: EXPECTED_SCHEMATIC_REVISION,
            found: raw.identity.schematic_revision.clone(),
        });
    }

    if raw.identity.manufacturer.trim().is_empty() || raw.identity.model.trim().is_empty() {
        return Err(invalid("identity", "fabricante o modelo vacíos"));
    }
    if raw.identity.documented_nominal_output_w <= 0.0 {
        return Err(invalid(
            "identity.documented_nominal_output_w",
            "potencia nominal no positiva",
        ));
    }

    validate_controls(&raw.controls)?;
    validate_preamp(&raw.preamp)?;
    validate_tone_stack(&raw.tone_stack)?;
    validate_phase_inverter(&raw.phase_inverter)?;
    validate_power_stage(&raw.power_stage)?;
    validate_output_transformer(&raw.output_transformer)?;
    validate_power_supply(&raw.power_supply)?;
    validate_cabinet(&raw.cabinet)?;
    validate_validation_block(raw)?;

    Ok(())
}

/// Campos que el JSON deja sin caracterizar (`null`) y que ningún módulo debe
/// completar con valores ocultos.
#[must_use]
pub fn uncharacterized_fields(raw: &HistoricalB18nJson) -> Vec<&'static str> {
    let mut fields = Vec::new();
    if raw.preamp.tube_6sl7gt_characteristic_curves.is_none() {
        fields.push("preamp.tube_6sl7gt_characteristic_curves");
    }
    if raw.power_stage.plate_current_idle_a_each.is_none() {
        fields.push("power_stage.plate_current_idle_a_each");
    }
    if raw.power_stage.screen_current_idle_a_each.is_none() {
        fields.push("power_stage.screen_current_idle_a_each");
    }
    if raw.power_stage.grid_conduction_model.is_none() {
        fields.push("power_stage.grid_conduction_model");
    }
    if raw.power_stage.tube_characteristic_curves.is_none() {
        fields.push("power_stage.tube_characteristic_curves");
    }
    if raw.output_transformer.winding_inductance_h.is_none() {
        fields.push("output_transformer.winding_inductance_h");
    }
    if raw.output_transformer.leakage_inductance_h.is_none() {
        fields.push("output_transformer.leakage_inductance_h");
    }
    if raw.output_transformer.parasitic_capacitance_f.is_none() {
        fields.push("output_transformer.parasitic_capacitance_f");
    }
    if raw.output_transformer.core_saturation_curve.is_none() {
        fields.push("output_transformer.core_saturation_curve");
    }
    if raw.cabinet.speaker_impedance_curve.is_none() {
        fields.push("cabinet.speaker_impedance_curve");
    }
    if raw.cabinet.speaker_thiele_small.is_none() {
        fields.push("cabinet.speaker_thiele_small");
    }
    if raw.cabinet.cabinet_3d_geometry.is_none() {
        fields.push("cabinet.cabinet_3d_geometry");
    }
    if raw.cabinet.measured_ir.is_none() {
        fields.push("cabinet.measured_ir");
    }
    if raw.cabinet.measured_frequency_response.is_none() {
        fields.push("cabinet.measured_frequency_response");
    }
    fields
}

fn validate_controls(controls: &Controls) -> Result<(), crate::ConfigError> {
    if controls.channels != 2 {
        return Err(invalid("controls.channels", "se esperaban 2 canales"));
    }
    validate_pot(
        "controls.each_channel.volume",
        &controls.each_channel.volume,
        PotLawJson::Logarithmic,
    )?;
    validate_pot(
        "controls.each_channel.bass",
        &controls.each_channel.bass,
        PotLawJson::Linear,
    )?;
    validate_pot(
        "controls.each_channel.treble",
        &controls.each_channel.treble,
        PotLawJson::Linear,
    )?;
    if controls.hum_balance.ohms <= 0.0 || controls.hum_balance.rating_w <= 0.0 {
        return Err(invalid(
            "controls.hum_balance",
            "valor óhmico o potencia no positivos",
        ));
    }
    if controls.potentiometer_tolerance_percent < 0.0 {
        return Err(invalid(
            "controls.potentiometer_tolerance_percent",
            "tolerancia negativa",
        ));
    }
    Ok(())
}

fn validate_pot(
    field: &'static str,
    pot: &crate::schema::PotSpec,
    expected_law: PotLawJson,
) -> Result<(), crate::ConfigError> {
    if pot.ohms <= 0.0 {
        return Err(invalid(field, "resistencia no positiva"));
    }
    if pot.law != expected_law {
        return Err(invalid(
            field,
            "ley del potenciómetro distinta de la documentada",
        ));
    }
    Ok(())
}

fn validate_preamp(preamp: &Preamp) -> Result<(), crate::ConfigError> {
    let ch = &preamp.each_channel;
    for (field, value) in [
        (
            "preamp.each_channel.triode_1_plate_resistor_ohms",
            ch.triode_1_plate_resistor_ohms,
        ),
        (
            "preamp.each_channel.triode_1_cathode_resistor_ohms",
            ch.triode_1_cathode_resistor_ohms,
        ),
        (
            "preamp.each_channel.coupling_cap_1_farads",
            ch.coupling_cap_1_farads,
        ),
        (
            "preamp.each_channel.triode_2_plate_resistor_ohms",
            ch.triode_2_plate_resistor_ohms,
        ),
        (
            "preamp.each_channel.triode_2_cathode_resistor_ohms",
            ch.triode_2_cathode_resistor_ohms,
        ),
        (
            "preamp.each_channel.coupling_cap_2_farads",
            ch.coupling_cap_2_farads,
        ),
        (
            "preamp.each_channel.channel_mixer_resistor_ohms",
            ch.channel_mixer_resistor_ohms,
        ),
    ] {
        if !(value.is_finite() && value > 0.0) {
            return Err(invalid(field, "debe ser finito y positivo"));
        }
    }
    if preamp.input_network_values_visible_ohms.is_empty() {
        return Err(invalid(
            "preamp.input_network_values_visible_ohms",
            "lista vacía",
        ));
    }
    let reference = &preamp.tube_6sl7gt_reference_at_250v;
    if reference.mu_typ <= 0.0 || reference.rp_ohms_typ <= 0.0 || reference.gm_s_typ <= 0.0 {
        return Err(invalid(
            "preamp.tube_6sl7gt_reference_at_250v",
            "referencia de válvula no positiva",
        ));
    }
    Ok(())
}

fn validate_tone_stack(tone_stack: &ToneStack) -> Result<(), crate::ConfigError> {
    if tone_stack.printed_board_resistors_ohms.is_empty()
        || tone_stack.printed_board_capacitors_farads.is_empty()
    {
        return Err(invalid("tone_stack", "listas de componentes vacías"));
    }
    if tone_stack.fixed_bass_frequency_hz.is_some()
        || tone_stack.fixed_treble_frequency_hz.is_some()
    {
        return Err(invalid(
            "tone_stack.fixed_bass_frequency_hz/fixed_treble_frequency_hz",
            "la red documentada no define frecuencias fijas de ecualización",
        ));
    }
    Ok(())
}

fn validate_phase_inverter(pi: &PhaseInverter) -> Result<(), crate::ConfigError> {
    for (field, value) in [
        (
            "phase_inverter.pentode_plate_resistor_ohms",
            pi.pentode_plate_resistor_ohms,
        ),
        (
            "phase_inverter.pentode_screen_feed_resistor_ohms",
            pi.pentode_screen_feed_resistor_ohms,
        ),
        (
            "phase_inverter.pentode_cathode_resistor_ohms",
            pi.pentode_cathode_resistor_ohms,
        ),
        (
            "phase_inverter.pentode_to_triode_coupling_farads",
            pi.pentode_to_triode_coupling_farads,
        ),
    ] {
        if !(value.is_finite() && value > 0.0) {
            return Err(invalid(field, "debe ser finito y positivo"));
        }
    }
    if pi.large_resistors_ohms.len() != 2 || pi.phase_splitter_resistors_ohms.len() != 2 {
        return Err(invalid(
            "phase_inverter.large_resistors_ohms/phase_splitter_resistors_ohms",
            "se esperaban dos resistencias en cada grupo",
        ));
    }
    if pi.dc_pin_volts_no_signal.is_empty() {
        return Err(invalid(
            "phase_inverter.dc_pin_volts_no_signal",
            "sin tensiones de referencia",
        ));
    }
    if pi
        .dc_pin_volts_no_signal
        .values()
        .any(|volts| !volts.is_finite())
    {
        return Err(invalid(
            "phase_inverter.dc_pin_volts_no_signal",
            "tensión no finita",
        ));
    }
    for resistor in &pi.additional_traced_resistors {
        if !(resistor.ohms.is_finite() && resistor.ohms > 0.0) {
            return Err(invalid(
                "phase_inverter.additional_traced_resistors.ohms",
                "resistor trazado no finito o no positivo",
            ));
        }
        if resistor.from_node.trim().is_empty()
            || resistor.to_node.trim().is_empty()
            || resistor.evidence.trim().is_empty()
        {
            return Err(invalid(
                "phase_inverter.additional_traced_resistors",
                "componente trazado sin nodos o sin evidencia",
            ));
        }
    }
    if let Some(supply_v) = pi.triode_supply_v {
        if !(supply_v.is_finite() && supply_v > 0.0) {
            return Err(invalid(
                "phase_inverter.triode_supply_v",
                "carril del triodo no finito o no positivo",
            ));
        }
    }
    Ok(())
}

fn validate_power_stage(stage: &PowerStage) -> Result<(), crate::ConfigError> {
    if stage.tubes.len() != 2 || stage.tubes.iter().any(|tube| tube != "7027A") {
        return Err(invalid("power_stage.tubes", "se esperaban dos 7027A"));
    }
    if !(stage.plate_voltage_v_nominal > 0.0 && stage.screen_voltage_v_nominal > 0.0) {
        return Err(invalid(
            "power_stage.plate_voltage_v_nominal/screen_voltage_v_nominal",
            "tensiones de placa/pantalla no positivas",
        ));
    }
    if stage.control_grid_voltage_v_nominal >= 0.0 {
        return Err(invalid(
            "power_stage.control_grid_voltage_v_nominal",
            "el bias fijo documentado es negativo",
        ));
    }
    for (field, value) in [
        (
            "power_stage.grid_stopper_ohms_each",
            stage.grid_stopper_ohms_each,
        ),
        (
            "power_stage.grid_return_ohms_each",
            stage.grid_return_ohms_each,
        ),
        (
            "power_stage.inverter_to_grid_coupling_farads_each",
            stage.inverter_to_grid_coupling_farads_each,
        ),
    ] {
        if !(value.is_finite() && value > 0.0) {
            return Err(invalid(field, "debe ser finito y positivo"));
        }
    }
    Ok(())
}

fn validate_output_transformer(
    transformer: &crate::schema::OutputTransformer,
) -> Result<(), crate::ConfigError> {
    if transformer.primary_plate_to_plate_ohms_nominal <= 0.0 {
        return Err(invalid(
            "output_transformer.primary_plate_to_plate_ohms_nominal",
            "impedancia primaria no positiva",
        ));
    }
    for required in [8.0_f64, 16.0] {
        if !transformer.secondary_ohms.contains(&required) {
            return Err(invalid(
                "output_transformer.secondary_ohms",
                "faltan las tomas de 8 Ω o 16 Ω",
            ));
        }
    }
    if transformer.nominal_turns_ratio_primary_to_8ohm <= 1.0
        || transformer.nominal_turns_ratio_primary_to_16ohm <= 1.0
    {
        return Err(invalid(
            "output_transformer.nominal_turns_ratio_*",
            "relaciones de transformación no plausibles",
        ));
    }
    if !transformer.negative_feedback.present
        || transformer
            .negative_feedback
            .resistor_from_secondary_ohms_visible
            <= 0.0
    {
        return Err(invalid(
            "output_transformer.negative_feedback",
            "la revisión 1967-05 documenta realimentación presente",
        ));
    }
    Ok(())
}

fn validate_power_supply(supply: &PowerSupply) -> Result<(), crate::ConfigError> {
    if supply.rectifier.trim().is_empty() {
        return Err(invalid("power_supply.rectifier", "vacío"));
    }
    if !(100.0..=1000.0).contains(&supply.bplus_idle_vdc) {
        return Err(invalid(
            "power_supply.bplus_idle_vdc",
            "B+ fuera del rango esperado para la revisión",
        ));
    }
    if !(-200.0..=-1.0).contains(&supply.fixed_bias_nominal_vdc) {
        return Err(invalid(
            "power_supply.fixed_bias_nominal_vdc",
            "el bias fijo debe ser negativo",
        ));
    }
    if supply.high_voltage_filter_capacitors_farads.is_empty() {
        return Err(invalid(
            "power_supply.high_voltage_filter_capacitors_farads",
            "sin capacitores de filtrado",
        ));
    }
    if supply.mains_frequency_hz.iter().any(|hz| *hz <= 0.0) {
        return Err(invalid(
            "power_supply.mains_frequency_hz",
            "frecuencia no positiva",
        ));
    }
    Ok(())
}

fn validate_cabinet(cabinet: &crate::schema::Cabinet) -> Result<(), crate::ConfigError> {
    if cabinet.nominal_load_ohms <= 0.0 || cabinet.nominal_speaker_diameter_inches <= 0.0 {
        return Err(invalid("cabinet", "carga o diámetro no positivos"));
    }
    Ok(())
}

fn validate_validation_block(raw: &HistoricalB18nJson) -> Result<(), crate::ConfigError> {
    let validation = &raw.validation;
    if validation.reported_dc_meter_loading_ohms_per_volt <= 0.0 {
        return Err(invalid(
            "validation.reported_dc_meter_loading_ohms_per_volt",
            "carga del instrumento no positiva",
        ));
    }
    if validation.dc_voltage_tolerance_percent_reported <= 0.0
        || validation.resistor_tolerance_percent_general <= 0.0
    {
        return Err(invalid(
            "validation.*_tolerance_percent_*",
            "tolerancias no positivas",
        ));
    }
    Ok(())
}

fn invalid(field: &'static str, reason: &'static str) -> crate::ConfigError {
    crate::ConfigError::InvalidValue { field, reason }
}
