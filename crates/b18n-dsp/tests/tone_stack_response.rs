//! Contraste de la red de tono contra una implementación independiente.
//!
//! La referencia `tone_stack_numpy.csv` la genera
//! `scripts/tone_stack_reference.py` con numpy (solver `numpy.linalg.solve`)
//! sobre el mismo netlist. Es el sustituto declarado de SPICE hasta poder
//! ejecutar ngspice del mismo netlist (véase `docs/MEASUREMENT_PROTOCOL.md`).

use b18n_dsp::stages::tone_stack::{ToneStackControls, ToneStackDesign};

const REFERENCE_CSV: &str =
    include_str!("../../../assets/fixtures/reference/tone_stack/tone_stack_numpy.csv");

const SPICE_CSV: &str =
    include_str!("../../../assets/fixtures/reference/tone_stack/tone_stack_spice.csv");

/// Acoplo de placa del 6SL7#2 dentro del MNA del tono (B18-003).
const COUPLED_SPICE_CSV: &str =
    include_str!("../../../assets/fixtures/reference/tone_stack/tone_stack_coupled_spice.csv");
const COUPLING_FARADS: f64 = 2.2e-8;

const SOURCE_OHMS: f64 = 100_000.0;
const LOAD_OHMS: f64 = 270_000.0;

const EXPECTED_ROWS: usize = 25 * 31;

#[test]
fn matches_independent_numpy_reference() {
    let circuit = b18n_config::load_reference().expect("JSON canónico válido");
    let design = ToneStackDesign::from_circuit(&circuit).expect("red de tono completa");

    let mut compared = 0_usize;
    let mut worst_magnitude_db = 0.0_f64;
    let mut worst_phase_deg = 0.0_f64;
    let mut last_frequency = 0.0_f64;

    for (index, line) in REFERENCE_CSV.lines().enumerate() {
        if index == 0 {
            continue; // cabecera
        }
        let mut fields = line.split(',');
        let parse = |field: Option<&str>| -> f64 {
            field
                .expect("campo presente")
                .trim()
                .parse()
                .expect("campo numérico")
        };
        let bass = parse(fields.next()) as f32;
        let treble = parse(fields.next()) as f32;
        let frequency = parse(fields.next());
        let expected_magnitude_db = parse(fields.next());
        let expected_phase_deg = parse(fields.next());

        let response = design
            .frequency_response(
                ToneStackControls { bass, treble },
                frequency,
                SOURCE_OHMS,
                LOAD_OHMS,
            )
            .expect("sistema resoluble");

        let magnitude_error = (response.magnitude_db() - expected_magnitude_db).abs();
        let phase_error = (response.phase_deg() - expected_phase_deg).abs();

        worst_magnitude_db = worst_magnitude_db.max(magnitude_error);
        worst_phase_deg = worst_phase_deg.max(phase_error);
        last_frequency = frequency;
        compared += 1;

        assert!(
            magnitude_error < 1e-6,
            "magnitud fuera de tolerancia en bass={bass} treble={treble} f={frequency}: \
             {magnitude_error} dB"
        );
        assert!(
            phase_error < 1e-4,
            "fase fuera de tolerancia en bass={bass} treble={treble} f={frequency}: \
             {phase_error}°"
        );
    }

    assert_eq!(compared, EXPECTED_ROWS, "referencia incompleta");
    assert!(last_frequency >= 20_000.0);
    assert!(worst_magnitude_db < 1e-6);
    assert!(worst_phase_deg < 1e-4);
}

/// Contraste contra ngspice 47 del mismo netlist y condiciones
/// (`scripts/spice/tone_stack_sweep.py`, golden `tone_stack_spice.csv`).
///
/// La referencia SPICE es independiente del solver Rust y se regenera con
/// `python scripts/spice/tone_stack_sweep.py`.
#[test]
fn matches_ngspice_reference() {
    let circuit = b18n_config::load_reference().expect("JSON canónico válido");
    let design = ToneStackDesign::from_circuit(&circuit).expect("red de tono completa");

    let mut compared = 0_usize;
    let mut worst_magnitude_db = 0.0_f64;
    let mut worst_phase_deg = 0.0_f64;

    for (index, line) in SPICE_CSV.lines().enumerate() {
        if index == 0 {
            continue; // cabecera
        }
        let mut fields = line.split(',');
        let parse = |field: Option<&str>| -> f64 {
            field
                .expect("campo presente")
                .trim()
                .parse()
                .expect("campo numérico")
        };
        let bass = parse(fields.next()) as f32;
        let treble = parse(fields.next()) as f32;
        let frequency = parse(fields.next());
        let expected_magnitude_db = parse(fields.next());
        let expected_phase_deg = parse(fields.next());

        let response = design
            .frequency_response(
                ToneStackControls { bass, treble },
                frequency,
                SOURCE_OHMS,
                LOAD_OHMS,
            )
            .expect("sistema resoluble");

        let magnitude_error = (response.magnitude_db() - expected_magnitude_db).abs();
        let phase_error = (response.phase_deg() - expected_phase_deg).abs();
        worst_magnitude_db = worst_magnitude_db.max(magnitude_error);
        worst_phase_deg = worst_phase_deg.max(phase_error);
        compared += 1;

        assert!(
            magnitude_error < 1e-6,
            "SPICE: magnitud fuera de tolerancia en bass={bass} treble={treble} \
             f={frequency}: {magnitude_error} dB"
        );
        assert!(
            phase_error < 1e-4,
            "SPICE: fase fuera de tolerancia en bass={bass} treble={treble} \
             f={frequency}: {phase_error}°"
        );
    }

    assert_eq!(compared, EXPECTED_ROWS, "referencia SPICE incompleta");
    assert!(worst_magnitude_db < 1e-6);
    assert!(worst_phase_deg < 1e-4);
}

