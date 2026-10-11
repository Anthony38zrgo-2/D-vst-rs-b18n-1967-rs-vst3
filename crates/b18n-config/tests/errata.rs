//! Pruebas de la errata histórica: el canónico no se edita y la corrección
//! llega al `CircuitDefinition` con guardas de hash y deriva.

use b18n_config::{
    convert_to_circuit, load_reference, parse_reference, sha256_hex, ConfigError, HistoricalErrata,
    EMBEDDED_ERRATA_JSON, EMBEDDED_REFERENCE_JSON, EXPECTED_REFERENCE_SHA256,
};

fn loaded_plate_resistor() -> f64 {
    load_reference()
        .expect("el modelo embebido debe cargar con errata")
        .phase_inverter
        .pentode_plate_resistor_ohms
}

fn loaded_screen_resistor() -> f64 {
    load_reference()
        .expect("el modelo embebido debe cargar con errata")
        .phase_inverter
        .pentode_screen_feed_resistor_ohms
}

#[test]
fn loaded_circuit_uses_corrected_pin2_and_pin3_values() {
    assert_eq!(loaded_plate_resistor(), 270_000.0, "pin 2 (placa) = 270 kΩ");
    assert_eq!(
        loaded_screen_resistor(),
        470_000.0,
        "pin 3 (pantalla) = 470 kΩ"
    );
}

#[test]
fn raw_canonical_reference_keeps_historical_values() {
    let raw = parse_reference(EMBEDDED_REFERENCE_JSON).expect("canónico válido");
    let circuit = convert_to_circuit(&raw).expect("mapeo");
    assert_eq!(
        circuit.phase_inverter.pentode_plate_resistor_ohms,
        470_000.0
    );
    assert_eq!(
        circuit.phase_inverter.pentode_screen_feed_resistor_ohms,
        270_000.0
    );
}

#[test]
fn errata_is_bound_to_the_canonical_hash() {
    let errata = HistoricalErrata::parse(EMBEDDED_ERRATA_JSON).expect("errata embebida válida");
    let tampered = EMBEDDED_REFERENCE_JSON.replace("Portaflex", "Portaflux");
    assert!(matches!(
        errata.apply_to_json_str(&tampered),
        Err(ConfigError::Errata { .. })
    ));
}

#[test]
fn errata_detects_canonical_drift() {
    let drifted = EMBEDDED_ERRATA_JSON.replace("\"json_value\": 470000", "\"json_value\": 1");
    let errata =
        HistoricalErrata::parse(&drifted).expect("la errata parsea; la deriva se ve al aplicar");
    assert!(matches!(
        errata.apply_to_json_str(EMBEDDED_REFERENCE_JSON),
        Err(ConfigError::Errata { .. })
    ));
}

#[test]
fn effective_json_differs_from_canonical_and_is_traceable() {
    assert_eq!(
        sha256_hex(EMBEDDED_REFERENCE_JSON.as_bytes()),
        EXPECTED_REFERENCE_SHA256
    );
    let effective =
        b18n_config::apply_embedded_errata(EMBEDDED_REFERENCE_JSON).expect("la errata aplica");
    assert_ne!(
        sha256_hex(effective.as_bytes()),
        EXPECTED_REFERENCE_SHA256,
        "el JSON efectivo debe diferir del canónico"
    );
    assert_eq!(sha256_hex(EMBEDDED_ERRATA_JSON.as_bytes()).len(), 64);
}

#[test]
fn traced_screen_resistor_reaches_the_circuit() {
    let circuit = load_reference().expect("el modelo embebido debe cargar");
    let traced = &circuit.phase_inverter.additional_traced_resistors;
    assert_eq!(traced.len(), 1, "una adición trazada (150 kΩ de pantalla)");
    assert_eq!(traced[0].ohms, 150_000.0);
    assert!(traced[0].from_node.contains("pin 3"));
    assert!(traced[0].to_node.contains("masa"));
    assert!(!traced[0].evidence.is_empty());
}

#[test]
fn raw_canonical_has_no_traced_resistors() {
    let raw = parse_reference(EMBEDDED_REFERENCE_JSON).expect("canónico válido");
    let circuit = convert_to_circuit(&raw).expect("mapeo");
    assert!(circuit
        .phase_inverter
        .additional_traced_resistors
        .is_empty());
    assert_eq!(circuit.phase_inverter.triode_supply_v, None);
}

#[test]
fn traced_triode_supply_reaches_the_circuit() {
    let circuit = load_reference().expect("carga");
    assert_eq!(circuit.phase_inverter.triode_supply_v, Some(460.0));
}

#[test]
fn traced_screen_resistor_matches_spice_golden() {
    const GOLDEN: &str =
        include_str!("../../../assets/fixtures/reference/phase_7199/b18n_7199_dc_1967_golden.txt");
    let expected: f64 = GOLDEN
        .lines()
        .find_map(|line| line.strip_prefix("variant_A_off_floating_rail_ohms = "))
        .expect("golden de la variante A")
        .trim()
        .parse()
        .expect("valor numérico");
    let circuit = load_reference().expect("carga");
    assert_eq!(
        circuit.phase_inverter.additional_traced_resistors[0].ohms,
        expected
    );
}
