//! Análisis nodal modificado (MNA) para redes lineales.
//!
//! Dos variantes sin dependencias externas:
//! - [`RealMna`]: dominio temporal, con estampado de conductancias y fuentes
//!   de corriente (los capacitores se acoplan con [`crate::numerics::discretization::TptCapacitor`]).
//! - [`ComplexMna`]: dominio frecuencial (`jω`), para funciones de transferencia
//!   y comparación contra referencias independientes.
//!
//! El nodo `0` es siempre masa y no forma parte de la matriz.

use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};

/// Número complejo mínimo suficiente para el solver frecuencial.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Complex {
    pub re: f64,
    pub im: f64,
}

impl Complex {
    pub const ZERO: Self = Self { re: 0.0, im: 0.0 };
    pub const ONE: Self = Self { re: 1.0, im: 0.0 };

    #[must_use]
    pub const fn new(re: f64, im: f64) -> Self {
        Self { re, im }
    }

    /// Admitancia de un capacitor: `jωC`.
    #[must_use]
    pub fn capacitor_admittance(farads: f64, omega: f64) -> Self {
        Self::new(0.0, omega * farads)
    }

    #[must_use]
    pub fn magnitude(self) -> f64 {
        self.re.hypot(self.im)
    }

    #[must_use]
    pub fn magnitude_db(self) -> f64 {
        20.0 * self.magnitude().max(f64::MIN_POSITIVE).log10()
    }

    #[must_use]
    pub fn phase_deg(self) -> f64 {
        self.im.atan2(self.re).to_degrees()
    }
}

impl Add for Complex {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self::new(self.re + rhs.re, self.im + rhs.im)
    }
}

impl Sub for Complex {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self::new(self.re - rhs.re, self.im - rhs.im)
    }
}

impl Mul for Complex {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        Self::new(
            self.re * rhs.re - self.im * rhs.im,
            self.re * rhs.im + self.im * rhs.re,
        )
    }
}

impl Mul<f64> for Complex {
    type Output = Self;
    fn mul(self, rhs: f64) -> Self {
        Self::new(self.re * rhs, self.im * rhs)
    }
}

impl Div for Complex {
    type Output = Self;
    fn div(self, rhs: Self) -> Self {
        let denom = rhs.re * rhs.re + rhs.im * rhs.im;
        Self::new(
            (self.re * rhs.re + self.im * rhs.im) / denom,
            (self.im * rhs.re - self.re * rhs.im) / denom,
        )
    }
}

impl AddAssign for Complex {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl SubAssign for Complex {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl Neg for Complex {
    type Output = Self;
    fn neg(self) -> Self {
        Self::new(-self.re, -self.im)
    }
}

/// Sistema MNA real de tamaño fijo, reutilizable sin asignaciones al resolver.
#[derive(Debug, Clone)]
pub struct RealMna {
    size: usize,
    matrix: Vec<f64>,
    rhs: Vec<f64>,
    scratch_matrix: Vec<f64>,
    scratch_rhs: Vec<f64>,
    pivots: Vec<usize>,
    factorized: bool,
}

impl RealMna {
    /// `size` es el número de nodos excluyendo masa.
    #[must_use]
    pub fn new(size: usize) -> Self {
        Self {
            size,
            matrix: vec![0.0; size * size],
            rhs: vec![0.0; size],
            scratch_matrix: vec![0.0; size * size],
            scratch_rhs: vec![0.0; size],
            pivots: vec![0; size],
            factorized: false,
        }
    }

    #[must_use]
    pub const fn size(&self) -> usize {
        self.size
    }

    /// Prepara el sistema para un nuevo estampado.
    pub fn clear(&mut self) {
        self.factorized = false;
        self.matrix.fill(0.0);
        self.rhs.fill(0.0);
    }

    /// Conserva la matriz y su factorización para una nueva entrada.
    pub fn clear_rhs(&mut self) {
        self.rhs.fill(0.0);
    }

    /// Restaura el estado desde otro sistema **sin asignar memoria** (los
    /// buffers existentes se reutilizan; `Vec::clone_from` conserva la
    /// capacidad). Debe llamarse con el mismo `size`.
    pub fn restore_from(&mut self, other: &Self) {
        debug_assert_eq!(self.size, other.size, "tamaños MNA distintos");
        self.matrix.clone_from(&other.matrix);
        self.rhs.clone_from(&other.rhs);
        self.scratch_matrix.clone_from(&other.scratch_matrix);
        self.scratch_rhs.clone_from(&other.scratch_rhs);
        self.pivots.clone_from(&other.pivots);
        self.factorized = other.factorized;
    }

