//! Contraste AC del inversor 7199 contra ngspice (P1.4).
//!
//! Golden: `assets/fixtures/reference/phase_7199/b18n_7199_ac_1967_golden.csv`
//! (excitación AC unitaria en la rejilla del pentodo; salidas tras el acoplo de
//! 0.05 µF con el retorno de 100 kΩ de la 7027A).

use b18n_dsp::stages::phase_splitter::PhaseInverter7199;

const GOLDEN: &str =
    include_str!("../../../assets/fixtures/reference/phase_7199/b18n_7199_ac_1967_golden.csv");

const TRAN_GOLDEN: &str =
    include_str!("../../../assets/fixtures/reference/phase_7199/b18n_7199_tran_1967_golden.txt");

const SAMPLE_RATE_HZ: f64 = 48_000.0;
const AMPLITUDE: f64 = 0.005;

/// Fasor de la componente en `frequency_hz` (ventana rectangular, periodos
/// enteros: sin fuga espectral). Convención `v(t) = Re{V·e^{jωt}}`, igual que
/// `ph()` de ngspice: `im −= x·sin(ωn)`.
fn phasor(samples: &[f64], frequency_hz: f64) -> (f64, f64) {
    let omega = 2.0 * std::f64::consts::PI * frequency_hz / SAMPLE_RATE_HZ;
    let (mut re, mut im) = (0.0, 0.0);
    for (index, sample) in samples.iter().enumerate() {
        re += sample * (omega * index as f64).cos();
        im -= sample * (omega * index as f64).sin();
    }
    (re, im)
}

fn wrap_degrees(value: f64) -> f64 {
    (value + 180.0).rem_euclid(360.0) - 180.0
}

#[test]
fn ac_response_matches_spice() {
    let circuit = b18n_config::load_reference().expect("modelo con errata");
    let mut stage = PhaseInverter7199::from_circuit(&circuit, SAMPLE_RATE_HZ).expect("inversor");

    let mut compared = 0_usize;
    for (index, line) in GOLDEN.lines().enumerate() {
        if index == 0 {
            continue;
        }
        let fields: Vec<f64> = line
            .split(',')
            .map(|field| field.trim().parse().expect("campo numérico"))
            .collect();
        let frequency = fields[0];
        let (golden_mag_up, golden_phase_up, golden_mag_down, golden_phase_down) =
            (fields[1], fields[2], fields[3], fields[4]);

        // La red de acoplo tiene su polo en ~3 Hz (20 nF con 10M/3.3M): se
        // asienta al menos 1 s y se mide al menos 0,5 s para no contaminar la
        // respuesta en baja frecuencia.
        let settle = (4.0 * SAMPLE_RATE_HZ / frequency)
            .ceil()
            .max(SAMPLE_RATE_HZ) as usize;
        let measure = (16.0 * SAMPLE_RATE_HZ / frequency)
            .ceil()
            .max(SAMPLE_RATE_HZ / 2.0) as usize;
        let total = settle + measure;

        let mut input = Vec::with_capacity(measure);
        let mut upper = Vec::with_capacity(measure);
        let mut lower = Vec::with_capacity(measure);
        for sample_index in 0..total {
            let value = AMPLITUDE
                * (2.0 * std::f64::consts::PI * frequency * sample_index as f64 / SAMPLE_RATE_HZ)
                    .sin();
            let (up, down) = stage.process(value, 0.0);
            if sample_index >= settle {
                input.push(value);
                upper.push(up);
                lower.push(down);
            }
        }

        let (in_re, in_im) = phasor(&input, frequency);
        let (up_re, up_im) = phasor(&upper, frequency);
        let (dn_re, dn_im) = phasor(&lower, frequency);
        let in_norm = (in_re * in_re + in_im * in_im).sqrt().max(1e-30);
        let up_mag = (up_re * up_re + up_im * up_im).sqrt() / in_norm;
        let dn_mag = (dn_re * dn_re + dn_im * dn_im).sqrt() / in_norm;
        let up_phase = (up_im.atan2(up_re) - in_im.atan2(in_re)).to_degrees();
        let dn_phase = (dn_im.atan2(dn_re) - in_im.atan2(in_re)).to_degrees();

        let mag_up_error = (up_mag - golden_mag_up).abs() / golden_mag_up;
        let mag_dn_error = (dn_mag - golden_mag_down).abs() / golden_mag_down;
        let phase_up_error = wrap_degrees(up_phase - golden_phase_up).abs();
        let phase_dn_error = wrap_degrees(dn_phase - golden_phase_down).abs();

        let magnitude_error = mag_up_error.max(mag_dn_error);
        let phase_error = phase_up_error.max(phase_dn_error);

        if std::env::var_os("B18N_PHASE_REPORT").is_some() {
            println!(
                "f={frequency:>6.0} Hz: mag_up {:.4}% mag_dn {:.4}% fase_up {:.3}° fase_dn {:.3}°",
                mag_up_error * 100.0,
                mag_dn_error * 100.0,
                phase_up_error,
                phase_dn_error
            );
        }

        // Tolerancias documentadas tras B18-008 (red de acoplo resuelta de
        // forma acoplada con el pentodo): medido ≤0,0002 % y ≤0,05° en
        // 20 Hz–20 kHz a 48 kHz; se exige 0,1 % y 0,2° con margen.
        assert!(
            magnitude_error < 0.001 && phase_error < 0.2,
            "f={frequency} Hz: magnitud {:.4}%, fase {:.3}°",
            magnitude_error * 100.0,
            phase_error
        );
        compared += 1;
    }
    assert_eq!(compared, 10, "golden AC incompleto");
}

