//! Redes de jack de entrada (B18-007): contraste con ngspice y con el manual,
//! y verificación de que la conmutación de canal aplica la carga trazada.
//!
//! Trazado del raster SCH-567 (véase `crates/b18n-dsp/src/stages/input.rs`):
//! CH1 lleva 120 kΩ por jack y 5.6 MΩ de fuga, con la llave del jack `guitar`
//! libre cortando a masa por 100 kΩ; CH2 (`instrument`) va directa a la rejilla
//! con 2.2 MΩ de fuga.

use std::f64::consts::PI;

use b18n_dsp::stages::input::InputJackNetwork;
use b18n_dsp::{B18nEngine, InputChannel, ProcessSpec, RuntimeControls};

const JACKS_CSV: &str =
    include_str!("../../../assets/fixtures/reference/input_jacks/input_jacks_spice.csv");

const RATE_HZ: f64 = 48_000.0;
const FRAMES: usize = 16_384;
const TAIL: usize = 4_096;

fn network_for(name: &str) -> InputJackNetwork {
    match name {
        "bass" => InputJackNetwork::for_channel(InputChannel::Bass),
        "guitar" => InputJackNetwork::for_channel(InputChannel::Guitar),
        "instrument" => InputJackNetwork::for_channel(InputChannel::Instrument),
        other => panic!("jack desconocido en el golden: {other}"),
    }
}

/// Divisor trazado contra ngspice 47 en 20 Hz–20 kHz y dos impedancias de
/// fuente (0 Ω ideal y 10 kΩ de instrumento pasivo, condición de ensayo).
#[test]
fn jack_networks_match_ngspice_reference() {
    let mut compared = 0_usize;
    let mut worst_magnitude_db = 0.0_f64;
    let mut worst_phase_deg = 0.0_f64;

    for (index, line) in JACKS_CSV.lines().enumerate() {
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
        let jack = fields.next().expect("jack").trim();
        let source_ohms = parse(fields.next());
        let frequency = parse(fields.next());
        let expected_magnitude_db = parse(fields.next());
        let expected_phase_deg = parse(fields.next());

        let network = network_for(jack);
        let division = network.grid_division(source_ohms);
        let magnitude_db = 20.0 * division.log10();
        let magnitude_error = (magnitude_db - expected_magnitude_db).abs();
        let phase_error = expected_phase_deg.abs();
        worst_magnitude_db = worst_magnitude_db.max(magnitude_error);
        worst_phase_deg = worst_phase_deg.max(phase_error);
        compared += 1;

        assert!(
            magnitude_error < 1e-6,
            "{jack} @ {frequency} Hz, Rsrc {source_ohms}: {magnitude_error} dB"
        );
        assert!(
            phase_error < 1e-4,
            "{jack} @ {frequency} Hz, Rsrc {source_ohms}: fase {expected_phase_deg}°"
        );
    }

    assert_eq!(compared, 3 * 2 * 31, "referencia de jacks incompleta");
    assert!(worst_magnitude_db < 1e-6);
    assert!(worst_phase_deg < 1e-4);
}

fn engine(channel: InputChannel) -> B18nEngine {
    let mut engine = B18nEngine::new(
        b18n_config::load_reference().expect("modelo histórico"),
        ProcessSpec {
            sample_rate_hz: RATE_HZ,
            max_block_frames: FRAMES,
            oversampling: 1,
        },
    )
    .expect("motor");
    engine.set_controls(RuntimeControls {
        channel,
        ..RuntimeControls::default()
    });
    engine
}

fn tail_peak(channel: InputChannel) -> f64 {
    let mut engine = engine(channel);
    let input: Vec<f32> = (0..FRAMES)
        .map(|index| (2.0 * PI * 1_000.0 * index as f64 / RATE_HZ).sin() as f32 * 0.01)
        .collect();
    let mut output = vec![0.0_f32; FRAMES];
    engine.process_mono(&input, &mut output);
    assert_eq!(engine.convergence_failures(), 0, "{channel:?}");
    output[FRAMES - TAIL..]
        .iter()
        .fold(0.0_f64, |acc, sample| acc.max(f64::from(sample.abs())))
}

/// La conmutación de jack cambia la carga de rejilla: la ganancia relativa
/// entre canales reproduce los divisores trazados.
#[test]
fn channel_selection_applies_the_traced_jack_loads() {
    let bass = tail_peak(InputChannel::Bass);
    let guitar = tail_peak(InputChannel::Guitar);
    let instrument = tail_peak(InputChannel::Instrument);
    assert!(bass > 0.0 && guitar > 0.0 && instrument > 0.0);

    let division = |channel| InputJackNetwork::for_channel(channel).grid_division(0.0);
    let expected_guitar_over_bass = division(InputChannel::Guitar) / division(InputChannel::Bass);
    let expected_instrument_over_guitar =
        division(InputChannel::Instrument) / division(InputChannel::Guitar);

    let measured_guitar_over_bass = guitar / bass;
    let measured_instrument_over_guitar = instrument / guitar;

    assert!(
        (measured_guitar_over_bass - expected_guitar_over_bass).abs() / expected_guitar_over_bass
            < 0.02,
        "guitar/bass medido {measured_guitar_over_bass:.4}, esperado {expected_guitar_over_bass:.4}"
    );
    assert!(
        (measured_instrument_over_guitar - expected_instrument_over_guitar).abs()
            / expected_instrument_over_guitar
            < 0.02,
        "instrument/guitar medido {measured_instrument_over_guitar:.4}, \
         esperado {expected_instrument_over_guitar:.4}"
    );
}
