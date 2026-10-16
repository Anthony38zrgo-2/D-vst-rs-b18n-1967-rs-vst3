//! B18-012: dinámica de la fuente (5AR4 + RC + bias).
//!
//! Red trazada SCH-567 (Hipótesis A, `estimated`): H2 [30 µF] -> 1k ->
//! H1 [40 µF, pantallas] -> 10k -> H3 [40 µF, triodo] y 22k -> H4 [40 µF,
//! previo]. Rectificador como Thevenin promedio (sin curva 5AR4 ni DCR
//! documentadas). Contraste DC contra ngspice (`power_supply/`).

use b18n_dsp::{B18nEngine, PowerSupply, PowerSupplyConfig, ProcessSpec, SupplyLoad};

const DC_GOLDEN: &str = include_str!(
    "../../../assets/fixtures/reference/power_supply/b18n_power_supply_dc_1967_golden.txt"
);
const TRAN_GOLDEN: &str = include_str!(
    "../../../assets/fixtures/reference/power_supply/b18n_power_supply_tran_1967_golden.csv"
);

fn test_config(sample_rate_hz: f64) -> PowerSupplyConfig {
    PowerSupplyConfig {
        rectifier_source_v: 518.935,
        rectifier_series_ohms: 150.0,
        r_1k_ohms: 1_000.0,
        r_10k_ohms: 10_000.0,
        r_22k_ohms: 22_000.0,
        c_h2_farads: 30e-6,
        c_h1_farads: 40e-6,
        c_h3_farads: 40e-6,
        c_h4_farads: 40e-6,
        bias_source_v: -55.0,
        bias_series_ohms: 15_000.0,
        bias_cap_farads: 100e-6,
        sample_rate_hz,
    }
}

fn nominal_load() -> SupplyLoad {
    SupplyLoad::idle(0.083, 0.003, 0.0023, 0.0046)
}

fn parse_dc_golden() -> (f64, f64, f64, f64, f64) {
    let mut h2 = 0.0;
    let mut h1 = 0.0;
    let mut h3 = 0.0;
    let mut h4 = 0.0;
    let mut vbias = 0.0;
    for line in DC_GOLDEN.lines() {
        let line = line.trim();
        if line.starts_with("v(h2)") {
            h2 = line
                .split('=')
                .nth(1)
                .expect("h2")
                .trim()
                .parse()
                .expect("num");
        } else if line.starts_with("v(h1)") {
            h1 = line
                .split('=')
                .nth(1)
                .expect("h1")
                .trim()
                .parse()
                .expect("num");
        } else if line.starts_with("v(h3)") {
            h3 = line
                .split('=')
                .nth(1)
                .expect("h3")
                .trim()
                .parse()
                .expect("num");
        } else if line.starts_with("v(h4)") {
            h4 = line
                .split('=')
                .nth(1)
                .expect("h4")
                .trim()
                .parse()
                .expect("num");
        } else if line.starts_with("v(vbias)") {
            vbias = line
                .split('=')
                .nth(1)
                .expect("vb")
                .trim()
                .parse()
                .expect("num");
        }
    }
    (h2, h1, h3, h4, vbias)
}

#[test]
fn zero_current_gives_rectifier_source() {
    let supply = PowerSupply::new(test_config(48_000.0), nominal_load());
    let dc = supply.dc_solution(SupplyLoad::idle(0.0, 0.0, 0.0, 0.0));
    // Sin carga no hay caídas: todos los nodos HV a la fuente.
    assert!(
        (dc.bplus_ct_v - 518.935).abs() < 1e-9,
        "H2 {}",
        dc.bplus_ct_v
    );
    assert!((dc.screen_v - 518.935).abs() < 1e-9, "H1 {}", dc.screen_v);
    assert!((dc.triode_v - 518.935).abs() < 1e-9, "H3 {}", dc.triode_v);
    assert!((dc.preamp_v - 518.935).abs() < 1e-9, "H4 {}", dc.preamp_v);
    assert!((dc.bias_v + 55.0).abs() < 1e-9, "bias {}", dc.bias_v);
}

