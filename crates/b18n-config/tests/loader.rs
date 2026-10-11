//! Pruebas de aceptación del loader (Hito A).

use b18n_config::{
    load_reference, parse_reference, uncharacterized_fields, CircuitDefinition, ConfigError,
    EMBEDDED_REFERENCE_JSON, EXPECTED_REFERENCE_SHA256, EXPECTED_SCHEMATIC_REVISION,
};
use sha2::{Digest, Sha256};

fn canonical() -> CircuitDefinition {
    load_reference().expect("el JSON canónico debe cargar")
}

#[test]
fn canonical_json_sha256_is_unchanged() {
    let digest = Sha256::digest(EMBEDDED_REFERENCE_JSON.as_bytes());
    assert_eq!(
        format!("{digest:x}"),
        EXPECTED_REFERENCE_SHA256,
        "el JSON canónico no debe editarse: toda corrección documentada va al archivo de errata"
    );
}

#[test]
fn loader_accepts_canonical_reference() {
    let circuit = canonical();
    assert_eq!(
        circuit.identity.schematic_revision,
        EXPECTED_SCHEMATIC_REVISION
    );
    assert_eq!(circuit.identity.model, "B-18N Portaflex");
    assert_eq!(circuit.channel.channel_mixer_resistor_ohms, 270_000.0);
    assert_eq!(circuit.power_stage.plate_voltage_v_nominal, 505.0);
    assert_eq!(circuit.power_supply.bplus_idle_vdc, 505.0);
    assert_eq!(
        circuit
            .output_transformer
            .primary_plate_to_plate_ohms_nominal,
        4_300.0
    );
    assert_eq!(circuit.speaker.nominal_load_ohms, 8.0);
    assert!(!circuit.missing.is_empty());
}

#[test]
fn loader_rejects_wrong_revision() {
    let modified = EMBEDDED_REFERENCE_JSON.replace("1967-05", "1966-01");
    let error = parse_reference(&modified).expect_err("revisión incorrecta debe fallar");
    match error {
        ConfigError::UnexpectedRevision { expected, found } => {
            assert_eq!(expected, "1967-05");
            assert_eq!(found, "1966-01");
        }
        other => panic!("error inesperado: {other}"),
    }
}

#[test]
fn loader_rejects_unknown_fields() {
    let modified = EMBEDDED_REFERENCE_JSON.replace(
        "\"identity\": {",
        "\"identity\": {\"unexpected_key\": true,",
    );
    assert!(matches!(
        parse_reference(&modified),
        Err(ConfigError::Json(_))
    ));
}

#[test]
fn loader_rejects_non_documented_pot_law() {
    let modified = EMBEDDED_REFERENCE_JSON.replace(
        "\"volume\": {\"ohms\": 1000000, \"law\": \"logarithmic\"}",
        "\"volume\": {\"ohms\": 1000000, \"law\": \"linear\"}",
    );
    assert!(matches!(
        parse_reference(&modified),
        Err(ConfigError::InvalidValue { .. })
    ));
}

#[test]
fn loader_detects_null_fields() {
    let raw = parse_reference(EMBEDDED_REFERENCE_JSON).expect("JSON canónico válido");
    let fields = uncharacterized_fields(&raw);
    assert!(fields.contains(&"power_stage.plate_current_idle_a_each"));
    assert!(fields.contains(&"cabinet.measured_ir"));
    assert!(fields.contains(&"output_transformer.winding_inductance_h"));
    assert_eq!(fields.len(), 14);
}

#[test]
fn mapped_missing_values_cover_all_nulls() {
    let raw = parse_reference(EMBEDDED_REFERENCE_JSON).expect("JSON canónico válido");
    let circuit = canonical();
    assert_eq!(circuit.missing.len(), uncharacterized_fields(&raw).len());
    for missing in &circuit.missing {
        assert!(!missing.note.is_empty(), "sin nota para {}", missing.field);
    }
}

#[test]
fn fidelity_starts_unverified() {
    let circuit = canonical();
    assert_eq!(
        circuit.fidelity.as_str(),
        "topology_unverified",
        "la topología aún no está contrastada nodo a nodo"
    );
}

#[test]
fn calibration_includes_7199_models() {
    let circuit = canonical();
    let pentode = circuit
        .tube_models
        .koren_7199_pentode
        .expect("pentodo 7199 en la calibración");
    assert!((pentode.mu - 22.279_549).abs() < 1e-6);
    assert!((pentode.kg2 - 3_807.560_112).abs() < 1e-6);
    assert!((pentode.kvb - 12.522_225).abs() < 1e-6);
    let triode = circuit
        .tube_models
        .koren_7199_triode
        .expect("triodo 7199 en la calibración");
    assert!((triode.mu - 17.0).abs() < 1e-6);
    assert!((triode.kvb - 160.0).abs() < 1e-6);
}
