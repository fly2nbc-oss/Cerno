//! Personal taste model: learns the user's star ratings from CLIP embeddings.
//!
//! Ridge regression on the L2-normalised embeddings; rejected photos count as 0 stars. The
//! regularisation strength is picked by 5-fold cross-validation. Training runs in the background
//! and again after ratings change; with fewer examples than dimensions it is solved in the dual
//! form ([`Ridge`]).

/// Below this many examples (or with only one distinct label) there is no model.
pub const MIN_EXAMPLES: usize = 15;
/// Regularisation candidates, relative to `n / dimensions` (the average eigenvalue scale).
const LAMBDAS: [f64; 4] = [0.03, 0.3, 3.0, 30.0];
const FOLDS: usize = 5;

#[derive(Debug, Clone)]
pub struct TasteModel {
    mean_x: Vec<f64>,
    mean_y: f64,
    weights: Vec<f64>,
    /// Photos it was trained on.
    pub examples: usize,
    /// Cross-validated mean absolute error in stars (how far off it typically is).
    pub error: f32,
}

impl TasteModel {
    /// `None` until there are enough, and varied enough, examples – or when `stop` says so
    /// between two of its solves (a primal one takes ~0.1 s at 768 dimensions, and Cerno waits
    /// for this thread when it ends).
    pub fn train(examples: &[(Vec<f32>, f32)], stop: &dyn Fn() -> bool) -> Option<Self> {
        Self::train_in(examples, stop, None)
    }

    /// `dual`: the form to solve in; `None` takes the smaller one (the tests compare both).
    fn train_in(
        examples: &[(Vec<f32>, f32)],
        stop: &dyn Fn() -> bool,
        dual: Option<bool>,
    ) -> Option<Self> {
        let dim = examples.first()?.0.len();
        let data: Vec<(Vec<f64>, f64)> = examples
            .iter()
            .filter(|(x, _)| x.len() == dim)
            .filter_map(|(x, y)| Some((normalised(x)?, f64::from(*y))))
            .collect();
        let n = data.len();
        let first = data.first()?.1;
        if n < MIN_EXAMPLES || data.iter().all(|(_, y)| (*y - first).abs() < 1e-9) {
            return None;
        }

        // Centre globally; each fold learns from the rows of the other folds.
        let mean_x = mean(data.iter().map(|(x, _)| x.as_slice()), dim);
        let mean_y = data.iter().map(|(_, y)| y).sum::<f64>() / n as f64;
        let centred: Vec<(Vec<f64>, f64)> = data
            .iter()
            .map(|(x, y)| {
                (
                    x.iter().zip(&mean_x).map(|(a, m)| a - m).collect(),
                    y - mean_y,
                )
            })
            .collect();
        let ridge = Ridge::new(&centred, dim, dual.unwrap_or(n < dim));
        let scale = n as f64 / dim as f64;

        let mut best: Option<(f64, f64)> = None; // (error, lambda)
        if n >= 2 * FOLDS {
            for relative in LAMBDAS {
                let lambda = relative * scale;
                let mut abs_error = 0.0;
                for fold in 0..FOLDS {
                    if stop() {
                        return None;
                    }
                    let Some(w) = ridge.weights(Some(fold), lambda) else {
                        continue;
                    };
                    for (i, (x, y)) in centred.iter().enumerate() {
                        if fold_of(i) == fold {
                            abs_error += (dot(&w, x) - y).abs();
                        }
                    }
                }
                let error = abs_error / n as f64;
                if best.is_none_or(|(e, _)| error < e) {
                    best = Some((error, lambda));
                }
            }
        }
        let (error, lambda) = best.unwrap_or((f64::NAN, LAMBDAS[1] * scale));
        if stop() {
            return None;
        }
        let weights = ridge.weights(None, lambda)?;
        Some(Self {
            mean_x,
            mean_y,
            weights,
            examples: n,
            error: error as f32,
        })
    }

    /// Predicted stars, 0..=5.
    pub fn predict(&self, embedding: &[f32]) -> Option<f32> {
        let x = normalised(embedding).filter(|x| x.len() == self.mean_x.len())?;
        let centred: Vec<f64> = x.iter().zip(&self.mean_x).map(|(a, m)| a - m).collect();
        Some((self.mean_y + dot(&self.weights, &centred)).clamp(0.0, 5.0) as f32)
    }
}

/// The cross-validation fold a row belongs to.
fn fold_of(row: usize) -> usize {
    row % FOLDS
}

/// Ridge regression on centred rows, solved in the smaller of two forms that give the same
/// weights (the push-through identity), up to rounding: the primal (XᵀX + λI) w = Xᵀy is
/// dim × dim, the dual (XXᵀ + λI) α = y, w = Xᵀα, is n × n. Users rate far fewer photos than
/// CLIP has dimensions (768): 223 examples took 2.7 s in the primal form, 21 solves of 768 × 768.
enum Ridge<'a> {
    /// One Gram matrix (upper triangle) and right-hand side per fold, and their sums.
    Primal {
        dim: usize,
        gram: Vec<Vec<f64>>,
        rhs: Vec<Vec<f64>>,
        total_gram: Vec<f64>,
        total_rhs: Vec<f64>,
    },
    /// The rows and their dot products (n × n, both triangles).
    Dual {
        rows: &'a [(Vec<f64>, f64)],
        kernel: Vec<f64>,
    },
}