#[test]
fn nominal_current_recovers_to_rest() {
    let mut supply = PowerSupply::new(test_config(48_000.0), nominal_load());
    let rest = supply.rails();
    // B+ calibrado exacto; aguas abajo predice la red (contraste manual ±5 %).
    assert!(
        (rest.bplus_ct_v - 505.0).abs() < 0.5,
        "B+ {}",
        rest.bplus_ct_v
    );
    assert!(
        (rest.screen_v - 500.0).abs() / 500.0 < 0.05,
        "H1 {}",
        rest.screen_v
    );
    assert!(
        (rest.triode_v - 460.0).abs() / 460.0 < 0.08,
        "H3 {}",
        rest.triode_v
    );
    assert!(
        (rest.preamp_v - 380.0).abs() / 380.0 < 0.08,
        "H4 {}",
        rest.preamp_v
    );
    assert!((rest.bias_v + 55.0).abs() < 0.5, "bias {}", rest.bias_v);
    // Con la carga nominal no deriva (punto fijo del Euler implícito).
    for _ in 0..48_000 {
        supply.advance(nominal_load());
    }
    let settled = supply.rails();
    assert!((settled.bplus_ct_v - rest.bplus_ct_v).abs() < 1e-6);
    assert!((settled.screen_v - rest.screen_v).abs() < 1e-6);
    assert!((settled.preamp_v - rest.preamp_v).abs() < 1e-6);
}

#[test]
fn dc_matches_ngspice_golden() {
    let supply = PowerSupply::new(test_config(48_000.0), nominal_load());
    let dc = supply.dc_solution(nominal_load());
    let (h2, h1, h3, h4, vbias) = parse_dc_golden();
    // Misma red resistiva; el integrador no interviene en DC.
    assert!(
        (dc.bplus_ct_v - h2).abs() < 2.0,
        "H2 {} vs {h2}",
        dc.bplus_ct_v
    );
    assert!((dc.screen_v - h1).abs() < 2.0, "H1 {} vs {h1}", dc.screen_v);
    assert!((dc.triode_v - h3).abs() < 2.0, "H3 {} vs {h3}", dc.triode_v);
    assert!((dc.preamp_v - h4).abs() < 2.0, "H4 {} vs {h4}", dc.preamp_v);
    assert!(
        (dc.bias_v - vbias).abs() < 1.0,
        "bias {} vs {vbias}",
        dc.bias_v
    );
}

#[test]
fn tran_sag_matches_ngspice_step() {
    // Escalón +20 mA en H2 como el golden (sag ≈ 2.9 V en B+).
    let mut supply = PowerSupply::new(test_config(192_000.0), nominal_load());
    let rest = supply.rails().bplus_ct_v;
    let mut stepped = nominal_load();
    stepped.ct_a += 0.02;
    let mut min_bplus = rest;
    // 150 ms con el escalón (suficiente para el mínimo con τ ≈ 22 ms).
    for _ in 0..(0.15 * 192_000.0) as usize {
        let rails = supply.advance(stepped);
        min_bplus = min_bplus.min(rails.bplus_ct_v);
    }
    let sag = rest - min_bplus;
    // Golden ngspice: caída 2.91 V; Backward Euler vs trapezoidal: ±1 V.
    assert!(sag > 1.5 && sag < 4.5, "sag {sag:.2} V (rest {rest:.2})");
    // Al retirar el escalón recupera al reposo.
    for _ in 0..(0.5 * 192_000.0) as usize {
        supply.advance(nominal_load());
    }
    let recovered = supply.rails().bplus_ct_v;
    assert!(
        (recovered - rest).abs() < 0.5,
        "recupera {recovered:.2} vs {rest:.2}"
    );
    // El golden tran confirma el orden de magnitud (parseo mínimo).
    let mut tran_min = f64::INFINITY;
    let mut tran_pre = 0.0;
    let mut pre_count = 0_usize;
    for line in TRAN_GOLDEN.lines().skip(1) {
        let fields: Vec<&str> = line.split(',').collect();
        if fields.len() < 5 {
            continue;
        }
        let t: f64 = fields[0].parse().unwrap_or(f64::NAN);
        let v: f64 = fields[1].parse().unwrap_or(f64::NAN);
        if !t.is_finite() || !v.is_finite() {
            continue;
        }
        if t < 0.04 {
            tran_pre += v;
            pre_count += 1;
        }
        if (0.05..0.15).contains(&t) {
            tran_min = tran_min.min(v);
        }
    }
    let tran_pre = tran_pre / pre_count.max(1) as f64;
    let tran_sag = tran_pre - tran_min;
    assert!(
        tran_sag > 2.0 && tran_sag < 4.0,
        "golden sag {tran_sag:.2} V"
    );
}

