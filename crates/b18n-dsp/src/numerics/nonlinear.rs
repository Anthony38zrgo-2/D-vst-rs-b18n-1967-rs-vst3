//! Newton-Raphson amortiguado para sistemas no lineales pequeños.
//!
//! Características exigidas por el proyecto (guía §6.2):
//! - límite explícito de iteraciones por muestra;
//! - cota del incremento (sin saltos de voltaje descontrolados);
//! - retroceso de línea si el residuo no mejora;
//! - sin `panic` ni bucles infinitos; el llamador conserva el último estado
//!   válido si no hay convergencia.
//!
//! El jacobiano se calcula por diferencias finitas hacia adelante: los
//! sistemas del Hito C tienen 3–4 incógnitas y el coste es aceptable.
//!
//! **Sin asignaciones:** el tamaño del sistema es const-genérico (`N`) y toda
//! la memoria de trabajo vive en la pila; el callback de audio del plugin
//! verifica la ausencia de `alloc`/`realloc` en `process()`.

/// Configuración del solver.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NewtonConfig {
    pub max_iterations: usize,
    /// Convergencia por incremento máximo de las incógnitas.
    pub tolerance: f64,
    /// Residuo (misma unidad que `r(x)`) por debajo del cual se considera
    /// cero numérico; evita el retroceso de línea sobre ruido de máquina.
    pub residual_tolerance: f64,
    /// Cota del incremento por iteración (misma unidad que las incógnitas).
    pub max_step: f64,
    pub max_backtracks: usize,
}

impl Default for NewtonConfig {
    fn default() -> Self {
        Self {
            max_iterations: 40,
            tolerance: 1e-6,
            residual_tolerance: 1e-12,
            max_step: 20.0,
            max_backtracks: 10,
        }
    }
}

/// Causa de un fallo de convergencia (diagnóstico).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewtonFailure {
    NonFiniteStart,
    NonFiniteResidual,
    SingularJacobian,
    LineSearch,
    MaxIterations,
}

/// Resultado de una resolución.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NewtonReport {
    pub iterations: usize,
    pub converged: bool,
    pub residual_norm: f64,
    pub failure: Option<NewtonFailure>,
}

/// Resuelve `r(x) = 0` con jacobiano numérico (diferencias hacia adelante).
///
/// `N` es el número de incógnitas; no hay asignaciones dinámicas.
pub fn solve<const N: usize, F>(
    x: &mut [f64; N],
    mut residual: F,
    config: &NewtonConfig,
) -> NewtonReport
where
    F: FnMut(&[f64], &mut [f64]),
{
    let mut r_trial = [0.0; N];
    solve_impl::<N, _>(
        x,
        |state, out_r, out_j| {
            residual(state, out_r);
            for column in 0..N {
                let step = 1e-6 * state[column].abs().max(1.0);
                let saved = state[column];
                let mut probe = [0.0; N];
                probe.copy_from_slice(state);
                probe[column] = saved + step;
                residual(&probe, &mut r_trial);
                for row in 0..N {
                    out_j[row][column] = (r_trial[row] - out_r[row]) / step;
                }
            }
        },
        config,
    )
}

/// Resuelve `r(x) = 0` con jacobiano analítico.
///
/// `evaluate` debe llenar residuo y jacobiano (fila mayor) para el estado dado.
pub fn solve_with_jacobian<const N: usize, F>(
    x: &mut [f64; N],
    evaluate: F,
    config: &NewtonConfig,
) -> NewtonReport
where
    F: FnMut(&[f64], &mut [f64], &mut [[f64; N]; N]),
{
    solve_impl::<N, F>(x, evaluate, config)
}

fn solve_impl<const N: usize, F>(
    x: &mut [f64; N],
    mut evaluate: F,
    config: &NewtonConfig,
) -> NewtonReport
where
    F: FnMut(&[f64], &mut [f64], &mut [[f64; N]; N]),
{
    let mut r = [0.0; N];
    let mut jacobian = [[0.0; N]; N];
    let mut delta = [0.0; N];
    let mut x_trial = [0.0; N];
    let mut rhs = [0.0; N];

    if x.iter().any(|value| !value.is_finite()) {
        return NewtonReport {
            iterations: 0,
            converged: false,
            residual_norm: f64::INFINITY,
            failure: Some(NewtonFailure::NonFiniteStart),
        };
    }

    // La evaluación es **una por iteración**: la del intento aceptado en el
    // retroceso de línea sirve, sin recalcular, para la iteración siguiente
    // (mismo estado evaluado). `r`/`jacobian` viven siempre en x actual.
    evaluate(x, &mut r, &mut jacobian);
    let mut norm = max_abs(&r);
    for iteration in 0..config.max_iterations {
        if !norm.is_finite() {
            return NewtonReport {
                iterations: iteration,
                converged: false,
                residual_norm: f64::INFINITY,
                failure: Some(NewtonFailure::NonFiniteResidual),
            };
        }
        if norm < config.residual_tolerance {
            return NewtonReport {
                iterations: iteration,
                converged: true,
                residual_norm: norm,
                failure: None,
            };
        }

        if !solve_linear::<N>(&mut jacobian, &r, &mut delta, &mut rhs)
            || !delta.iter().all(|value| value.is_finite())
        {
            return NewtonReport {
                iterations: iteration,
                converged: false,
                residual_norm: norm,
                failure: Some(NewtonFailure::SingularJacobian),
            };
        }

        // Cota del paso y retroceso de línea.
        let step_norm = max_abs(&delta);
        let scale = if step_norm > config.max_step {
            config.max_step / step_norm
        } else {
            1.0
        };
        for step in &mut delta {
            *step *= scale;
        }

        let mut accepted = false;
        let mut factor = 1.0;
        for _ in 0..=config.max_backtracks {
            // `delta` resuelve J·δ = −r: el paso es x + δ. La evaluación del
            // intento aceptado queda como evaluación del estado x siguiente,
            // así que se escribe directamente en `r`/`jacobian`.
            for ((trial, state), step) in x_trial.iter_mut().zip(x.iter()).zip(delta.iter()) {
                *trial = state + factor * step;
            }
            evaluate(&x_trial, &mut r, &mut jacobian);
            let trial_norm = max_abs(&r);
            if trial_norm.is_finite() && trial_norm <= norm {
                x.copy_from_slice(&x_trial);
                norm = trial_norm;
                delta.iter_mut().for_each(|value| *value *= factor);
                accepted = true;
                break;
            }
            factor *= 0.5;
        }
        if !accepted {
            return NewtonReport {
                iterations: iteration,
                converged: false,
                residual_norm: norm,
                failure: Some(NewtonFailure::LineSearch),
            };
        }

        if max_abs(&delta) < config.tolerance {
            return NewtonReport {
                iterations: iteration + 1,
                converged: true,
                residual_norm: norm,
                failure: None,
            };
        }
    }

    NewtonReport {
        iterations: config.max_iterations,
        converged: false,
        residual_norm: max_abs(&r),
        failure: Some(NewtonFailure::MaxIterations),
    }
}

