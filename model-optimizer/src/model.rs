//! Direct calculation of the model, the reduced loss L*(b) and dL*/db.
//!
//! Model: H(s) = a / (b s^2 + s). For one input step of size d at time t_i,
//! with tau = t - t_i >= 0 and x = tau / b:
//!
//!   o(t)  = a * d * g,       g(tau, b) = tau + b*(exp(-tau/b) - 1) = b * phi(x)
//!   phi(x)  = x - 1 + e^-x
//!   dg/db   = dphi(x) = e^-x (1 + x) - 1
//!
//! Total: o(t) = a * S(t,b), S = sum_i d_i * g_i.
//! For fixed b the optimal gain is a*(b) = sum(S y) / sum(S^2), and since
//! dL/da = 0 there, dL*/db = dL/db = -2 a sum(S' (y - a S)).
//!
//! Units: t, tau, b in seconds; a in output-counts per (input-unit * second).
//!
//! # Performance notes
//!
//! * Sample times are swept in order, carrying a decayed running state
//!   (A, C, E, F, see [`Decayed`]) for "old" steps (x >= `FOLD_X`), so each
//!   step costs one `exp` in total instead of one per (sample, step) pair.
//!   "Recent" steps (x < `FOLD_X`) are still evaluated directly with
//!   [`phi_dphi`], which avoids the cancellation in the running-state formula.
//!   If sample times are not sorted, we fall back to the direct O(N*M) loop.
//! * `evaluate` reuses a scratch buffer, and does not print.
//! * With the `parallel` cargo feature, large datasets are split into fixed
//!   chunks processed by rayon. The chunking (and so the floating-point
//!   summation order) is the same with and without the feature.
//!
//! Required steps order: `steps` sorted by time (as before).

use std::sync::Mutex;

use crate::error::FitError;
use crate::input::{Dataset, Step};

#[cfg(feature = "parallel")]
use rayon::prelude::*;

/// Below this x the closed forms lose precision to cancellation, so a Taylor
/// series is used instead. The first omitted term (x^9/9!) is ~7e-15 relative
/// at this threshold.
const SERIES_THRESHOLD: f64 = 0.05;

/// At and above this x, `x + expm1(-x) * (1 + x)` loses absolute accuracy
/// (~eps * x), so dphi is computed from `exp` directly instead.
const EXPM1_LIMIT: f64 = 30.0;

/// Smallest accepted sum(S^2); below it a*(b) is considered undefined.
const MIN_SUM_SQUARES: f64 = 1e-30;

/// Steps older than this many time constants (x = tau / b) are folded into the
/// running decayed state; younger ones are evaluated directly.
const FOLD_X: f64 = 1.0;

/// Fixed chunk size for fills and reductions (deterministic summation order).
const CHUNK: usize = 4096;

/// Minimum number of samples before rayon is used.
#[cfg(feature = "parallel")]
const PAR_MIN_POINTS: usize = 16_384;

/// Returns `(phi(x), dphi(x))` for x >= 0, accurate for all x.
///
/// For small x, `x - 1 + e^-x` and `e^-x (1+x) - 1` subtract nearly equal
/// numbers, so we use their Taylor series (terms through x^8):
///   phi  = x^2/2 - x^3/6 + x^4/24 - ...      (coeff of x^k: (-1)^k / k!)
///   dphi = -x^2/2 + x^3/3 - x^4/8 + ...      (coeff of x^k: (-1)^k (1-k) / k!)
///
/// Otherwise a single transcendental call is used. With em1 = e^-x - 1:
///   phi  = x + em1
///   dphi = e^-x (1+x) - 1 = x + em1 (1+x)
pub fn phi_dphi(x: f64) -> (f64, f64) {
    if x < SERIES_THRESHOLD {
        let phi = x
            * x
            * (1.0 / 2.0
                + x * (-1.0 / 6.0
                    + x * (1.0 / 24.0
                        + x * (-1.0 / 120.0
                            + x * (1.0 / 720.0 + x * (-1.0 / 5040.0 + x / 40320.0))))));
        let dphi = x
            * x
            * (-1.0 / 2.0
                + x * (1.0 / 3.0
                    + x * (-1.0 / 8.0
                        + x * (1.0 / 30.0
                            + x * (-1.0 / 144.0 + x * (1.0 / 840.0 + x * (-1.0 / 5760.0)))))));
        (phi, dphi)
    } else if x < EXPM1_LIMIT {
        let em1 = (-x).exp_m1();
        (x + em1, x + em1 * (1.0 + x))
    } else {
        // exp(-x) never overflows for x >= 0 (it underflows harmlessly to 0).
        let e = (-x).exp();
        (x - 1.0 + e, e * (1.0 + x) - 1.0)
    }
}