#[test]
fn bursts_sag_and_recover_at_bass_frequencies() {
    // Ráfagas 40/60/100 Hz: la corriente de placa modulada hunde B+ y el
    // riel vuelve al reposo sin resets ni valores imposibles.
    for freq_hz in [40.0, 60.0, 100.0] {
        let rate = 192_000.0;
        let mut supply = PowerSupply::new(test_config(rate), nominal_load());
        let rest = supply.rails();
        let burst_len = (0.25 * rate) as usize;
        let mut min_bplus = rest.bplus_ct_v;
        for index in 0..burst_len {
            let t = index as f64 / rate;
            // Envolvente de ráfaga (ataque suave) + 50 mA pico a `freq`.
            let env = (index as f64 / burst_len as f64 * std::f64::consts::PI).sin();
            let mut load = nominal_load();
            load.ct_a += 0.05 * env * (2.0 * std::f64::consts::PI * freq_hz * t).sin().max(0.0);
            load.screen_a +=
                0.005 * env * (2.0 * std::f64::consts::PI * freq_hz * t).sin().max(0.0);
            let rails = supply.advance(load);
            min_bplus = min_bplus.min(rails.bplus_ct_v);
            assert!(rails.bplus_ct_v.is_finite() && rails.bplus_ct_v > 0.0);
            assert!(rails.bplus_ct_v < 650.0, "riel imposible");
            assert!(rails.bias_v.is_finite() && rails.bias_v <= 0.0);
        }
        assert!(
            rest.bplus_ct_v - min_bplus > 0.5,
            "{freq_hz} Hz: sin sag ({:.2} V)",
            rest.bplus_ct_v - min_bplus
        );
        assert!(
            rest.bplus_ct_v - min_bplus < 30.0,
            "{freq_hz} Hz: sag excesivo ({:.2} V)",
            rest.bplus_ct_v - min_bplus
        );
        for _ in 0..(0.5 * rate) as usize {
            supply.advance(nominal_load());
        }
        let recovered = supply.rails();
        assert!(
            (recovered.bplus_ct_v - rest.bplus_ct_v).abs() < 0.5,
            "{freq_hz} Hz: no recupera ({:.2} vs {:.2})",
            recovered.bplus_ct_v,
            rest.bplus_ct_v
        );
    }
}

#[test]
fn energy_is_consistent_and_bounded() {
    let mut supply = PowerSupply::new(test_config(48_000.0), nominal_load());
    let rest_energy = supply.capacitor_energy_j();
    assert!(rest_energy.is_finite() && rest_energy > 0.0);
    // Carga extra hunde tensiones -> energía menor (sin fuente ideal rígida).
    let mut heavy = nominal_load();
    heavy.ct_a += 0.05;
    for _ in 0..9_600 {
        supply.advance(heavy);
    }
    let sagged = supply.capacitor_energy_j();
    assert!(sagged < rest_energy, "{sagged} vs {rest_energy}");
    // La energía es suma de 1/2·C·V² con los valores del esquema.
    let rails = supply.rails();
    let expected = 0.5 * 30e-6 * rails.bplus_ct_v * rails.bplus_ct_v
        + 0.5 * 40e-6 * rails.screen_v * rails.screen_v
        + 0.5 * 40e-6 * rails.triode_v * rails.triode_v
        + 0.5 * 40e-6 * rails.preamp_v * rails.preamp_v
        + 0.5 * 100e-6 * rails.bias_v * rails.bias_v;
    assert!((sagged - expected).abs() / expected < 1e-12);
}

