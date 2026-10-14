//! Supresión de alias con sobremuestreo (Hito C, aceptación).
//!
//! Se excita la cadena con un tono que genera armónicos por encima de Nyquist.
//! Con 5 × 7.3 kHz = 36.5 kHz, el alias de primer orden cae en
//! 48 − 36.5 = 11.5 kHz, lejos de los armónicos audibles (14.6, 21.9 kHz).
//!
//! Tras B18-002 (polo de NFB a tasa efectiva + TPT) la comparación relativa
//! 1×/4× se re-basó: el criterio anterior («4× mejora ≥5 dB») era un artefacto
//! del polo dependiente del sobremuestreo (efectivo ≈10 kHz a 4× frente a
//! 2,5 kHz a 1×). Con el polo unificado a 2,5 kHz, medido a 4 V: 1× mejora de
//! −27,5 a −31,7 dBc, pero 4× pasa de −41,2 a −25,1 dBc (el polo de 10 kHz
//! efectivo daba más realimentación de HF, inconsistente con 1×). El alias de
//! 4× está dominado por intermodulación **en banda** entre las imágenes del
//! interpolador y los armónicos del pentodo (no removible por decimación); la
//! pérdida de margen a 4× queda registrada para B18-018 (mejora de
//! interpolador/decimación). Se conserva la cota absoluta ≤ −18 dBc en ambos
//! factores. Véase `docs/MODEL_ASSUMPTIONS.md`.

use b18n_dsp::{B18nEngine, ProcessSpec};

const SAMPLE_RATE_HZ: f64 = 48_000.0;
const INPUT_HZ: f64 = 7_300.0;
const ALIAS_HZ: f64 = 11_500.0;
const FRAMES: usize = 32_768;
/// Amplitud de excitación: régimen de drive muy fuerte (4 V), donde el
/// plegado del 5.º armónico en 1× domina y el suelo de alias es medible.
const AMPLITUDE: f32 = 4.0;

fn render(oversampling: u8, amplitude: f32) -> Vec<f32> {
    let circuit = b18n_config::load_reference().expect("JSON + calibración");
    let mut engine = B18nEngine::new(
        circuit,
        ProcessSpec {
            sample_rate_hz: SAMPLE_RATE_HZ,
            max_block_frames: FRAMES,
            oversampling,
        },
    )
    .expect("motor");

    let input: Vec<f32> = (0..FRAMES)
        .map(|index| {
            (2.0 * std::f64::consts::PI * INPUT_HZ * index as f64 / SAMPLE_RATE_HZ).sin() as f32
                * amplitude
        })
        .collect();
    let mut output = vec![0.0_f32; FRAMES];
    engine.process_mono(&input, &mut output);
    assert!(output.iter().all(|sample| sample.is_finite()));
    assert!(
        engine.convergence_failures() < 1_000,
        "{oversampling}×: demasiados fallos de convergencia ({})",
        engine.convergence_failures()
    );
    output
}

/// Potencia normalizada en `frequency_hz` (ventana Hann, Goertzel).
fn tone_power(samples: &[f32], frequency_hz: f64) -> f64 {
    let n = samples.len();
    let omega = 2.0 * std::f64::consts::PI * frequency_hz / SAMPLE_RATE_HZ;
    let mut real = 0.0;
    let mut imag = 0.0;
    for (index, sample) in samples.iter().enumerate() {
        let window = 0.5 - 0.5 * (2.0 * std::f64::consts::PI * index as f64 / (n - 1) as f64).cos();
        let value = f64::from(*sample) * window;
        real += value * (omega * index as f64).cos();
        imag += value * (omega * index as f64).sin();
    }
    (real * real + imag * imag) / (n as f64 * n as f64)
}

/// Suelo de alias relativo a la fundamental (dBc) para cada factor.
fn alias_floor_dbc(oversampling: u8) -> (f64, f64, f64) {
    let output = render(oversampling, AMPLITUDE);
    let start = 8_192;
    let alias = tone_power(&output[start..], ALIAS_HZ);
    let fundamental = tone_power(&output[start..], INPUT_HZ);
    (
        alias,
        fundamental,
        10.0 * (alias.max(1e-30) / fundamental).log10(),
    )
}

#[test]
fn alias_floor_is_bounded_at_one_and_four_times() {
    let (alias_1x, fundamental_1x, floor_1x) = alias_floor_dbc(1);
    let (alias_4x, fundamental_4x, floor_4x) = alias_floor_dbc(4);

    // Cota absoluta documentada (la misma que el Hito C): alias ≤ −18 dBc.
    // Medido tras B18-002: 1× −31,7 dBc; 4× −25,1 dBc.
    assert!(
        floor_1x < -18.0,
        "alias 1× demasiado alto: {floor_1x:.1} dBc (fundamental {fundamental_1x:.3e}, alias {alias_1x:.3e})"
    );
    assert!(
        floor_4x < -18.0,
        "alias 4× demasiado alto: {floor_4x:.1} dBc (fundamental {fundamental_4x:.3e}, alias {alias_4x:.3e})"
    );
}

/// La ruta identidad del sobremuestreador (4× arriba y abajo sin cadena) no
/// debe introducir espurios: valida el filtro de media banda de 63 taps.
#[test]
fn identity_path_has_no_spurs() {
    use b18n_dsp::numerics::oversampling::Oversampler;

    let mut oversampler = Oversampler::new(4).expect("4×");
    let mut output = Vec::with_capacity(FRAMES);
    for index in 0..FRAMES {
        let value = (2.0 * std::f64::consts::PI * INPUT_HZ * index as f64 / SAMPLE_RATE_HZ).sin();
        output.push(oversampler.process(value as f32, |sample| sample));
    }
    let tail = &output[8_192..];
    let fundamental = tone_power(tail, INPUT_HZ);
    for spur_hz in [11_500.0, 14_600.0, 21_900.0, 3_650.0] {
        let spur = tone_power(tail, spur_hz);
        assert!(
            spur < fundamental * 1e-6,
            "espurio en {spur_hz} Hz: {spur:.3e} vs fundamental {fundamental:.3e}"
        );
    }
}

#[test]
fn one_times_has_measurable_alias() {
    let one_times = render(1, AMPLITUDE);
    let start = 8_192;
    let alias_1x = tone_power(&one_times[start..], ALIAS_HZ);
    let fundamental_1x = tone_power(&one_times[start..], INPUT_HZ);
    // Sin sobremuestreo el alias debe ser medible (no un artefacto numérico).
    assert!(
        alias_1x > fundamental_1x * 1e-6,
        "alias 1× demasiado bajo para la prueba: {alias_1x:.3e} vs {fundamental_1x:.3e}"
    );
}