impl<'a> Ridge<'a> {
    fn new(rows: &'a [(Vec<f64>, f64)], dim: usize, dual: bool) -> Self {
        if dual {
            let n = rows.len();
            let mut kernel = vec![0.0; n * n];
            for i in 0..n {
                for j in i..n {
                    let value = dot(&rows[i].0, &rows[j].0);
                    kernel[i * n + j] = value;
                    kernel[j * n + i] = value;
                }
            }
            return Self::Dual { rows, kernel };
        }
        let mut gram = vec![vec![0.0; dim * dim]; FOLDS];
        let mut rhs = vec![vec![0.0; dim]; FOLDS];
        for (i, (x, y)) in rows.iter().enumerate() {
            accumulate(&mut gram[fold_of(i)], &mut rhs[fold_of(i)], x, *y);
        }
        let total_gram = sum_all(&gram);
        let total_rhs = sum_all(&rhs);
        Self::Primal {
            dim,
            gram,
            rhs,
            total_gram,
            total_rhs,
        }
    }

    /// The weights learnt from the rows outside `fold` – from all rows for `None`.
    fn weights(&self, fold: Option<usize>, lambda: f64) -> Option<Vec<f64>> {
        match self {
            Self::Primal {
                dim,
                gram,
                rhs,
                total_gram,
                total_rhs,
            } => match fold {
                Some(fold) => solve_ridge(
                    subtract(total_gram, &gram[fold]),
                    &subtract(total_rhs, &rhs[fold]),
                    lambda,
                    *dim,
                ),
                None => solve_ridge(total_gram.clone(), total_rhs, lambda, *dim),
            },
            Self::Dual { rows, kernel } => {
                let n = rows.len();
                let kept: Vec<usize> = (0..n).filter(|&i| Some(fold_of(i)) != fold).collect();
                let m = kept.len();
                // The kept rows' block, upper triangle (all `solve_ridge` reads).
                let mut a = vec![0.0; m * m];
                for (r, &i) in kept.iter().enumerate() {
                    for (c, &j) in kept.iter().enumerate().skip(r) {
                        a[r * m + c] = kernel[i * n + j];
                    }
                }
                let y: Vec<f64> = kept.iter().map(|&i| rows[i].1).collect();
                let alpha = solve_ridge(a, &y, lambda, m)?;
                let mut weights = vec![0.0; rows.first()?.0.len()];
                for (&i, a) in kept.iter().zip(&alpha) {
                    for (w, x) in weights.iter_mut().zip(&rows[i].0) {
                        *w += a * x;
                    }
                }
                Some(weights)
            }
        }
    }
}

fn normalised(x: &[f32]) -> Option<Vec<f64>> {
    let norm = x.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>().sqrt();
    (norm > 1e-12).then(|| x.iter().map(|v| f64::from(*v) / norm).collect())
}