/// Direct sum over the given steps (all assumed started at `t`):
/// `(sum d*b*phi, sum d*dphi)`.
#[inline]
fn direct_sum(steps: &[Step], t: f64, b: f64, inv_b: f64) -> (f64, f64) {
    let mut s = 0.0;
    let mut ds = 0.0;
    for step in steps {
        let x = (t - step.time) * inv_b;
        let (phi, dphi) = phi_dphi(x);
        s += step.delta * b * phi;
        ds += step.delta * dphi;
    }
    (s, ds)
}

/// `(S(t,b), dS/db(t,b))`: the response at time `t` of all steps already
/// started (steps are time-ordered, so we stop at the first future one).
pub fn response(steps: &[Step], t: f64, b: f64) -> (f64, f64) {
    let n = steps.partition_point(|st| st.time <= t);
    direct_sum(&steps[..n], t, b, 1.0 / b)
}

/// Running sums over "old" steps, with tau measured to the current time:
///   A = sum d,  C = sum d*tau,  E = sum d*e^(-tau/b),  F = sum d*tau*e^(-tau/b)
/// Then S = C - b*A + b*E and dS/db = E + F/b - A.
#[derive(Debug, Default, Clone, Copy)]
struct Decayed {
    a: f64,
    c: f64,
    e: f64,
    f: f64,
}

impl Decayed {
    /// Move the reference time forward by `dt >= 0`.
    #[inline]
    fn advance(&mut self, dt: f64, inv_b: f64) {
        let k = (-dt * inv_b).exp();
        self.c += self.a * dt;
        self.f = k * (self.f + dt * self.e);
        self.e *= k;
    }

    /// Add a step that has age `tau` (x = tau / b) at the current time.
    #[inline]
    fn absorb(&mut self, delta: f64, tau: f64, x: f64) {
        let ex = (-x).exp();
        self.a += delta;
        self.c += delta * tau;
        self.e += delta * ex;
        self.f += delta * tau * ex;
    }

    #[inline]
    fn response(&self, b: f64, inv_b: f64) -> (f64, f64) {
        (
            self.c - b * self.a + b * self.e,
            self.e + self.f * inv_b - self.a,
        )
    }
}

/// Result of evaluating the reduced problem at one `b`.
#[derive(Debug, Clone, Copy)]
pub struct Eval {
    /// Optimal gain a*(b).
    pub a: f64,
    /// L*(b) = L(a*(b), b).
    pub loss: f64,
    /// dL*/db.
    pub dloss: f64,
}

pub struct Evaluator {
    data: Dataset,
    /// Whether `data.times` is non-decreasing (enables the sweep).
    times_sorted: bool,
    /// Reused `(S, dS/db)` buffer. `try_lock` is used, so concurrent callers
    /// simply allocate their own instead of blocking.
    scratch: Mutex<Vec<(f64, f64)>>,
}

impl Evaluator {
    pub fn new(data: Dataset) -> Self {
        let times_sorted = data.times.windows(2).all(|w| w[0] <= w[1]);
        Self {
            data,
            times_sorted,
            scratch: Mutex::new(Vec::new()),
        }
    }

    /// Fill `out[k] = (S, dS/db)` at sample time index `start + k`.
    fn fill_responses(&self, b: f64, start: usize, out: &mut [(f64, f64)]) {
        if out.is_empty() {
            return;
        }
        let steps = &self.data.steps;
        let times = &self.data.times[start..start + out.len()];

        if !self.times_sorted {
            for (slot, &t) in out.iter_mut().zip(times) {
                *slot = response(steps, t, b);
            }
            return;
        }

        let inv_b = 1.0 / b;
        let mut state = Decayed::default();
        let mut t_ref = times[0];
        // steps[..fold]      : absorbed into `state`
        // steps[fold..next]  : started, evaluated directly
        let mut fold = 0usize;
        let mut next = steps.partition_point(|st| st.time <= t_ref);

        for (slot, &t) in out.iter_mut().zip(times) {
            state.advance(t - t_ref, inv_b);
            t_ref = t;

            while next < steps.len() && steps[next].time <= t {
                next += 1;
            }
            // Steps are time-ordered, so the oldest come first and ages are
            // monotone: stop at the first one that is still "recent".
            while fold < next {
                let st = &steps[fold];
                let tau = t - st.time;
                let x = tau * inv_b;
                if x < FOLD_X {
                    break;
                }
                state.absorb(st.delta, tau, x);
                fold += 1;
            }

            let (s_old, ds_old) = state.response(b, inv_b);
            let (s_new, ds_new) = direct_sum(&steps[fold..next], t, b, inv_b);
            *slot = (s_old + s_new, ds_old + ds_new);
        }
    }

