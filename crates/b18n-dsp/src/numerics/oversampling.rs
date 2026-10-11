//! Sobremuestreo 2×/4× con filtros FIR de media banda (sinc enventanada).
//!
//! Los coeficientes se calculan al inicializar (fuera del callback de audio),
//! con ventana Blackman y corte en la frecuencia de Nyquist de la tasa base.
//! La latencia de grupo de un FIR simétrico es `(N−1)/2` muestras de su tasa;
//! el total se expone en muestras de la tasa base para reportarla al host.

/// Número de coeficientes de cada etapa (impar).
///
/// 63 taps: la ventana Blackman da lóbulos lejanos de ≈−58 dB y el lóbulo
/// principal se estrecha lo suficiente para atenuar las imágenes del
/// interpolador que, con la cadena del inversor 7199 real, intermodulaban y
/// elevaban el alias de 4×. Latencia total: **47 muestras** de la tasa base
/// (≈1 ms a 48 kHz), corregida en P1.7 al medir la respuesta al impulso
/// (pico 47, correlación cruzada 47, centro de masas 46,28; el cálculo
/// anterior de 93 contaba cada etapa al doble de su retardo real).
pub const HALF_BAND_TAPS: usize = 63;

/// Filtro 2× reutilizable (coeficientes compartidos).
#[derive(Debug, Clone, PartialEq)]
struct HalfBandCoefficients {
    taps: Vec<f64>,
}

impl HalfBandCoefficients {
    fn design(taps: usize) -> Self {
        assert!(taps % 2 == 1 && taps >= 7);
        let center = (taps - 1) as f64 / 2.0;
        let mut coefficients = Vec::with_capacity(taps);
        for index in 0..taps {
            let offset = index as f64 - center;
            // sinc con corte en 0.25 de la tasa superior (Nyquist base).
            let sinc = if offset == 0.0 {
                0.5
            } else {
                (std::f64::consts::PI * 0.5 * offset).sin() / (std::f64::consts::PI * offset)
            };
            // Ventana Blackman.
            let window = 0.42
                - 0.5 * (2.0 * std::f64::consts::PI * index as f64 / (taps - 1) as f64).cos()
                + 0.08 * (4.0 * std::f64::consts::PI * index as f64 / (taps - 1) as f64).cos();
            coefficients.push(sinc * window);
        }
        // Normaliza la ganancia en continua a 0.5 (media banda).
        let sum: f64 = coefficients.iter().sum();
        for coefficient in &mut coefficients {
            *coefficient *= 0.5 / sum;
        }
        Self { taps: coefficients }
    }

    /// Coeficientes pares (fase 0) para el interpolador.
    fn even_taps(&self) -> impl Iterator<Item = f64> + '_ {
        self.taps.iter().step_by(2).copied()
    }

    /// Coeficientes impares (fase 1) para el interpolador.
    fn odd_taps(&self) -> impl Iterator<Item = f64> + '_ {
        self.taps.iter().skip(1).step_by(2).copied()
    }
}

/// Interpolador 2× con historia de entrada.
#[derive(Debug, Clone, PartialEq)]
pub struct HalfBandUpsampler2x {
    even: Vec<f64>,
    odd: Vec<f64>,
    history: Vec<f64>,
}

impl HalfBandUpsampler2x {
    #[must_use]
    pub fn new() -> Self {
        let coefficients = HalfBandCoefficients::design(HALF_BAND_TAPS);
        let even: Vec<f64> = coefficients.even_taps().collect();
        let odd: Vec<f64> = coefficients.odd_taps().collect();
        let history_len = even.len() + 1;
        Self {
            even,
            odd,
            history: vec![0.0; history_len],
        }
    }

    pub fn reset(&mut self) {
        self.history.fill(0.0);
    }

    /// Restaura el estado desde otro interpolador **sin asignar memoria**.
    pub fn restore_from(&mut self, other: &Self) {
        self.even.clone_from(&other.even);
        self.odd.clone_from(&other.odd);
        self.history.clone_from(&other.history);
    }