/// Contraste transitorio: 1 kHz, 0.2 V en la rejilla, ventana 15-20 ms.
#[test]
fn transient_matches_spice() {
    let mut expected = std::collections::BTreeMap::new();
    for line in TRAN_GOLDEN.lines() {
        if line.trim_start().starts_with('#') || !line.contains('=') {
            continue;
        }
        let (key, value) = line.split_once('=').expect("clave=valor");
        expected.insert(
            key.trim().to_string(),
            value.trim().parse::<f64>().expect("valor numérico"),
        );
    }

    let circuit = b18n_config::load_reference().expect("modelo con errata");
    let mut stage = PhaseInverter7199::from_circuit(&circuit, SAMPLE_RATE_HZ).expect("inversor");

    let frequency = 1_000.0_f64;
    let amplitude = 0.2_f64;
    let frames = (20.0 * SAMPLE_RATE_HZ / 1_000.0) as usize; // 20 ms
    let window_start = frames * 3 / 4;
    let (mut peak_up, mut min_up, mut peak_dn, mut min_dn) =
        (f64::MIN, f64::MAX, f64::MIN, f64::MAX);
    for index in 0..frames {
        let value = amplitude
            * (2.0 * std::f64::consts::PI * frequency * index as f64 / SAMPLE_RATE_HZ).sin();
        let (up, down) = stage.process(value, 0.0);
        if index >= window_start {
            peak_up = peak_up.max(up);
            min_up = min_up.min(up);
            peak_dn = peak_dn.max(down);
            min_dn = min_dn.min(down);
        }
    }

    // La asimetría de la válvula hace que pico y valle difieran; tras B18-008
    // (red de acoplo acoplada) medido ≤0,03 %; se compara con 0,5 % de margen.
    for (label, measured) in [
        ("peak_up_v", peak_up),
        ("min_up_v", min_up),
        ("peak_down_v", peak_dn),
        ("min_down_v", min_dn),
    ] {
        let golden = expected[label];
        let error = (measured - golden).abs() / golden.abs();
        assert!(
            error < 0.005,
            "{label}: Rust {measured:.4} vs SPICE {golden:.4} ({:.2}%)",
            error * 100.0
        );
    }
    assert!(peak_up > 0.0 && min_up < 0.0, "polaridad de la placa");
    assert!(peak_dn > 0.0 && min_dn < 0.0, "polaridad del cátodo");
}
