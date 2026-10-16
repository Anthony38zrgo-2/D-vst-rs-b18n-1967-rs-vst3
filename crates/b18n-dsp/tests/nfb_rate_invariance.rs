//! Invariancia de la respuesta en lazo cerrado respecto al sobremuestreo tras
//! B18-009.
//!
//! B18-009 sustituyó el polo de compensación de la NFB y el `z^-1` (que
//! escalaban con el factor de sobremuestreo, B18-002) por la etapa de salida
//! física acoplada por muestra. Ya no hay compensación dependiente de la tasa,
//! de modo que la única diferencia 1×/2×/4× es la **discretización** del propio
//! modelo: en la banda baja (≪ Nyquist de 1×) la latencia compensada debe ser
//! comparable entre factores; cerca de Nyquist, la autorresonancia del OT
//! (≈37 kHz) introduce una pérdida de ancho de banda real a 1× que no es un
//! defecto de la compensación (queda documentada, no se oculta).
//!
//! Criterio **provisional de ingeniería** (no medición de hardware): en
//! 0,5–1 kHz la dispersión medida es ≤0,04 dB / ≤4,2°; los límites se fijan en
//! 0,5 dB / 8°.

use std::f64::consts::PI;

use b18n_dsp::{B18nEngine, ProcessSpec};

const FRAMES: usize = 16_384;
const TAIL_FRAMES: usize = 8_192;
const AMPLITUDE_V: f32 = 0.05;

/// Dispersión máxima admitida entre factores en la banda de validación.
const TOL_MAG_DB: f64 = 0.5;
const TOL_PHASE_DEG: f64 = 8.0;

/// Banda donde la discretización es despreciable a todos los factores.
const BAND_HZ: &[f64] = &[500.0, 1_000.0];

fn engine(rate_hz: f64, oversampling: u8) -> B18nEngine {
    B18nEngine::new(
        b18n_config::load_reference().expect("modelo histórico"),
        ProcessSpec {
            sample_rate_hz: rate_hz,
            max_block_frames: FRAMES,
            oversampling,
        },
    )
    .expect("motor")
}

/// Frecuencia exacta en la rejilla de la ventana (evita fuga espectral).
fn snapped(frequency_hz: f64, rate_hz: f64) -> f64 {
    let bin = (frequency_hz * TAIL_FRAMES as f64 / rate_hz)
        .round()
        .max(1.0);
    bin * rate_hz / TAIL_FRAMES as f64
}

fn dft(samples: &[f64], rate_hz: f64, frequency_hz: f64) -> (f64, f64) {
    let omega = 2.0 * PI * frequency_hz / rate_hz;
    let (mut re, mut im) = (0.0, 0.0);
    for (index, sample) in samples.iter().enumerate() {
        let angle = omega * index as f64;
        re += sample * angle.cos();
        im -= sample * angle.sin();
    }
    (re, im)
}

/// Ganancia compleja de la cadena para un seno pequeño: `(dB, grados)` con la
/// latencia del sobremuestreo compensada.
fn complex_gain(rate_hz: f64, oversampling: u8, frequency_hz: f64) -> (f64, f64) {
    let frequency_hz = snapped(frequency_hz, rate_hz);
    let mut engine = engine(rate_hz, oversampling);

    let input: Vec<f32> = (0..FRAMES)
        .map(|index| (2.0 * PI * frequency_hz * index as f64 / rate_hz).sin() as f32 * AMPLITUDE_V)
        .collect();
    let mut output = vec![0.0_f32; FRAMES];
    engine.process_mono(&input, &mut output);

    assert_eq!(
        engine.convergence_failures(),
        0,
        "{rate_hz} Hz, {oversampling}×: sin fallos de convergencia"
    );

    let input_tail = &input[FRAMES - TAIL_FRAMES..];
    let output_tail = &output[FRAMES - TAIL_FRAMES..];
    let input_tail: Vec<f64> = input_tail.iter().map(|sample| f64::from(*sample)).collect();
    let output_tail: Vec<f64> = output_tail
        .iter()
        .map(|sample| f64::from(*sample))
        .collect();

    let (in_re, in_im) = dft(&input_tail, rate_hz, frequency_hz);
    let (out_re, out_im) = dft(&output_tail, rate_hz, frequency_hz);

    let magnitude =
        (out_re.hypot(out_im) / in_re.hypot(in_im)).ln() * 20.0 / std::f64::consts::LN_10;
    let mut phase_deg = (out_im.atan2(out_re) - in_im.atan2(in_re)).to_degrees();
    // Compensa la latencia de grupo del sobremuestreo (muestras de tasa base)
    // y normaliza a (−180, 180]: la latencia no es un número entero de períodos
    // y cada factor acumula vueltas distintas antes de comparar.
    phase_deg += 360.0 * frequency_hz * f64::from(engine.latency_samples()) / rate_hz;
    phase_deg = (phase_deg + 180.0).rem_euclid(360.0) - 180.0;
    (magnitude, phase_deg)
}

fn spread(values: &[f64]) -> f64 {
    let min = values.iter().copied().fold(f64::INFINITY, f64::min);
    let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    max - min
}

fn check_invariance(rate_hz: f64, frequencies_hz: &[f64]) {
    for &frequency_hz in frequencies_hz {
        let gains: Vec<(f64, f64)> = [1_u8, 2, 4]
            .iter()
            .map(|&oversampling| complex_gain(rate_hz, oversampling, frequency_hz))
            .collect();
        let magnitudes: Vec<f64> = gains.iter().map(|(mag, _)| *mag).collect();
        let phases: Vec<f64> = gains.iter().map(|(_, phase)| *phase).collect();
        let mag_spread = spread(&magnitudes);
        let phase_spread = spread(&phases);

        if std::env::var_os("B18N_NFB_REPORT").is_some() {
            println!(
                "{rate_hz:>7.0} Hz  {frequency_hz:>7.1} Hz  mag {:+.3}/{:+.3}/{:+.3} dB \
                 (Δ{:.3})  fase {:+.2}/{:+.2}/{:+.2}° (Δ{:.2})",
                magnitudes[0],
                magnitudes[1],
                magnitudes[2],
                mag_spread,
                phases[0],
                phases[1],
                phases[2],
                phase_spread
            );
        }

        let (mag_tol, phase_tol) = (TOL_MAG_DB, TOL_PHASE_DEG);
        assert!(
            mag_spread <= mag_tol,
            "{rate_hz} Hz, {frequency_hz} Hz: dispersión de magnitud {mag_spread:.3} dB \
             (límite {mag_tol}): {magnitudes:?}"
        );
        assert!(
            phase_spread <= phase_tol,
            "{rate_hz} Hz, {frequency_hz} Hz: dispersión de fase {phase_spread:.2}° \
             (límite {phase_tol}): {phases:?}"
        );
    }
}

#[test]
fn closed_loop_is_invariant_at_48k() {
    check_invariance(48_000.0, BAND_HZ);
}

#[test]
fn closed_loop_is_invariant_across_sample_rates() {
    for rate_hz in [44_100.0, 48_000.0, 96_000.0, 192_000.0] {
        check_invariance(rate_hz, BAND_HZ);
    }
}