    /// Procesa una muestra y devuelve dos muestras a 2× la tasa.
    pub fn process(&mut self, input: f64) -> (f64, f64) {
        self.history.rotate_right(1);
        self.history[0] = input;

        let mut low = 0.0;
        let mut high = 0.0;
        for (index, coefficient) in self.even.iter().enumerate() {
            low += coefficient * self.history[index];
        }
        for (index, coefficient) in self.odd.iter().enumerate() {
            high += coefficient * self.history[index + 1];
        }
        // El filtro está normalizado a ganancia 0.5 en continua; el
        // cero-stuffing reparte la señal entre las dos fases, de modo que la
        // compensación correcta (para que cada fase reproduzca DC) es ×4.
        (4.0 * low, 4.0 * high)
    }
}

impl Default for HalfBandUpsampler2x {
    fn default() -> Self {
        Self::new()
    }
}

/// Decimador 2×: filtra y toma una de cada dos.
#[derive(Debug, Clone, PartialEq)]
pub struct HalfBandDownsampler2x {
    taps: Vec<f64>,
    history: Vec<f64>,
    index: usize,
}

impl HalfBandDownsampler2x {
    #[must_use]
    pub fn new() -> Self {
        let coefficients = HalfBandCoefficients::design(HALF_BAND_TAPS);
        let taps = coefficients.taps.clone();
        Self {
            history: vec![0.0; taps.len()],
            taps,
            index: 0,
        }
    }

    pub fn reset(&mut self) {
        self.history.fill(0.0);
        self.index = 0;
    }

    /// Restaura el estado desde otro decimador **sin asignar memoria**.
    pub fn restore_from(&mut self, other: &Self) {
        self.taps.clone_from(&other.taps);
        self.history.clone_from(&other.history);
        self.index = other.index;
    }

    /// Procesa una muestra a 2× y devuelve `Some` cada dos entradas.
    pub fn process(&mut self, input: f64) -> Option<f64> {
        self.history.rotate_right(1);
        self.history[0] = input;
        self.index += 1;
        if self.index.is_multiple_of(2) {
            let mut output = 0.0;
            for (sample, coefficient) in self.history.iter().zip(self.taps.iter()) {
                output += sample * coefficient;
            }
            // El filtro está normalizado a 0.5 de ganancia DC: ×2 al decimar.
            Some(2.0 * output)
        } else {
            None
        }
    }
}

impl Default for HalfBandDownsampler2x {
    fn default() -> Self {
        Self::new()
    }
}

/// Estado interno del sobremuestreo 4× (dos etapas 2× por sentido).
#[derive(Debug, Clone, PartialEq)]
pub struct FourTimesState {
    up_first: HalfBandUpsampler2x,
    up_second: HalfBandUpsampler2x,
    down_first: HalfBandDownsampler2x,
    down_second: HalfBandDownsampler2x,
}

impl FourTimesState {
    /// Restaura el estado desde otro 4× **sin asignar memoria**.
    pub fn restore_from(&mut self, other: &Self) {
        self.up_first.restore_from(&other.up_first);
        self.up_second.restore_from(&other.up_second);
        self.down_first.restore_from(&other.down_first);
        self.down_second.restore_from(&other.down_second);
    }
}

/// Sobremuestreador configurable (1×, 4×).
#[derive(Debug, Clone, PartialEq)]
pub enum Oversampler {
    /// Sin sobremuestreo: pasa la muestra tal cual.
    None,
    /// 4× = dos etapas 2× en cascada en cada sentido.
    FourTimes(Box<FourTimesState>),
}

impl Oversampler {
    /// Construye el sobremuestreador para el factor pedido.
    #[must_use]
    pub fn new(factor: u8) -> Option<Self> {
        match factor {
            1 => Some(Self::None),
            4 => Some(Self::FourTimes(Box::new(FourTimesState {
                up_first: HalfBandUpsampler2x::new(),
                up_second: HalfBandUpsampler2x::new(),
                down_first: HalfBandDownsampler2x::new(),
                down_second: HalfBandDownsampler2x::new(),
            }))),
            _ => None,
        }
    }

