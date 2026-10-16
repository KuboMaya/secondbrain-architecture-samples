//! Portfolio sample — reward curve, hard-cap, budget clamp.
//! Tests mirror production `tests/reward/math_tests.rs` + `mcdc_tests.rs`.
//! Source: `domains/reward/services.rs` (math helpers).

/// Soft cap ratio applied by the daily batch against monthly_budget.
pub const SOFT_CAP_RATIO: f64 = 0.8;

/// UI spend buttons (points deducted from `hidden_points`).
pub const SPEND_AMOUNTS: [f64; 4] = [500.0, 1000.0, 2000.0, 5000.0];

pub fn hard_cap_limit(monthly_budget: f64) -> f64 {
    if monthly_budget.is_finite() {
        monthly_budget.max(0.0)
    } else {
        0.0
    }
}

/// Raw reward curve: `10x^3 + 50x^50`.
pub fn compute_raw(x: f64) -> f64 {
    if !x.is_finite() || x <= 0.0 {
        return 0.0;
    }
    let cubic = 10.0 * x.powi(3);
    let high = 50.0 * x.powi(50);
    let sum = cubic + high;
    if sum.is_finite() {
        sum
    } else {
        f64::INFINITY
    }
}

pub fn compute_capped(x: f64, monthly_budget: f64) -> f64 {
    let raw = compute_raw(x);
    let cap = hard_cap_limit(monthly_budget);
    if raw >= cap {
        cap
    } else {
        raw
    }
}

pub fn is_hard_capped(x: f64, monthly_budget: f64) -> bool {
    compute_raw(x) >= hard_cap_limit(monthly_budget)
}

pub fn apply_budget(amount: f64, hidden_points: f64, monthly_budget: f64) -> f64 {
    let remaining = (monthly_budget - hidden_points).max(0.0);
    amount.min(remaining).max(0.0)
}

pub fn drop_ratio(raw: f64, amount: f64) -> f64 {
    if !raw.is_finite() || raw <= 0.0 || !amount.is_finite() || amount <= 0.0 {
        0.0
    } else {
        amount / raw
    }
}

/// Full gacha evaluation: hard-cap then remaining-budget clamp.
pub fn evaluate_gacha(x: f64, hidden_points: f64, monthly_budget: f64) -> (f64, f64, bool) {
    let raw = compute_raw(x);
    let capped = compute_capped(x, monthly_budget);
    let amount = apply_budget(capped, hidden_points, monthly_budget);
    let hard_capped = raw >= hard_cap_limit(monthly_budget);
    (amount, drop_ratio(raw, amount), hard_capped)
}

pub fn apply_soft_cap(hidden_points: f64, monthly_budget: f64) -> f64 {
    let soft = (monthly_budget * SOFT_CAP_RATIO).max(0.0);
    hidden_points.min(soft)
}

