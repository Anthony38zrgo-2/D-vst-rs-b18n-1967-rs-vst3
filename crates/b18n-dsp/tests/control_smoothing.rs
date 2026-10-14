//! Tiempo físico del suavizado de los trims de entrada/salida (B18-004).
//!
//! El motor avanza los suavizados una vez por muestra de la **tasa de proceso
//! efectiva** (`base × oversampling`). El código anterior multiplicaba el
//! tiempo nominal (15 ms) por el factor de sobremuestreo al construir los
//! suavizados, de modo que el escalón se alargaba a 30/60 ms a 2×/4× mientras
//! los potenciómetros de `ChannelChain` sí usaban el tiempo constante.
//!
//! La medición se hace sobre la **envolvente de salida en el dominio de dB**
//! (el suavizado de los trims vive en dB): el trim multiplica la señal ya
//! resuelta al final de la cadena, y la ventana de medida empieza con las
//! válvulas y filtros en régimen, de modo que el efecto del asentamiento no
//! entra en la trayectoria medida.

use std::f64::consts::PI;

use b18n_dsp::utilities::smoothing::SmoothedValue;
use b18n_dsp::{B18nEngine, ProcessSpec, RuntimeControls};

const RATE_HZ: f64 = 48_000.0;
const TONE_HZ: f64 = 1_000.0;
/// Muestras por ciclo del tono (48 kHz / 1 kHz) para alinear las ventanas.
const CYCLE: usize = 48;
const SETTLE_FRAMES: usize = 9_600; // 200 ms
const MEASURE_FRAMES: usize = 19_200; // 400 ms
const AMPLITUDE: f32 = 0.5;
const TRIM_START_DB: f32 = -20.0;

fn engine(oversampling: u8) -> B18nEngine {
    B18nEngine::new(
        b18n_config::load_reference().expect("modelo histórico"),
        ProcessSpec {
            sample_rate_hz: RATE_HZ,
            max_block_frames: SETTLE_FRAMES + MEASURE_FRAMES,
            oversampling,
        },
    )
    .expect("motor")
}

fn sine(frames: usize, offset: usize) -> Vec<f32> {
    (0..frames)
        .map(|index| {
            (2.0 * PI * TONE_HZ * (offset + index) as f64 / RATE_HZ).sin() as f32 * AMPLITUDE
        })
        .collect()
}

fn peak(samples: &[f32]) -> f64 {
    samples
        .iter()
        .fold(0.0_f64, |acc, sample| acc.max(f64::from(sample.abs())))
}

/// Instante (ms) en que la envolvente cruza el 63,2 % del escalón de trim.
fn trim_crossing_63_ms(oversampling: u8) -> f64 {
    let mut engine = engine(oversampling);

    let mut controls = RuntimeControls {
        output_trim_db: TRIM_START_DB,
        ..RuntimeControls::default()
    };
    // La primera `set_controls` de la instancia salta directo al objetivo.
    engine.set_controls(controls);

    let settle_input = sine(SETTLE_FRAMES, 0);
    let mut settle_output = vec![0.0_f32; SETTLE_FRAMES];
    engine.process_mono(&settle_input, &mut settle_output);

    controls.output_trim_db = 0.0;
    engine.set_controls(controls);

    let measure_input = sine(MEASURE_FRAMES, SETTLE_FRAMES);
    let mut measure_output = vec![0.0_f32; MEASURE_FRAMES];
    engine.process_mono(&measure_input, &mut measure_output);

    let start_level = peak(&settle_output[SETTLE_FRAMES - CYCLE..]);
    let end_level = peak(&measure_output[MEASURE_FRAMES - CYCLE..]);
    assert!(
        start_level > 0.0 && end_level > start_level * 3.0,
        "escalón de trim insuficiente: {start_level} -> {end_level}"
    );

    // Umbral en dB (el suavizado vive en dB): cruce del 63,2 % del recorrido.
    let start_db = 20.0 * start_level.log10();
    let end_db = 20.0 * end_level.log10();
    let threshold_db = start_db + (end_db - start_db) * 0.632;

    // Ventanas de medio ciclo: resolución de 0,5 ms a 48 kHz.
    let window = CYCLE / 2;
    for start in (0..MEASURE_FRAMES - window).step_by(window) {
        let window_peak = peak(&measure_output[start..start + window]);
        if window_peak > 0.0 && 20.0 * window_peak.log10() >= threshold_db {
            let center = start as f64 + window as f64 / 2.0;
            return center / RATE_HZ * 1_000.0;
        }
    }
    panic!("la envolvente no alcanzó el 63,2 % del escalón");
}