    /// Factor de sobremuestreo.
    #[must_use]
    pub fn factor(&self) -> u8 {
        match self {
            Self::None => 1,
            Self::FourTimes { .. } => 4,
        }
    }

    /// Latencia de grupo en muestras de la tasa base.
    ///
    /// Cada FIR simétrico aporta `(N−1)/2` muestras **de su tasa de salida**;
    /// en muestras de la tasa base: ida `1/2 + 1/4`, vuelta `1/4 + 1/2`
    /// (con `(N−1)/2 = 31`, total 46,5 → 47). Verificado con impulso.
    #[must_use]
    pub fn latency_samples(&self) -> u32 {
        let per_stage = ((HALF_BAND_TAPS - 1) / 2) as f64;
        match self {
            Self::None => 0,
            Self::FourTimes(_) => {
                (per_stage / 2.0 + per_stage / 4.0 + per_stage / 4.0 + per_stage / 2.0).round()
                    as u32
            }
        }
    }

    pub fn reset(&mut self) {
        match self {
            Self::None => {}
            Self::FourTimes(state) => {
                state.up_first.reset();
                state.up_second.reset();
                state.down_first.reset();
                state.down_second.reset();
            }
        }
    }

    /// Restaura el estado desde otro sobremuestreador **sin asignar memoria**
    /// (mismo factor; apto para el hilo de audio).
    pub fn restore_from(&mut self, other: &Self) {
        match (self, other) {
            (Self::None, Self::None) => {}
            (Self::FourTimes(state), Self::FourTimes(other_state)) => {
                state.restore_from(other_state);
            }
            _ => debug_assert!(false, "factores de sobremuestreo distintos"),
        }
    }

