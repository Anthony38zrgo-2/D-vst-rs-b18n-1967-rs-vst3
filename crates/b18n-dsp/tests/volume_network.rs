//! Red de volumen física del 6SL7#2 (B18-005): contraste con ngspice,
//! monotonicidad y ausencia de pops tras `reset`.
//!
//! La red trazada es: placa del 6SL7#1 (`Vsrc` detrás de `Ro`) → capacitor de
//! acoplo de 0.022 µF → potenciómetro de 1 MΩ log (Rtop–cursor–Rbottom a masa)
//! → rejilla del 6SL7#2. El golden `volume_network_spice.csv` se genera con
//! `scripts/spice/volume_network_sweep.py` (ngspice 47, circuito continuo);
//! el test mide la transferencia `v(cursor)/v(src)` del solve temporal a
//! 192 kHz (misma tasa efectiva que el motor a 4×48 kHz).

use std::f64::consts::PI;

use b18n_dsp::stages::triode_6sl7::{NonlinearTriodeStage, TriodeStageConfig};
use b18n_dsp::stages::tube_models::KorenTriodeModel;
use b18n_dsp::{volume_fraction, B18nEngine, ProcessSpec, RuntimeControls};

const GOLDEN_CSV: &str =
    include_str!("../../../assets/fixtures/reference/volume_network/volume_network_spice.csv");

const RATE_HZ: f64 = 192_000.0;
const FRAMES: usize = 96_000; // 0,5 s: número entero de períodos en el golden
const AMPLITUDE_V: f64 = 0.05;

fn stage(ro_ohms: f64, coupling_farads: f64, pot_ohms: f64) -> NonlinearTriodeStage {
    let circuit = b18n_config::load_reference().expect("modelo histórico");
    let params = circuit
        .tube_models
        .koren_6sl7
        .expect("modelo 6SL7 en la calibración");
    let supply_v = circuit
        .tube_models
        .preamp_supply_v
        .expect("carril de previo");
    let mut stage = NonlinearTriodeStage::new(
        KorenTriodeModel::new(params),
        TriodeStageConfig {
            plate_resistor_ohms: circuit.channel.triode_2.plate_resistor_ohms,
            cathode_resistor_ohms: circuit.channel.triode_2.cathode_resistor_ohms,
            grid_leak_ohms: pot_ohms,
            supply_v,
            coupling_farads: None,
            series_resistance_ohms: 1.0,
            sample_rate_hz: RATE_HZ,
        },
    );
    stage.enable_volume_network(ro_ohms, coupling_farads, pot_ohms);
    assert!(stage.solve_operating_point().converged, "reposo del triodo");
    stage
}

/// Transferencia compleja medida `(magnitud dB, fase grados)` del cursor
/// respecto de `Vsrc`, con referencia coseno y ventana de períodos enteros.
fn measured_transfer(stage: &mut NonlinearTriodeStage, frequency_hz: f64) -> (f64, f64) {
    let omega = 2.0 * PI * frequency_hz / RATE_HZ;
    let settle = FRAMES / 2;
    let (mut re_in, mut im_in) = (0.0, 0.0);
    let (mut re_grid, mut im_grid) = (0.0, 0.0);
    for index in 0..FRAMES {
        let angle = omega * index as f64;
        let input = angle.cos() * AMPLITUDE_V;
        stage.process(input);
        if index >= settle {
            re_in += input * angle.cos();
            im_in -= input * angle.sin();
            let grid = stage.grid_voltage();
            re_grid += grid * angle.cos();
            im_grid -= grid * angle.sin();
        }
    }
    let input_magnitude = re_in.hypot(im_in);
    let grid_magnitude = re_grid.hypot(im_grid);
    let magnitude = grid_magnitude / input_magnitude;
    let phase_deg = (im_grid.atan2(re_grid) - im_in.atan2(re_in)).to_degrees();
    (20.0 * magnitude.log10(), phase_deg)
}

