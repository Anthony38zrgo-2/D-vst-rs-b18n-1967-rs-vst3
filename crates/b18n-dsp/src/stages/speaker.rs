//! Approximate acoustic cabinet profiles, not measured IRs or reactive loads.
//! All frequencies are modern voicing choices; the amplifier's 8-ohm load
//! and feedback remain unchanged. Causal IIRs add no buffering latency.
use crate::controls::SpeakerProfile;
use crate::utilities::smoothing::SmoothedValue;
use std::f64::consts::PI;

#[derive(Clone, Copy)]
enum FilterKind {
    LowPass,
    HighPass,
    Peak,
}

#[derive(Clone, Copy)]
struct Biquad {
    b: [f64; 3],
    a: [f64; 2],
    z: [f64; 2],
}
impl Biquad {
    fn new(rate: f64, hz: f64, kind: FilterKind, db: f64) -> Self {
        let omega = 2.0 * PI * hz.min(rate * 0.45) / rate;
        let c = omega.cos();
        let alpha = omega.sin() / (2.0 * std::f64::consts::FRAC_1_SQRT_2);
        let (b, a) = match kind {
            FilterKind::LowPass => (
                [(1.0 - c) / 2.0, 1.0 - c, (1.0 - c) / 2.0],
                [1.0 + alpha, -2.0 * c, 1.0 - alpha],
            ),
            FilterKind::HighPass => (
                [(1.0 + c) / 2.0, -(1.0 + c), (1.0 + c) / 2.0],
                [1.0 + alpha, -2.0 * c, 1.0 - alpha],
            ),
            FilterKind::Peak => {
                let gain = 10.0_f64.powf(db / 40.0);
                (
                    [1.0 + alpha * gain, -2.0 * c, 1.0 - alpha * gain],
                    [1.0 + alpha / gain, -2.0 * c, 1.0 - alpha / gain],
                )
            }
        };
        Self {
            b: b.map(|v| v / a[0]),
            a: [a[1] / a[0], a[2] / a[0]],
            z: [0.0; 2],
        }
    }
    fn process(&mut self, x: f64) -> f64 {
        let y = self.b[0] * x + self.z[0];
        self.z[0] = self.b[1] * x - self.a[0] * y + self.z[1];
        self.z[1] = self.b[2] * x - self.a[1] * y;
        y
    }
}

pub(crate) struct SpeakerCabinet {
    eighteen: [Biquad; 3],
    fifteen: [Biquad; 3],
    mix18: SmoothedValue,
    mix15: SmoothedValue,
}
impl SpeakerCabinet {
    pub(crate) fn new(rate: f64) -> Self {
        Self {
            eighteen: [
                Biquad::new(rate, 40.0, FilterKind::HighPass, 0.0),
                Biquad::new(rate, 80.0, FilterKind::Peak, 2.0),
                Biquad::new(rate, 3200.0, FilterKind::LowPass, 0.0),
            ],
            fifteen: [
                Biquad::new(rate, 50.0, FilterKind::HighPass, 0.0),
                Biquad::new(rate, 110.0, FilterKind::Peak, 2.0),
                Biquad::new(rate, 4200.0, FilterKind::LowPass, 0.0),
            ],
            mix18: SmoothedValue::new(0.0, rate as f32, 15.0),
            mix15: SmoothedValue::new(0.0, rate as f32, 15.0),
        }
    }
    pub(crate) fn set_profile(&mut self, profile: SpeakerProfile) {
        self.mix18
            .set_target(if profile == SpeakerProfile::Eighteen {
                1.0
            } else {
                0.0
            });
        self.mix15
            .set_target(if profile == SpeakerProfile::Fifteen {
                1.0
            } else {
                0.0
            });
    }
    pub(crate) fn snap(&mut self) {
        self.mix18.snap();
        self.mix15.snap();
    }
    pub(crate) fn reset(&mut self) {
        for filter in self.eighteen.iter_mut().chain(self.fifteen.iter_mut()) {
            filter.z = [0.0; 2];
        }
        self.snap();
    }
    pub(crate) fn process(&mut self, input: f32) -> f32 {
        let x = f64::from(input);
        // Keep both profiles warm so switching does not start cold filters.
        let y18 = self
            .eighteen
            .iter_mut()
            .fold(x, |x, filter| filter.process(x));
        let y15 = self
            .fifteen
            .iter_mut()
            .fold(x, |x, filter| filter.process(x));
        let m18 = f64::from(self.mix18.next_value());
        let m15 = f64::from(self.mix15.next_value());
        (x + m18 * (y18 - x) + m15 * (y15 - x)) as f32
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn rms(profile: SpeakerProfile, hz: f64, rate: f64) -> f64 {
        let mut cab = SpeakerCabinet::new(rate);
        cab.set_profile(profile);
        cab.snap();
        let mut sum = 0.0;
        for i in 0..8192 {
            let y = cab.process((2.0 * PI * hz * i as f64 / rate).sin() as f32);
            assert!(y.is_finite());
            if i >= 4096 {
                sum += f64::from(y).powi(2);
            }
        }
        (sum / 4096.0).sqrt()
    }
    #[test]
    fn direct_is_exact_and_profiles_have_distinct_bass_and_treble() {
        let mut cab = SpeakerCabinet::new(48000.0);
        for i in 0..4096 {
            let x = (i as f32 * 0.07).sin();
            assert_eq!(cab.process(x), x);
        }
        for rate in [8000.0, 44100.0, 48000.0, 96000.0, 192000.0] {
            assert!(
                rms(SpeakerProfile::Eighteen, 30.0, rate)
                    > rms(SpeakerProfile::Fifteen, 30.0, rate)
            );
            assert!(
                rms(SpeakerProfile::Eighteen, 2500.0, rate)
                    < rms(SpeakerProfile::Fifteen, 2500.0, rate)
            );
        }
    }
    #[test]
    fn switch_is_smoothed_and_reset_is_reproducible() {
        let mut cab = SpeakerCabinet::new(48000.0);
        for _ in 0..4096 {
            cab.process(0.5);
        }
        cab.set_profile(SpeakerProfile::Eighteen);
        assert!((cab.process(0.5) - 0.5).abs() < 0.01);
        cab.reset();
        let first: Vec<_> = (0..4096)
            .map(|i| cab.process(if i == 0 { 1.0 } else { 0.0 }))
            .collect();
        cab.reset();
        let second: Vec<_> = (0..4096)
            .map(|i| cab.process(if i == 0 { 1.0 } else { 0.0 }))
            .collect();
        assert_eq!(first, second);
    }
}