fn max_abs(values: &[f64]) -> f64 {
    values
        .iter()
        .fold(0.0_f64, |acc, value| acc.max(value.abs()))
}

/// Eliminación gaussiana con pivoteo parcial; resuelve `J·delta = -r`.
///
/// `rhs` es memoria de trabajo preasignada por el llamador (sin heap).
fn solve_linear<const N: usize>(
    jacobian: &mut [[f64; N]; N],
    residual: &[f64],
    out: &mut [f64],
    rhs: &mut [f64],
) -> bool {
    for (index, value) in residual.iter().enumerate() {
        rhs[index] = -value;
    }

    for column in 0..N {
        let mut pivot = column;
        let mut best = jacobian[column][column].abs();
        for (row, values) in jacobian.iter().enumerate().skip(column + 1) {
            let candidate = values[column].abs();
            if candidate > best {
                best = candidate;
                pivot = row;
            }
        }
        if best < 1e-18 {
            return false;
        }
        if pivot != column {
            jacobian.swap(column, pivot);
            rhs.swap(column, pivot);
        }
        let diagonal = jacobian[column][column];
        let (head, tail) = jacobian.split_at_mut(column + 1);
        let pivot_row = &head[column];
        for (offset, values) in tail.iter_mut().enumerate() {
            let row = column + 1 + offset;
            let factor = values[column] / diagonal;
            if factor != 0.0 {
                for (k, value) in values.iter_mut().enumerate().skip(column) {
                    *value -= factor * pivot_row[k];
                }
                rhs[row] -= factor * rhs[column];
            }
        }
    }

    for column in (0..N).rev() {
        let row = &jacobian[column];
        let mut sum = rhs[column];
        for (k, value) in row.iter().enumerate().skip(column + 1) {
            sum -= value * out[k];
        }
        out[column] = sum / row[column];
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solves_square_root() {
        // r(x) = x² − 2 = 0; solución sqrt(2).
        let mut x = [1.0];
        let report = solve(
            &mut x,
            |state, out| out[0] = state[0] * state[0] - 2.0,
            &NewtonConfig::default(),
        );
        assert!(report.converged, "no convergió: {report:?}");
        assert!((x[0] - 2.0_f64.sqrt()).abs() < 1e-9);
    }

    #[test]
    fn solves_coupled_system() {
        // x² + y = 1; x + y² = 1; solución (0,1) o (1,0).
        let mut x = [0.2, 0.7];
        let report = solve(
            &mut x,
            |state, out| {
                out[0] = state[0] * state[0] + state[1] - 1.0;
                out[1] = state[0] + state[1] * state[1] - 1.0;
            },
            &NewtonConfig::default(),
        );
        assert!(report.converged, "no convergió: {report:?}");
        assert!((x[0]).abs() < 1e-7 && (x[1] - 1.0).abs() < 1e-7, "{x:?}");
    }

    #[test]
    fn reports_failure_without_nan() {
        // r(x) = x² + 1 nunca tiene raíz real.
        let mut x = [0.5];
        let report = solve(
            &mut x,
            |state, out| out[0] = state[0] * state[0] + 1.0,
            &NewtonConfig::default(),
        );
        assert!(!report.converged);
        assert!(x[0].is_finite());
    }

    #[test]
    fn rejects_non_finite_start() {
        let mut x = [f64::NAN];
        let report = solve(
            &mut x,
            |state, out| out[0] = state[0],
            &NewtonConfig::default(),
        );
        assert!(!report.converged);
    }

    #[test]
    fn analytic_jacobian_matches_numeric_solution() {
        // r(x, y) = [x² + y − 1, x + y² − 1] con jacobiano analítico.
        let mut x = [0.2, 0.7];
        let report = solve_with_jacobian(
            &mut x,
            |state, out, jacobian| {
                out[0] = state[0] * state[0] + state[1] - 1.0;
                out[1] = state[0] + state[1] * state[1] - 1.0;
                jacobian[0] = [2.0 * state[0], 1.0];
                jacobian[1] = [1.0, 2.0 * state[1]];
            },
            &NewtonConfig::default(),
        );
        assert!(report.converged, "no convergió: {report:?}");
        assert!((x[0]).abs() < 1e-7 && (x[1] - 1.0).abs() < 1e-7, "{x:?}");
    }
}