/// Contraste contra ngspice del **acoplo de placa + red de tono** (B18-003):
/// fuente de 1 V tras 100 kΩ, capacitor de 0.022 µF y la misma red con carga
/// de 270 kΩ. Golden `tone_stack_coupled_spice.csv` generado por
/// `scripts/spice/tone_stack_coupled_sweep.py`; el fixture aislado anterior
/// se conserva sin cambios.
#[test]
fn matches_ngspice_coupled_reference() {
    let circuit = b18n_config::load_reference().expect("JSON canónico válido");
    let design = ToneStackDesign::from_circuit(&circuit).expect("red de tono completa");

    let mut compared = 0_usize;
    let mut worst_magnitude_db = 0.0_f64;
    let mut worst_phase_deg = 0.0_f64;

    for (index, line) in COUPLED_SPICE_CSV.lines().enumerate() {
        if index == 0 {
            continue; // cabecera
        }
        let mut fields = line.split(',');
        let parse = |field: Option<&str>| -> f64 {
            field
                .expect("campo presente")
                .trim()
                .parse()
                .expect("campo numérico")
        };
        let bass = parse(fields.next()) as f32;
        let treble = parse(fields.next()) as f32;
        let frequency = parse(fields.next());
        let expected_magnitude_db = parse(fields.next());
        let expected_phase_deg = parse(fields.next());

        let response = design
            .frequency_response_with_input_coupling(
                ToneStackControls { bass, treble },
                frequency,
                SOURCE_OHMS,
                COUPLING_FARADS,
                LOAD_OHMS,
            )
            .expect("sistema resoluble");

        let magnitude_error = (response.magnitude_db() - expected_magnitude_db).abs();
        let phase_error = (response.phase_deg() - expected_phase_deg).abs();
        worst_magnitude_db = worst_magnitude_db.max(magnitude_error);
        worst_phase_deg = worst_phase_deg.max(phase_error);
        compared += 1;

        assert!(
            magnitude_error < 1e-6,
            "SPICE acoplado: magnitud fuera de tolerancia en bass={bass} treble={treble} \
             f={frequency}: {magnitude_error} dB"
        );
        assert!(
            phase_error < 1e-4,
            "SPICE acoplado: fase fuera de tolerancia en bass={bass} treble={treble} \
             f={frequency}: {phase_error}°"
        );
    }

    assert_eq!(
        compared, EXPECTED_ROWS,
        "referencia SPICE acoplada incompleta"
    );
    assert!(worst_magnitude_db < 1e-6);
    assert!(worst_phase_deg < 1e-4);
}

#[test]
fn tone_controls_span_documented_directions() {
    let circuit = b18n_config::load_reference().expect("JSON canónico válido");
    let design = ToneStackDesign::from_circuit(&circuit).expect("red de tono completa");
    let at = |bass: f32, treble: f32, frequency: f64| {
        design
            .frequency_response(
                ToneStackControls { bass, treble },
                frequency,
                SOURCE_OHMS,
                LOAD_OHMS,
            )
            .expect("sistema resoluble")
            .magnitude_db()
    };

    // Las cifras publicadas para el canal 1966 de la reedición B-15N
    // (+18/-30 dB @5 kHz, ±11 dB @40 Hz) no coinciden exactamente con esta
    // revisión; se verifican aquí solo las direcciones y rangos amplios.
    let bass_boost = at(1.0, 0.5, 40.0) - at(0.5, 0.5, 40.0);
    let bass_cut = at(0.5, 0.5, 40.0) - at(0.0, 0.5, 40.0);
    let treble_boost = at(0.5, 1.0, 5_000.0) - at(0.5, 0.5, 5_000.0);
    let treble_cut = at(0.5, 0.5, 5_000.0) - at(0.5, 0.0, 5_000.0);

    assert!(
        (4.0..15.0).contains(&bass_boost),
        "bass boost {bass_boost} dB"
    );
    assert!((4.0..25.0).contains(&bass_cut), "bass cut {bass_cut} dB");
    assert!(
        (5.0..20.0).contains(&treble_boost),
        "treble boost {treble_boost} dB"
    );
    assert!(
        (15.0..40.0).contains(&treble_cut),
        "treble cut {treble_cut} dB"
    );
}