#[test]
fn one_sample_lag_error_is_bounded() {
    // Cuantifica el `z^-1` de la fuente: misma excitación con la carga exacta
    // frente a la retrasada una muestra (lo que hace el motor).
    let rate = 192_000.0;
    let mut exact = PowerSupply::new(test_config(rate), nominal_load());
    let mut lagged = PowerSupply::new(test_config(rate), nominal_load());
    let mut previous = nominal_load();
    let mut worst = 0.0_f64;
    for index in 0..(0.2 * rate) as usize {
        let t = index as f64 / rate;
        let mut load = nominal_load();
        load.ct_a += 0.04 * (2.0 * std::f64::consts::PI * 100.0 * t).sin();
        let rails_exact = exact.advance(load);
        let rails_lag = lagged.advance(previous);
        previous = load;
        worst = worst
            .max((rails_exact.bplus_ct_v - rails_lag.bplus_ct_v).abs())
            .max((rails_exact.screen_v - rails_lag.screen_v).abs());
    }
    // Con τ ≈ 22 ms >> dt ≈ 5 µs el error de una muestra es despreciable
    // frente al sag (V·sag ≈ 2-3 V): se documenta la cota medida.
    assert!(worst < 0.05, "error de lag {worst:.4} V");
}

#[test]
fn reset_returns_to_rest_without_clicks() {
    let mut supply = PowerSupply::new(test_config(48_000.0), nominal_load());
    let rest = supply.rails();
    let mut heavy = nominal_load();
    heavy.ct_a += 0.1;
    for _ in 0..4_800 {
        supply.advance(heavy);
    }
    assert!((supply.rails().bplus_ct_v - rest.bplus_ct_v).abs() > 1.0);
    supply.reset();
    let after = supply.rails();
    assert!((after.bplus_ct_v - rest.bplus_ct_v).abs() < 1e-12);
    assert!((after.preamp_v - rest.preamp_v).abs() < 1e-12);
    assert!((after.bias_v - rest.bias_v).abs() < 1e-12);
}

#[test]
fn engine_sags_under_bass_transient_and_recovers() {
    // Integración: el motor hunde B+ con graves fuertes y vuelve al reposo.
    let circuit = b18n_config::load_reference().expect("modelo histórico cargable");
    let spec = ProcessSpec {
        sample_rate_hz: 48_000.0,
        max_block_frames: 1024,
        oversampling: 4,
    };
    let mut engine = B18nEngine::new(circuit, spec).expect("motor");
    let rest = engine.supply_rails();
    assert!(
        (rest.bplus_ct_v - 505.0).abs() < 5.0,
        "B+ {:?}",
        rest.bplus_ct_v
    );
    // Ráfaga de 55 Hz a nivel alto (1 V pico = nivel DAW fuerte).
    let rate = 48_000.0;
    let frames = (0.3 * rate) as usize;
    let input: Vec<f32> = (0..frames)
        .map(|i| (2.0 * std::f64::consts::PI * 55.0 * i as f64 / rate).sin() as f32)
        .collect();
    let mut output = vec![0.0_f32; frames];
    engine.process_mono(&input, &mut output);
    assert!(output.iter().all(|s| s.is_finite()));
    let sagged = engine.supply_rails();
    assert!(
        sagged.bplus_ct_v < rest.bplus_ct_v,
        "B+ no hunde: {:.2} vs {:.2}",
        sagged.bplus_ct_v,
        rest.bplus_ct_v
    );
    assert!(
        rest.bplus_ct_v - sagged.bplus_ct_v < 30.0,
        "sag excesivo: {:.2} V",
        rest.bplus_ct_v - sagged.bplus_ct_v
    );
    // Silencio posterior: recupera al reposo.
    let silence = vec![0.0_f32; (0.5 * rate) as usize];
    let mut quiet = vec![0.0_f32; silence.len()];
    engine.process_mono(&silence, &mut quiet);
    let recovered = engine.supply_rails();
    assert!(
        (recovered.bplus_ct_v - rest.bplus_ct_v).abs() < 1.0,
        "no recupera: {:.2} vs {:.2}",
        recovered.bplus_ct_v,
        rest.bplus_ct_v
    );
}