    /// Estampa una conductancia entre dos nodos (`0` = masa).
    pub fn add_conductance(&mut self, node_a: usize, node_b: usize, g: f64) {
        self.factorized = false;
        let index = |node: usize| node - 1;
        if node_a != 0 {
            self.matrix[index(node_a) * self.size + index(node_a)] += g;
        }
        if node_b != 0 {
            self.matrix[index(node_b) * self.size + index(node_b)] += g;
        }
        if node_a != 0 && node_b != 0 {
            self.matrix[index(node_a) * self.size + index(node_b)] -= g;
            self.matrix[index(node_b) * self.size + index(node_a)] -= g;
        }
    }

    /// Inyecta una corriente en un nodo (`0` = masa se ignora).
    pub fn add_current_injection(&mut self, node: usize, current: f64) {
        if node != 0 {
            self.rhs[node - 1] += current;
        }
    }

    /// Resuelve el sistema. Escribe la solución en `out` (`out[0] = 0` es masa)
    /// y devuelve `false` si la matriz es singular.
    pub fn solve_into(&mut self, out: &mut [f64]) -> bool {
        let n = self.size;
        if out.len() < n + 1 {
            return false;
        }
        if !self.factorized {
            self.scratch_matrix.copy_from_slice(&self.matrix);

            for col in 0..n {
                let mut pivot = col;
                let mut best = self.scratch_matrix[col * n + col].abs();
                for row in (col + 1)..n {
                    let candidate = self.scratch_matrix[row * n + col].abs();
                    if candidate > best {
                        best = candidate;
                        pivot = row;
                    }
                }
                if best < 1e-18 {
                    return false;
                }
                if pivot != col {
                    for k in col..n {
                        self.scratch_matrix.swap(col * n + k, pivot * n + k);
                    }
                }
                self.pivots[col] = pivot;
                let diag = self.scratch_matrix[col * n + col];
                for row in (col + 1)..n {
                    let factor = self.scratch_matrix[row * n + col] / diag;
                    if factor != 0.0 {
                        for k in col..n {
                            self.scratch_matrix[row * n + k] -=
                                factor * self.scratch_matrix[col * n + k];
                        }
                    }
                    // Store elimination operations in execution order, below U.
                    self.scratch_matrix[row * n + col] = factor;
                }
            }
            self.factorized = true;
        }
        self.scratch_rhs.copy_from_slice(&self.rhs);
        for col in 0..n {
            self.scratch_rhs.swap(col, self.pivots[col]);
            for row in (col + 1)..n {
                let factor = self.scratch_matrix[row * n + col];
                if factor != 0.0 {
                    self.scratch_rhs[row] -= factor * self.scratch_rhs[col];
                }
            }
        }

        out[0] = 0.0;
        for col in (0..n).rev() {
            let mut sum = self.scratch_rhs[col];
            for k in (col + 1)..n {
                sum -= self.scratch_matrix[col * n + k] * out[k + 1];
            }
            out[col + 1] = sum / self.scratch_matrix[col * n + col];
        }
        true
    }
}

/// Sistema MNA complejo para análisis frecuencial.
#[derive(Debug, Clone)]
pub struct ComplexMna {
    size: usize,
    matrix: Vec<Complex>,
    rhs: Vec<Complex>,
}

impl ComplexMna {
    #[must_use]
    pub fn new(size: usize) -> Self {
        Self {
            size,
            matrix: vec![Complex::ZERO; size * size],
            rhs: vec![Complex::ZERO; size],
        }
    }

    /// Estampa una admitancia entre dos nodos (`0` = masa).
    pub fn add_admittance(&mut self, node_a: usize, node_b: usize, y: Complex) {
        let index = |node: usize| node - 1;
        if node_a != 0 {
            self.matrix[index(node_a) * self.size + index(node_a)] += y;
        }
        if node_b != 0 {
            self.matrix[index(node_b) * self.size + index(node_b)] += y;
        }
        if node_a != 0 && node_b != 0 {
            self.matrix[index(node_a) * self.size + index(node_b)] -= y;
            self.matrix[index(node_b) * self.size + index(node_a)] -= y;
        }
    }

    /// Admitancia de una resistencia.
    pub fn add_resistor(&mut self, node_a: usize, node_b: usize, ohms: f64) {
        self.add_admittance(node_a, node_b, Complex::new(1.0 / ohms, 0.0));
    }

    /// Admitancia de un capacitor a la frecuencia `omega`.
    pub fn add_capacitor(&mut self, node_a: usize, node_b: usize, farads: f64, omega: f64) {
        self.add_admittance(node_a, node_b, Complex::capacitor_admittance(farads, omega));
    }

    /// Inyecta una corriente fasorial en un nodo.
    pub fn add_current_injection(&mut self, node: usize, current: Complex) {
        if node != 0 {
            self.rhs[node - 1] += current;
        }
    }