fn mean<'a>(rows: impl Iterator<Item = &'a [f64]>, dim: usize) -> Vec<f64> {
    let mut sum = vec![0.0; dim];
    let mut n = 0.0;
    for row in rows {
        for (s, v) in sum.iter_mut().zip(row) {
            *s += v;
        }
        n += 1.0;
    }
    sum.iter().map(|s| s / n).collect()
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// Adds x·xᵀ to the (upper triangle of the) Gram matrix and x·y to the right-hand side.
fn accumulate(gram: &mut [f64], rhs: &mut [f64], x: &[f64], y: f64) {
    let dim = x.len();
    for i in 0..dim {
        let xi = x[i];
        rhs[i] += xi * y;
        let row = &mut gram[i * dim..(i + 1) * dim];
        for j in i..dim {
            row[j] += xi * x[j];
        }
    }
}

fn sum_all(parts: &[Vec<f64>]) -> Vec<f64> {
    let mut total = vec![0.0; parts[0].len()];
    for part in parts {
        for (t, v) in total.iter_mut().zip(part) {
            *t += v;
        }
    }
    total
}

fn subtract(a: &[f64], b: &[f64]) -> Vec<f64> {
    a.iter().zip(b).map(|(x, y)| x - y).collect()
}

/// Solves (A + λI) w = b by Cholesky. `a` holds the upper triangle (row-major).
fn solve_ridge(mut a: Vec<f64>, b: &[f64], lambda: f64, dim: usize) -> Option<Vec<f64>> {
    for i in 0..dim {
        a[i * dim + i] += lambda;
    }
    // In-place Cholesky, lower factor L stored transposed in the upper triangle: a = Lᵀ.
    for j in 0..dim {
        let mut diagonal = a[j * dim + j];
        for k in 0..j {
            diagonal -= a[k * dim + j] * a[k * dim + j];
        }
        if diagonal <= 0.0 {
            return None;
        }
        let diagonal = diagonal.sqrt();
        a[j * dim + j] = diagonal;
        for i in j + 1..dim {
            let mut value = a[j * dim + i];
            for k in 0..j {
                value -= a[k * dim + j] * a[k * dim + i];
            }
            a[j * dim + i] = value / diagonal;
        }
    }
    // Forward (L z = b), then backward (Lᵀ w = z).
    let mut z = b.to_vec();
    for i in 0..dim {
        for k in 0..i {
            z[i] -= a[k * dim + i] * z[k];
        }
        z[i] /= a[i * dim + i];
    }
    for i in (0..dim).rev() {
        for k in i + 1..dim {
            z[i] -= a[i * dim + k] * z[k];
        }
        z[i] /= a[i * dim + i];
    }
    Some(z)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deterministic pseudo-random numbers in -1..1 (no rand dependency).
    fn noise(seed: &mut u64) -> f32 {
        *seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((*seed >> 33) as f32 / (1u64 << 31) as f32) * 2.0 - 1.0
    }

    fn unit(seed: &mut u64, dim: usize) -> Vec<f32> {
        let v: Vec<f32> = (0..dim).map(|_| noise(seed)).collect();
        let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        v.iter().map(|x| x / n).collect()
    }

    #[test]
    fn learns_a_linear_taste() {
        let dim = 8;
        let mut seed = 7;
        let truth = unit(&mut seed, dim);
        let stars = |x: &[f32]| 2.5 + 6.0 * x.iter().zip(&truth).map(|(a, b)| a * b).sum::<f32>();
        let examples: Vec<(Vec<f32>, f32)> = (0..200)
            .map(|_| {
                let x = unit(&mut seed, dim);
                let y = stars(&x) + 0.05 * noise(&mut seed);
                (x, y)
            })
            .collect();
        let model = TasteModel::train(&examples, &|| false).unwrap();
        assert_eq!(model.examples, 200);
        assert!(model.error < 0.3, "cross-validated error {}", model.error);
        for _ in 0..20 {
            let x = unit(&mut seed, dim);
            let expected = stars(&x).clamp(0.0, 5.0);
            let got = model.predict(&x).unwrap();
            assert!((got - expected).abs() < 0.4, "{got} vs {expected}");
        }
    }

    #[test]
    fn needs_enough_varied_examples() {
        let mut seed = 1;
        let few: Vec<_> = (0..MIN_EXAMPLES - 1)
            .map(|i| (unit(&mut seed, 8), (i % 5) as f32))
            .collect();
        assert!(TasteModel::train(&few, &|| false).is_none());
        let same: Vec<_> = (0..50).map(|_| (unit(&mut seed, 8), 3.0)).collect();
        assert!(TasteModel::train(&same, &|| false).is_none());
        assert!(TasteModel::train(&[], &|| false).is_none());
    }

    #[test]
    fn both_forms_learn_the_same_model() {
        let mut seed = 11;
        // Fewer examples than dimensions (the dual form's case), and more.
        for (n, dim) in [(40, 64), (200, 8)] {
            let truth = unit(&mut seed, dim);
            let examples: Vec<(Vec<f32>, f32)> = (0..n)
                .map(|_| {
                    let x = unit(&mut seed, dim);
                    let y = 2.5 + 4.0 * x.iter().zip(&truth).map(|(a, b)| a * b).sum::<f32>();
                    (x, y + 0.3 * noise(&mut seed))
                })
                .collect();
            let primal = TasteModel::train_in(&examples, &|| false, Some(false)).unwrap();
            let dual = TasteModel::train_in(&examples, &|| false, Some(true)).unwrap();
            assert!(
                (primal.error - dual.error).abs() < 1e-6,
                "n {n}: error {} vs {}",
                primal.error,
                dual.error
            );
            for (p, d) in primal.weights.iter().zip(&dual.weights) {
                assert!((p - d).abs() < 1e-9 * (1.0 + p.abs()), "n {n}: {p} vs {d}");
            }
        }
    }

    #[test]
    fn a_stop_ends_the_training() {
        let mut seed = 3;
        let examples: Vec<_> = (0..100)
            .map(|i| (unit(&mut seed, 8), (i % 5) as f32))
            .collect();
        let asked = std::cell::Cell::new(0);
        let stop = || {
            asked.set(asked.get() + 1);
            true
        };
        assert!(TasteModel::train(&examples, &stop).is_none());
        assert_eq!(asked.get(), 1, "it stops at the first solve");
        assert!(TasteModel::train(&examples, &|| false).is_some());
    }

    #[test]
    fn cholesky_solves_a_known_system() {
        // A = [[4, 2], [2, 3]] (upper triangle stored), b = [2, 1] → w = [0.5, 0].
        let w = solve_ridge(vec![4.0, 2.0, 0.0, 3.0], &[2.0, 1.0], 0.0, 2).unwrap();
        assert!((w[0] - 0.5).abs() < 1e-12 && w[1].abs() < 1e-12, "{w:?}");
    }
}
