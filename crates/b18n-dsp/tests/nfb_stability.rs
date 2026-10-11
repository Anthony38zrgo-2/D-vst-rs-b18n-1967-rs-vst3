//! Estabilidad de la realimentación negativa (P1.6, separación explícita
//! `z^-1` con compensación documentada en `engine.rs`).
//!
//! La NFB se inyecta en el cátodo del pentodo (AN-183 y raster) a través de la
//! 22 kΩ desde la toma de 16 Ω. El modelo no incluye los parásitos de alta
//! frecuencia del tubo/transformador, así que el lazo explícito se compensa
//! con un polo en el camino de realimentación (20 kHz, declarado). Estos tests
//! verifican: ganancia menor con lazo cerrado, estabilidad a 44.1–192 kHz con
//! cargas de 8/16 Ω y drives fuertes, ausencia de oscilación sostenida y
//! recuperación del *blocking* tras sobrexcitación.

use b18n_dsp::{B18nEngine, ProcessSpec};

const SAMPLE_RATE_HZ: f64 = 48_000.0;

fn engine_with(rate_hz: f64, load_ohms: f64, feedback: bool) -> B18nEngine {
    let mut circuit = b18n_config::load_reference().expect("modelo");
    circuit.speaker.nominal_load_ohms = load_ohms;
    if !feedback {
        circuit.output_transformer.negative_feedback.present = false;
    }
    B18nEngine::new(
        circuit,
        ProcessSpec {
            sample_rate_hz: rate_hz,
            max_block_frames: 16_384,
            oversampling: 4,
        },
    )
    .expect("motor")
}

fn render(engine: &mut B18nEngine, input: &[f32]) -> Vec<f32> {
    let mut output = vec![0.0_f32; input.len()];
    engine.process_mono(input, &mut output);
    output
}

fn sine(frames: usize, rate_hz: f64, frequency_hz: f64, amplitude: f32) -> Vec<f32> {
    (0..frames)
        .map(|index| {
            (2.0 * std::f64::consts::PI * frequency_hz * index as f64 / rate_hz).sin() as f32
                * amplitude
        })
        .collect()
}

fn tail_peak(samples: &[f32]) -> f64 {
    samples[samples.len() * 3 / 4..]
        .iter()
        .fold(0.0_f32, |acc, s| acc.max(s.abs())) as f64
}

#[test]
fn closed_loop_reduces_gain_and_stays_stable() {
    let input = sine(16_384, SAMPLE_RATE_HZ, 997.0, 0.1);

    let mut open = engine_with(SAMPLE_RATE_HZ, 8.0, false);
    let open_out = render(&mut open, &input);
    let mut closed = engine_with(SAMPLE_RATE_HZ, 8.0, true);
    let closed_out = render(&mut closed, &input);

    assert!(open_out.iter().all(|s| s.is_finite()));
    assert!(closed_out.iter().all(|s| s.is_finite()));

    let open_peak = tail_peak(&open_out);
    let closed_peak = tail_peak(&closed_out);
    assert!(open_peak > 0.0 && closed_peak > 0.0);
    // La realimentación reduce la ganancia; cota amplia y documentada
    // (medido ≈0.14× con 22 kΩ).
    assert!(
        closed_peak < 0.6 * open_peak,
        "la NFB no redujo la ganancia: abierto {open_peak}, cerrado {closed_peak}"
    );
    assert_eq!(open.convergence_failures(), 0);
    assert_eq!(closed.convergence_failures(), 0);
}

#[test]
fn stable_across_rates_loads_and_drives() {
    for rate_hz in [44_100.0, 96_000.0, 192_000.0] {
        for load_ohms in [8.0, 16.0] {
            for amplitude in [0.2_f32, 1.0, 4.0] {
                let frames = 4_096;
                let input = sine(frames, rate_hz, 1_000.0, amplitude);
                let mut engine = engine_with(rate_hz, load_ohms, true);
                let output = render(&mut engine, &input);
                let peak = output.iter().fold(0.0_f32, |acc, s| acc.max(s.abs()));
                assert!(
                    output.iter().all(|s| s.is_finite()),
                    "{rate_hz} Hz, {load_ohms} Ω, {amplitude} V: no finito"
                );
                assert!(
                    peak < 500.0,
                    "{rate_hz} Hz, {load_ohms} Ω, {amplitude} V: pico {peak}"
                );
                assert!(
                    engine.convergence_failures() < frames as u64,
                    "{rate_hz} Hz, {load_ohms} Ω, {amplitude} V: fallos {}",
                    engine.convergence_failures()
                );
            }
        }
    }
}

#[test]
fn step_ringing_decays_without_sustained_oscillation() {
    let mut engine = engine_with(SAMPLE_RATE_HZ, 8.0, true);
    // Impulso fuerte y silencio: la cola no debe sostener una oscilación.
    let mut input = vec![0.0_f32; 16_384];
    input[0] = 2.0;
    let output = render(&mut engine, &input);

    let window = |index: usize| {
        let start = index * 4_096;
        output[start..start + 4_096]
            .iter()
            .fold(0.0_f32, |acc, s| acc.max(s.abs())) as f64
    };
    let first = window(0);
    let second = window(1);
    let last = window(3);
    assert!(first > 0.0);
    assert!(second < first, "la respuesta no decae: {first} -> {second}");
    assert!(last < 0.2 * first, "ringing sostenido: {first} -> {last}");
    assert!(engine.convergence_failures() < 100);
}

#[test]
fn blocking_recovers_after_overdrive() {
    let mut engine = engine_with(SAMPLE_RATE_HZ, 8.0, true);
    // Ráfaga fuerte a 100 Hz (carga los capacitores de acoplo por conducción
    // de rejilla) seguida de silencio. La recuperación incluye el polo de 1 Hz
    // del bloqueo de continua del OT (asumido conservador), por eso se
    // observan 1.5 s de silencio.
    let burst_frames = 4_800; // 100 ms
    let silence_frames = 72_000; // 1.5 s
    let mut input = sine(burst_frames, SAMPLE_RATE_HZ, 100.0, 6.0);
    input.resize(burst_frames + silence_frames, 0.0_f32);
    let output = render(&mut engine, &input);

    let peak = output.iter().fold(0.0_f32, |acc, s| acc.max(s.abs()));
    assert!(peak.is_finite() && peak < 500.0, "pico {peak}");

    // Al final del silencio la salida debe haber vuelto al reposo.
    let tail = &output[output.len() - 4_800..];
    let residual = tail.iter().fold(0.0_f32, |acc, s| acc.max(s.abs()));
    assert!(
        residual < 0.05,
        "el blocking no se recuperó: residuo {residual}"
    );
    assert!(engine.convergence_failures() < 500);
}