    /// Resuelve con eliminación gaussiana y pivoteo parcial.
    #[must_use]
    pub fn solve(&self) -> Option<Vec<Complex>> {
        let n = self.size;
        let mut a = self.matrix.clone();
        let mut z = self.rhs.clone();

        for col in 0..n {
            let mut pivot = col;
            let mut best = a[col * n + col].magnitude();
            for row in (col + 1)..n {
                let candidate = a[row * n + col].magnitude();
                if candidate > best {
                    best = candidate;
                    pivot = row;
                }
            }
            if best < 1e-30 {
                return None;
            }
            if pivot != col {
                for k in col..n {
                    a.swap(col * n + k, pivot * n + k);
                }
                z.swap(col, pivot);
            }
            let diag = a[col * n + col];
            for row in (col + 1)..n {
                let factor = a[row * n + col] / diag;
                if factor != Complex::ZERO {
                    for k in col..n {
                        let value = a[col * n + k];
                        a[row * n + k] -= factor * value;
                    }
                    let value = z[col];
                    z[row] -= factor * value;
                }
            }
        }

        let mut out = vec![Complex::ZERO; n + 1];
        for col in (0..n).rev() {
            let mut sum = z[col];
            for k in (col + 1)..n {
                sum -= a[col * n + k] * out[k + 1];
            }
            out[col + 1] = sum / a[col * n + col];
        }
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cached_elimination_handles_pivots_rhs_changes_and_invalidation() {
        let mut mna = RealMna::new(3);
        // Multiple pivot swaps; compare against a known solution, not another solver.
        let matrix = [0.0, 2.0, 1.0, 1.0, 0.0, 3.0, 4.0, 1.0, 0.0];
        mna.matrix.copy_from_slice(&matrix);
        for expected in [[1.0, -2.0, 3.0], [-4.0, 0.25, 2.0]] {
            mna.clear_rhs();
            for row in 0..3 {
                let rhs = (0..3)
                    .map(|col| matrix[row * 3 + col] * expected[col])
                    .sum();
                mna.add_current_injection(row + 1, rhs);
            }
            let mut out = [0.0; 4];
            assert!(mna.solve_into(&mut out));
            assert!(mna.factorized);
            for i in 0..3 {
                assert!((out[i + 1] - expected[i]).abs() < 1e-12);
            }
        }
        mna.add_conductance(1, 0, 2.0);
        assert!(!mna.factorized);
        mna.clear_rhs();
        for (i, rhs) in [9.0, 10.0, 6.0].into_iter().enumerate() {
            mna.add_current_injection(i + 1, rhs);
        }
        let mut out = [0.0; 4];
        assert!(mna.solve_into(&mut out));
        for (got, expected) in out[1..].iter().zip([1.0, 2.0, 3.0]) {
            assert!((got - expected).abs() < 1e-12);
        }
        let mut restored = RealMna::new(3);
        restored.restore_from(&mna);
        assert!(restored.solve_into(&mut out));
        mna.clear();
        assert!(!mna.solve_into(&mut out));
    }

    #[test]
    fn resistor_divider_matches_analytic() {
        // Inyección de 10 mA en el nodo 1; 1k de nodo 1 a masa, 1k de nodo
        // 1 a nodo 2 y 1k de nodo 2 a masa.
        // R_eq vista desde la fuente = 1k || (1k + 1k) = 666.67 Ω.
        let mut mna = RealMna::new(2);
        mna.add_conductance(1, 0, 1.0 / 1000.0);
        mna.add_conductance(1, 2, 1.0 / 1000.0);
        mna.add_conductance(2, 0, 1.0 / 1000.0);
        mna.add_current_injection(1, 0.01);
        let mut out = [0.0; 3];
        assert!(mna.solve_into(&mut out));
        assert!((out[1] - 20.0 / 3.0).abs() < 1e-9, "V1 = {}", out[1]);
        assert!((out[2] - 10.0 / 3.0).abs() < 1e-9, "V2 = {}", out[2]);
    }

    #[test]
    fn complex_rc_lowpass_matches_analytic() {
        // Fuente de 1 A en nodo 1, R = 1k a masa, C = 1 µF a masa.
        // Z = R || 1/(jωC) => V = Z.
        let r = 1000.0_f64;
        let c = 1e-6_f64;
        let omega = 2.0 * std::f64::consts::PI * 100.0;
        let mut mna = ComplexMna::new(1);
        mna.add_resistor(1, 0, r);
        mna.add_capacitor(1, 0, c, omega);
        mna.add_current_injection(1, Complex::ONE);
        let solution = mna.solve().expect("sistema resoluble");
        let analytic = Complex::ONE / Complex::new(1.0 / r, omega * c);
        assert!((solution[1] - analytic).magnitude() < 1e-12);
    }

    #[test]
    fn complex_complex_ops_roundtrip() {
        let a = Complex::new(3.0, 4.0);
        let b = Complex::new(-1.0, 2.0);
        let quotient = (a * b) / b;
        assert!((quotient - a).magnitude() < 1e-12);
        assert!((a.magnitude() - 5.0).abs() < 1e-12);
    }
}
