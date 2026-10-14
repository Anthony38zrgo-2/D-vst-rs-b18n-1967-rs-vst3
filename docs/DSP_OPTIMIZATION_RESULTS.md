# DSP optimization: tone, 7027A and tube curves

Scope: preserve circuit parameters, Newton tolerances, capacitor dynamics and oversampling latency. No additional runtime dependencies.

- Tone: rebuild conductances only when sanitized Bass/Treble change. RealMna caches upper-triangular elimination, pivot swaps and elimination factors; each sample supplies a new RHS. Reset and restore retain coherent cache state. Automation still updates at the original sample cadence.
- 7027A: prepare screen/grid-dependent Koren plate terms once per tube/sample, reuse through plate Newton and final diagnostic current evaluation.
- Tube curves: reuse the triode square root instead of powf(1.5); reuse one exponential for pentode grid softplus and sigmoid.

## Measurements

Local release test, stereo, 48 kHz, 512-frame blocks, input trim +6 dB, existing cpu_budget signal. Single before/after observations, not statistical estimates or DAW measurements.

| Factor | Before (ns/frame) | After (ns/frame) | Reduction |
| --- | ---: | ---: | ---: |
| 1x | 5364 | 4683 | 12.7% |
| 4x | 23657 | 21320 | 9.9% |

Reproduce: `cargo test -p b18n-plugin --release --test cpu_budget -- --ignored --nocapture --test-threads=1`.

## Validation

- Release workspace suite passed, including no-allocation/reset, plugin-engine parity, block invariance, latency, alias suppression, NFB stability and SPICE/NumPy reference checks.
- Added regression coverage for cached matrix pivoting, RHS changes, invalidation and restoration; tone cache versus rebuilding during automation/reset; prepared pentode curves versus the original equations.
- Temporary before/after full-engine capture: 65,536 output samples, 1x/4x, Bass/Instrument, driven sine followed by silence, tone automation. Maximum absolute f32 difference: 2.3841858e-7; reference peak: 31.21865. No convergence failures in these cases. Temporary capture harness removed after validation.
- DAW runtime and listening A/B remain unverified.

## Round 2: controls, inactive preamp and scalar fallbacks

- Cache input/output dB mappings and volume taper only while the actual smoothed f32 value is unchanged. Smoothing still advances on every sample, including inactive channels; its trajectory is unchanged.
- Suspend only the unselected preamp, after a zero-input step leaves all future-relevant dynamic state exactly unchanged and neither triode reports a new failure. Preserve its stationary output, the mixer loading, and capacitor histories. Selection or a change in effective smoothed controls wakes it. No silence threshold or arbitrary timeout; a decaying tail may remain running for a long time.
- Use value-only valve evaluations for scalar residual checks and bisections; skip screen-current/Jacobian work in the 7199 pentode fallback. Stop the power-grid bisection once its midpoint rounds to a bracket endpoint. Newton iteration limits, tolerances, bracket bounds and fallback availability are unchanged.

Same local release benchmark and signals as round 1; new before/after observations:

| Factor | Before round 2 (ns/frame) | After round 2 (ns/frame) | Reduction this round |
| --- | ---: | ---: | ---: |
| 1x | 4704 | 3953 | 16.0% |
| 4x | 20736 | 18083 | 12.8% |

These measurements describe this benchmark with fixed controls and one initially idle preamp; they do not quantify separate contributions or guarantee the same improvement during continuous automation/channel switching. Combined reduction relative to the original round-1 observations is approximately 26% at 1x and 24% at 4x, subject to run-to-run variation.

Validation: release workspace suite passed, including four additional regression tests for cached control trajectories, inactive-preamp versus always-running reference through tails/control changes/reactivation, and value-only triode/pentode evaluations. Existing reset/no-allocation, latency, NFB stability, alias and SPICE/NumPy checks also passed.

Temporary round-2 full-engine capture: 196,608 f32 output samples at 44.1/48/96 kHz, 1x/4x, channel switching, bass/volume/trim automation, driven sine and silence. Maximum absolute output difference against the pre-round-2 engine: 0. Reference peak: 30.143433. Zero convergence failures in these scenarios. Temporary comparison harness removed after validation.

Updated release VST3 generated via `cargo xtask bundle b18n-plugin --release`. DAW runtime and listening A/B remain unverified.

## Subsequent iteration: both preamps active, 2x and estimated speaker profiles

Inactive-preamp suspension is removed. Prior round-2 measurements are historical and do not represent this build. Both preamps now always run; control, tone, power-curve and fallback optimizations remain. Added 2x and acoustic profiles documented in MODEL_ASSUMPTIONS.md.

Release workspace tests passed, including 2x impulse/DC/reset/restore, latency reporting, speaker response distinctions and smoothed switching, exact adapter/engine parity with all three speaker modes and oversampling factors, block invariance, reset and no-allocation during speaker/factor switches. Additional 2x drive stability coverage passed across all five preferred sample rates. DAW/listening validation remains pending.

Current local release benchmark, stereo at 48 kHz with fixed controls and Direct speaker mode: 1x 4545 ns/frame; 2x 11117 ns/frame; 4x 20755 ns/frame. Single observations, not DAW measurements. These figures include both continuously running preamps and the warm speaker-filter histories.
