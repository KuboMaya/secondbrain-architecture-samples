//! Portfolio sample — reward curve, hard-cap, budget clamp (pure functions + tests).
//! High branch density for static analysis; no personal data.
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

    #[test]
    fn raw_zero_and_positive() {
        assert_eq!(compute_raw(0.0), 0.0);
        assert_eq!(compute_raw(-1.0), 0.0);
        assert!(compute_raw(1.0) > 0.0);
    }

    #[test]
    fn hard_cap_clamps_to_budget() {
        let budget = 100.0;
        assert!(is_hard_capped(2.0, budget));
        assert_eq!(compute_capped(2.0, budget), budget);
    }

    #[test]
    fn apply_budget_respects_remaining() {
        assert_eq!(apply_budget(80.0, 40.0, 100.0), 60.0);
        assert_eq!(apply_budget(80.0, 100.0, 100.0), 0.0);
    }

    #[test]
    fn evaluate_gacha_tuple() {
        let (amount, ratio, hard) = evaluate_gacha(1.0, 0.0, 10_000.0);
        assert!(amount > 0.0);
        assert!(ratio > 0.0);
        assert!(!hard);
    }

    #[test]
    fn soft_cap_and_spend_allowlist() {
        assert_eq!(apply_soft_cap(9000.0, 10_000.0), 8000.0);
        assert!(is_allowed_spend(500.0));
        assert!(!is_allowed_spend(123.0));
    }

    #[test]
    fn drop_ratio_guards() {
        assert_eq!(drop_ratio(0.0, 10.0), 0.0);
        assert_eq!(drop_ratio(100.0, 25.0), 0.25);
    }
}