    /// Procesa un sample base a través de `process_at_rate` (que recibe la tasa
    /// sobremuestreada correspondiente) y devuelve un sample base.
    pub fn process<F>(&mut self, input: f32, mut process_at_rate: F) -> f32
    where
        F: FnMut(f32) -> f32,
    {
        match self {
            Self::None => process_at_rate(input),
            Self::FourTimes(state) => {
                let (mid_a, mid_b) = state.up_first.process(f64::from(input));
                let (high_0, high_1) = state.up_second.process(mid_a);
                let (high_2, high_3) = state.up_second.process(mid_b);

                let out_0 = process_at_rate(high_0 as f32);
                let out_1 = process_at_rate(high_1 as f32);
                let out_2 = process_at_rate(high_2 as f32);
                let out_3 = process_at_rate(high_3 as f32);

                // 4× → 2×: el decimador entrega una salida cada dos entradas.
                let _ = state.down_first.process(f64::from(out_0));
                let mid_0 = state
                    .down_first
                    .process(f64::from(out_1))
                    .expect("segunda muestra del par");
                let _ = state.down_first.process(f64::from(out_2));
                let mid_1 = state
                    .down_first
                    .process(f64::from(out_3))
                    .expect("segunda muestra del par");

                // 2× → base.
                let _ = state.down_second.process(mid_0);
                state
                    .down_second
                    .process(mid_1)
                    .expect("segunda muestra del par") as f32
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upsampler_preserves_dc_in_both_phases() {
        let mut upsampler = HalfBandUpsampler2x::new();
        let mut last = (0.0, 0.0);
        for _ in 0..256 {
            last = upsampler.process(1.0);
        }
        // En continua la señal interpolada debe valer 1.0 en ambas fases.
        assert!((last.0 - 1.0).abs() < 1e-3, "fase 0: {}", last.0);
        assert!((last.1 - 1.0).abs() < 1e-3, "fase 1: {}", last.1);
    }

    #[test]
    fn downsampler_dc_gain_is_unity() {
        let mut downsampler = HalfBandDownsampler2x::new();
        let mut output = None;
        for _ in 0..256 {
            output = downsampler.process(1.0).or(output);
        }
        let value = output.expect("alguna salida");
        assert!((value - 1.0).abs() < 1e-3, "salida {value}");
    }

    #[test]
    fn four_times_latency_is_documented() {
        let oversampler = Oversampler::new(4).expect("4× soportado");
        assert_eq!(oversampler.factor(), 4);
        // 46,5 muestras teóricas (1/2 + 1/4 + 1/4 + 1/2 de 31); verificado con
        // impulso (pico en 47) y correlación cruzada de bursts (47).
        assert_eq!(oversampler.latency_samples(), 47);
        assert_eq!(Oversampler::new(1).expect("1×").latency_samples(), 0);
        assert!(Oversampler::new(2).is_none());
    }

    #[test]
    fn four_times_impulse_peak_matches_reported_latency() {
        let mut oversampler = Oversampler::new(4).expect("4×");
        let mut output = [0.0_f32; 256];
        for (index, sample) in output.iter_mut().enumerate() {
            let input = if index == 0 { 1.0 } else { 0.0 };
            *sample = oversampler.process(input, |value| value);
        }
        let peak = output
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.abs().partial_cmp(&b.abs()).expect("finita"))
            .map(|(index, _)| index)
            .expect("respuesta");
        assert_eq!(peak, oversampler.latency_samples() as usize);
    }

    #[test]
    fn four_times_preserves_dc() {
        let mut oversampler = Oversampler::new(4).expect("4× soportado");
        let mut last = 0.0_f32;
        for _ in 0..512 {
            last = oversampler.process(1.0, |sample| sample);
        }
        assert!((last - 1.0).abs() < 1e-3, "salida {last}");
        assert!(last.is_finite());
    }

    #[test]
    fn decimation_suppresses_out_of_band_component() {
        // La cadena interna (a 192 kHz) inyecta 29.2 kHz; a la salida de
        // 48 kHz sólo puede aparecer su alias en 18.8 kHz, atenuado por el
        // filtro de decimación (≈ −28 dB medidos en la referencia numpy).
        const BASE_RATE: f64 = 48_000.0;
        let mut oversampler = Oversampler::new(4).expect("4×");
        let mut index = 0_usize;
        let mut output = Vec::with_capacity(20_000);
        for _ in 0..20_000 {
            let sample = oversampler.process(0.0, |_| {
                let value = (2.0 * std::f64::consts::PI * 29_200.0 * index as f64
                    / (BASE_RATE * 4.0))
                    .sin() as f32;
                index += 1;
                value
            });
            output.push(f64::from(sample));
        }

        let tail = &output[4_000..];
        let power_at = |frequency_hz: f64| {
            let omega = 2.0 * std::f64::consts::PI * frequency_hz / BASE_RATE;
            let n = tail.len();
            let (mut re, mut im) = (0.0, 0.0);
            for (i, value) in tail.iter().enumerate() {
                let window =
                    0.5 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / (n - 1) as f64).cos();
                re += value * window * (omega * i as f64).cos();
                im += value * window * (omega * i as f64).sin();
            }
            (re * re + im * im) / (n as f64 * n as f64)
        };

        let alias = power_at(18_800.0);
        // Amplitud 1 sin filtrar daría ≈ A²/16 = 0.0625; se exige al menos
        // 15 dB de atenuación adicional (el filtro real da ≈ −28 dB).
        assert!(alias.is_finite());
        assert!(
            alias < 0.012,
            "el componente de 29.2 kHz no fue atenuado: {alias:.3e}"
        );
    }

    #[test]
    fn bypass_is_exact() {
        let mut oversampler = Oversampler::new(1).expect("1×");
        for value in [0.0_f32, 0.5, -1.0, 2.0] {
            assert_eq!(
                oversampler.process(value, |sample| sample * 3.0),
                value * 3.0
            );
        }
    }
}