    /// Fill the whole buffer, in parallel for large inputs. Every chunk starts
    /// from a fresh state, so chunks are independent.
    fn fill(&self, b: f64, buf: &mut [(f64, f64)]) {
        #[cfg(feature = "parallel")]
        {
            if buf.len() >= PAR_MIN_POINTS {
                buf.par_chunks_mut(CHUNK)
                    .enumerate()
                    .for_each(|(k, chunk)| self.fill_responses(b, k * CHUNK, chunk));
                return;
            }
        }
        self.fill_responses(b, 0, buf);
    }

    /// Compute a*(b), L*(b) and dL*/db.
    pub fn evaluate(&self, b: f64) -> Result<Eval, FitError> {
        if !b.is_finite() || b <= 0.0 {
            return Err(FitError::Numeric(format!(
                "invalid b = {b} (must be finite and > 0)"
            )));
        }
        let d = &self.data;
        let n = d.times.len().min(d.outputs.len());
        let y = &d.outputs[..n];

        let mut guard = self.scratch.try_lock();
        let mut local = Vec::new();
        let buf: &mut Vec<(f64, f64)> = match guard.as_deref_mut() {
            Ok(v) => v,
            Err(_) => &mut local,
        };
        buf.clear();
        buf.resize(n, (0.0, 0.0));
        self.fill(b, buf);

        // Pass 1: optimal gain.
        let (sum_sy, sum_ss) = reduce_chunks(buf, y, |sc, yc| {
            let mut sy = 0.0;
            let mut ss = 0.0;
            for (&(s, _), &yy) in sc.iter().zip(yc) {
                sy += s * yy;
                ss += s * s;
            }
            (sy, ss)
        });
        if !sum_ss.is_finite() || sum_ss < MIN_SUM_SQUARES {
            return Err(FitError::Numeric(format!(
                "sum(S^2) = {sum_ss:e} too small to compute a*(b) at b = {b}"
            )));
        }
        let a = sum_sy / sum_ss;

        // Pass 2: loss and derivative from residuals.
        let (loss, grad_sum) = reduce_chunks(buf, y, |sc, yc| {
            let mut l = 0.0;
            let mut g = 0.0;
            for (&(s, ds), &yy) in sc.iter().zip(yc) {
                let residual = yy - a * s;
                l += residual * residual;
                g += ds * residual;
            }
            (l, g)
        });
        let dloss = -2.0 * a * grad_sum;

        if !(a.is_finite() && loss.is_finite() && dloss.is_finite()) {
            return Err(FitError::Numeric(format!("non-finite result at b = {b}")));
        }
        //println!("b: {b}, a: {a}, loss: {loss}, dl/db: {dloss}");
        Ok(Eval { a, loss, dloss })
    }
}

/// Sum a pair of per-chunk partial results over fixed-size chunks of
/// `(buf, y)`. The chunking is identical for the sequential and parallel paths,
/// so the result does not depend on the thread count.
fn reduce_chunks<F>(buf: &[(f64, f64)], y: &[f64], f: F) -> (f64, f64)
where
    F: Fn(&[(f64, f64)], &[f64]) -> (f64, f64) + Sync + Send,
{
    #[cfg(feature = "parallel")]
    {
        if buf.len() >= PAR_MIN_POINTS {
            let parts: Vec<(f64, f64)> = buf
                .par_chunks(CHUNK)
                .zip(y.par_chunks(CHUNK))
                .map(|(sc, yc)| f(sc, yc))
                .collect();
            return parts
                .iter()
                .fold((0.0, 0.0), |acc, p| (acc.0 + p.0, acc.1 + p.1));
        }
    }
    let mut acc = (0.0, 0.0);
    for (sc, yc) in buf.chunks(CHUNK).zip(y.chunks(CHUNK)) {
        let p = f(sc, yc);
        acc.0 += p.0;
        acc.1 += p.1;
    }
    acc
}

