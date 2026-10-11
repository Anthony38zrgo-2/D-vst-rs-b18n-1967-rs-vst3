//! Conversiones de decibelios a ganancia lineal.

/// Convierte dB a ganancia lineal (`0 dB = 1.0`).
#[must_use]
pub fn db_to_gain(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}

/// Convierte ganancia lineal a dB. Un cero absoluto devuelve el mínimo
/// representable en vez de `-inf`, para no propagar valores no finitos.
#[must_use]
pub fn gain_to_db(gain: f32) -> f32 {
    20.0 * gain.abs().max(f32::MIN_POSITIVE).log10()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_within_tolerance() {
        for db in [-60.0_f32, -12.0, 0.0, 6.0, 24.0] {
            let roundtrip = gain_to_db(db_to_gain(db));
            assert!((roundtrip - db).abs() < 1e-4, "db {db} -> {roundtrip}");
        }
    }

    #[test]
    fn zero_gain_is_finite() {
        assert!(gain_to_db(0.0).is_finite());
    }
}