#[test]
fn volume_network_matches_ngspice_reference() {
    let mut compared = 0_usize;
    let mut worst_magnitude_db = 0.0_f64;
    let mut worst_phase_deg = 0.0_f64;

    for (index, line) in GOLDEN_CSV.lines().enumerate() {
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
        let position = parse(fields.next());
        let fraction = parse(fields.next());
        let ro_ohms = parse(fields.next());
        let coupling_farads = parse(fields.next());
        let pot_ohms = parse(fields.next());
        let frequency = parse(fields.next());
        let expected_magnitude_db = parse(fields.next());
        let expected_phase_deg = parse(fields.next());

        let mut stage = stage(ro_ohms, coupling_farads, pot_ohms);
        stage.set_volume_fraction(fraction);
        let (magnitude_db, phase_deg) = measured_transfer(&mut stage, frequency);

        let magnitude_error = (magnitude_db - expected_magnitude_db).abs();
        let phase_error = (phase_deg - expected_phase_deg).abs();
        worst_magnitude_db = worst_magnitude_db.max(magnitude_error);
        worst_phase_deg = worst_phase_deg.max(phase_error);
        compared += 1;

        assert!(
            magnitude_error < 0.02,
            "volumen {position} @ {frequency} Hz: {magnitude_error} dB \
             (medido {magnitude_db}, golden {expected_magnitude_db})"
        );
        assert!(
            phase_error < 0.2,
            "volumen {position} @ {frequency} Hz: {phase_error}° \
             (medido {phase_deg}, golden {expected_phase_deg})"
        );
    }

    assert_eq!(compared, 4 * 3, "golden de volumen incompleto");
    assert!(worst_magnitude_db < 0.02);
    assert!(worst_phase_deg < 0.2);
}

/// En pequeña señal la rejilla no conduce: el cursor sigue la división física
/// del potenciómetro, con el mínimo en 0 (cursor a masa) y crecimiento
/// monótono con la posición del mando.
#[test]
fn volume_response_is_monotonic_and_grounds_at_zero() {
    let mut gains = Vec::new();
    for position in [0.0_f32, 0.1, 0.5, 0.9, 1.0] {
        let mut stage = stage(227_700.0, 2.2e-8, 1_000_000.0);
        stage.set_volume_fraction(volume_fraction(position));
        let (magnitude_db, _) = measured_transfer(&mut stage, 1_000.0);
        gains.push(magnitude_db);
    }
    assert!(gains[0] < -100.0, "cursor a masa: {} dB", gains[0]);
    for pair in gains.windows(2) {
        assert!(
            pair[1] > pair[0],
            "respuesta de volumen no monótona: {pair:?} en {gains:?}"
        );
    }
    // La posición 1 no llega a 0 dB por la división con `Ro` (227,7 kΩ).
    assert!(
        gains[4] < 0.0 && gains[4] > -6.0,
        "fondo de escala: {}",
        gains[4]
    );
}

fn engine(volume: f32) -> B18nEngine {
    let mut engine = B18nEngine::new(
        b18n_config::load_reference().expect("modelo histórico"),
        ProcessSpec {
            sample_rate_hz: 48_000.0,
            max_block_frames: 8_192,
            oversampling: 1,
        },
    )
    .expect("motor");
    engine.set_controls(RuntimeControls {
        volume_1: volume,
        volume_2: volume,
        ..RuntimeControls::default()
    });
    engine
}

fn render_peak(engine: &mut B18nEngine, frames: usize) -> f64 {
    let input: Vec<f32> = (0..frames)
        .map(|index| (2.0 * PI * 1_000.0 * index as f64 / 48_000.0).sin() as f32 * 0.05)
        .collect();
    let mut output = vec![0.0_f32; frames];
    engine.process_mono(&input, &mut output);
    output[frames * 3 / 4..]
        .iter()
        .fold(0.0_f64, |acc, sample| acc.max(f64::from(sample.abs())))
}

#[test]
fn engine_volume_is_monotonic_in_small_signal() {
    let mut peaks = Vec::new();
    for volume in [0.0_f32, 0.25, 0.5, 0.75, 1.0] {
        let mut engine = engine(volume);
        peaks.push(render_peak(&mut engine, 16_384));
    }
    for pair in peaks.windows(2) {
        assert!(
            pair[1] > pair[0],
            "volumen del motor no monótono: {peaks:?}"
        );
    }
}

/// `reset()` vuelve al reposo sin rampa audible: la cola tras el reset no
/// arranca con un escalón grande respecto del régimen.
#[test]
fn engine_reset_does_not_pop() {
    let mut engine = engine(0.9);
    let steady = render_peak(&mut engine, 16_384);
    assert!(steady > 0.0);
    engine.reset();
    let input: Vec<f32> = (0..4_096)
        .map(|index| (2.0 * PI * 1_000.0 * index as f64 / 48_000.0).sin() as f32 * 0.05)
        .collect();
    let mut output = vec![0.0_f32; input.len()];
    engine.process_mono(&input, &mut output);
    let head = output[..64]
        .iter()
        .fold(0.0_f64, |acc, sample| acc.max(f64::from(sample.abs())));
    assert!(
        head < 3.0 * steady,
        "pop tras reset: cabeza {head} vs régimen {steady}"
    );
}