pub fn is_allowed_spend(amount: f64) -> bool {
    SPEND_AMOUNTS
        .iter()
        .any(|allowed| (*allowed - amount).abs() < f64::EPSILON)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1e-9,
            "expected {expected}, got {actual}"
        );
    }

    // ── math_tests style ────────────────────────────────────────────────────

    #[test]
    fn raw_curve_zero_and_negative() {
        assert_eq!(compute_raw(0.0), 0.0);
        assert_eq!(compute_raw(-1.0), 0.0);
        assert_eq!(compute_raw(f64::NAN), 0.0);
    }

    #[test]
    fn raw_curve_at_one() {
        // 10*(1^3) + 50*(1^50) = 60
        assert_close(compute_raw(1.0), 60.0);
    }

    #[test]
    fn cubic_term_dominates_below_one() {
        let x: f64 = 0.5;
        let cubic = 10.0 * x.powi(3);
        let high = 50.0 * x.powi(50);
        assert!(cubic > high);
        assert_close(compute_raw(x), cubic + high);
    }

    #[test]
    fn high_term_dominates_above_one() {
        let x: f64 = 1.3;
        let cubic = 10.0 * x.powi(3);
        let high = 50.0 * x.powi(50);
        assert!(high > cubic);
        let budget = 10_000.0;
        assert_eq!(compute_capped(x, budget), hard_cap_limit(budget));
        assert!(is_hard_capped(x, budget));
    }

    #[test]
    fn hard_cap_not_applied_when_under_budget() {
        let budget = 10_000.0;
        assert_close(compute_capped(1.0, budget), 60.0);
        assert!(!is_hard_capped(1.0, budget));
    }

    #[test]
    fn hard_cap_equals_monthly_budget_when_raw_exceeds() {
        let budget = 100.0;
        assert_eq!(compute_capped(1.5, budget), 100.0);
        assert_eq!(compute_capped(2.0, budget), hard_cap_limit(budget));
    }

    #[test]
    fn hard_cap_boundary_raw_equals_budget() {
        let budget = 60.0;
        assert_close(compute_raw(1.0), 60.0);
        assert!(is_hard_capped(1.0, budget));
        assert_eq!(compute_capped(1.0, budget), 60.0);
    }

    #[test]
    fn budget_clamp_independent_of_formula() {
        assert_close(apply_budget(60.0, 0.0, 100.0), 60.0);
        assert_close(apply_budget(60.0, 80.0, 100.0), 20.0);
        assert_close(apply_budget(60.0, 100.0, 100.0), 0.0);
        assert_close(apply_budget(60.0, 150.0, 100.0), 0.0);
    }

    #[test]
    fn soft_cap_is_ratio_of_monthly_budget() {
        assert_close(
            apply_soft_cap(10_000.0, 10_000.0),
            10_000.0 * SOFT_CAP_RATIO,
        );
        assert_close(apply_soft_cap(100.0, 10_000.0), 100.0);
    }

    // ── mcdc_tests style ────────────────────────────────────────────────────

    /// MC/DC for H: `hard_capped = (raw >= hard_cap_limit(monthly_budget))`
    #[test]
    fn mcdc_hard_cap_decision_independent_conditions() {
        let x_under = 1.0;
        let budget_loose = 100.0;
        assert_close(compute_raw(x_under), 60.0);
        assert!(!is_hard_capped(x_under, budget_loose));
        assert_close(compute_capped(x_under, budget_loose), 60.0);

        let x_over = 1.5;
        assert!(compute_raw(x_over) > budget_loose);
        assert!(is_hard_capped(x_over, budget_loose));
        assert_eq!(
            compute_capped(x_over, budget_loose),
            hard_cap_limit(budget_loose)
        );

        let raw_over = compute_raw(x_over);
        let budget_above_raw = raw_over + 1.0;
        assert!(!is_hard_capped(x_over, budget_above_raw));
        assert_close(compute_capped(x_over, budget_above_raw), raw_over);

        let budget_eq = 60.0;
        assert!(is_hard_capped(1.0, budget_eq));
        assert_eq!(compute_capped(1.0, budget_eq), 60.0);
    }

    #[test]
    fn mcdc_drop_ratio_falls_when_hard_cap_binds() {
        let budget = 100.0;
        let hidden = 0.0;

        let (amount_under, ratio_under, hard_under) = evaluate_gacha(1.0, hidden, budget);
        assert!(!hard_under);
        assert_close(amount_under, 60.0);
        assert_close(ratio_under, 1.0);

        let raw_over = compute_raw(1.5);
        let (amount_over, ratio_over, hard_over) = evaluate_gacha(1.5, hidden, budget);
        assert!(hard_over);
        assert_close(amount_over, 100.0);
        assert_close(ratio_over, 100.0 / raw_over);
        assert!(ratio_over < 1.0);
        assert!(ratio_over < ratio_under);
    }

    #[test]
    fn mcdc_drop_rate_zero_when_monthly_budget_exhausted() {
        let budget = 100.0;
        let x = 1.0;

        let (amount_ok, ratio_ok, _) = evaluate_gacha(x, 0.0, budget);
        assert!(amount_ok > 0.0);
        assert!(ratio_ok > 0.0);

        let (amount_zero, ratio_zero, _) = evaluate_gacha(x, 100.0, budget);
        assert_close(amount_zero, 0.0);
        assert_close(ratio_zero, 0.0);

        // Partial remaining independently clamps payout.
        let (amount_partial, _, _) = evaluate_gacha(x, 80.0, budget);
        assert_close(amount_partial, 20.0);
    }

    #[test]
    fn spend_allowlist() {
        assert!(is_allowed_spend(500.0));
        assert!(is_allowed_spend(5000.0));
        assert!(!is_allowed_spend(123.0));
    }
}