#[test]
fn trim_step_is_15_ms_physical_for_every_oversampling_factor() {
    for oversampling in [1_u8, 2, 4] {
        let crossing_ms = trim_crossing_63_ms(oversampling);
        if std::env::var_os("B18N_SMOOTHING_REPORT").is_some() {
            println!("{oversampling}×: cruce del 63,2 % en {crossing_ms:.2} ms");
        }
        // Tolerancia de medida por ventana (0,5 ms) más margen del montaje:
        // el valor defectuoso era 15/30/60 ms, muy fuera de esta cota.
        assert!(
            (crossing_ms - 15.0).abs() <= 2.5,
            "{oversampling}×: cruce del 63,2 % en {crossing_ms:.2} ms \
             (se esperan 15 ms físicos)"
        );
    }
}

/// Contrato del suavizado: 15 ms son 15 ms en la tasa efectiva, con tolerancia
/// de una muestra de esa tasa.
#[test]
fn smoothed_value_step_is_15_ms_at_every_effective_rate() {
    for base_rate_hz in [44_100.0, 48_000.0, 96_000.0] {
        for oversampling in [1_u8, 2, 4] {
            let effective_rate = base_rate_hz * f64::from(oversampling);
            let mut smoother = SmoothedValue::new(0.0, effective_rate as f32, 15.0);
            smoother.set_target(1.0);
            let mut crossing_sample = None;
            for sample in 0..(effective_rate as usize) {
                if smoother.next_value() >= 0.632 {
                    crossing_sample = Some(sample + 1);
                    break;
                }
            }
            let crossing_sample = crossing_sample.expect("cruce alcanzado");
            let crossing_ms = crossing_sample as f64 / effective_rate * 1_000.0;
            let one_sample_ms = 1_000.0 / effective_rate;
            assert!(
                (crossing_ms - 15.0).abs() <= one_sample_ms,
                "{base_rate_hz} Hz {oversampling}×: cruce en {crossing_ms:.4} ms"
            );
        }
    }
}

/// La primera sincronización de mandos y `reset()` saltan al objetivo sin
/// rampa (comportamiento documentado del motor desde la Ola 8).
#[test]
fn initial_set_controls_and_reset_snap_without_ramp() {
    let mut engine = engine(4);
    engine.set_controls(RuntimeControls {
        output_trim_db: -40.0,
        ..RuntimeControls::default()
    });

    let input = sine(SETTLE_FRAMES, 0);
    let mut first = vec![0.0_f32; SETTLE_FRAMES];
    engine.process_mono(&input, &mut first);

    let head = peak(&first[..CYCLE]);
    let tail = peak(&first[SETTLE_FRAMES - CYCLE..]);
    assert!(tail > 0.0, "sin señal de referencia");
    assert!(
        head <= tail * 1.5,
        "rampa en la primera sincronización: cabeza {head}, cola {tail}"
    );

    engine.reset();
    let mut after_reset = vec![0.0_f32; SETTLE_FRAMES];
    engine.process_mono(&input, &mut after_reset);
    let head_reset = peak(&after_reset[..CYCLE]);
    assert!(
        head_reset <= tail * 1.5,
        "rampa tras reset: {head_reset} vs {tail}"
    );
}
