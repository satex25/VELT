//! Fixed-point amortization.
//!
//! A 30-year payment schedule computed in `f64` drifts by dollars over its life.
//! Doctrine §5 bans floats in financial paths, so compounding is done in i128
//! fixed point at [`SCALE`] and rounded exactly once, at the end.

use velt_money::{Bps, Currency, Money, MoneyError};

/// Fixed-point scale for compounding intermediates (1.0 == 10^12).
///
/// 12 digits keeps `(1 + monthly_rate)^480` accurate well past the cent for any
/// rate and term VELT will underwrite, while leaving i128 headroom for a loan
/// principal in the billions.
pub const SCALE: i128 = 1_000_000_000_000;

/// Months in a year.
pub const MONTHS_PER_YEAR: i64 = 12;

type Result<T> = std::result::Result<T, MoneyError>;

/// Multiply two fixed-point values.
fn mul_fp(a: i128, b: i128) -> Result<i128> {
    let product = a.checked_mul(b).ok_or(MoneyError::Overflow {
        op: "amort::mul_fp",
    })?;
    product.checked_div(SCALE).ok_or(MoneyError::Overflow {
        op: "amort::mul_fp",
    })
}

/// Raise a fixed-point base to an integer power by binary exponentiation.
fn pow_fp(base: i128, exp: u32) -> Result<i128> {
    let mut result = SCALE;
    let mut base = base;
    let mut exp = exp;
    while exp > 0 {
        if exp & 1 == 1 {
            result = mul_fp(result, base)?;
        }
        exp >>= 1;
        if exp > 0 {
            base = mul_fp(base, base)?;
        }
    }
    Ok(result)
}

/// Integer division rounding half away from zero.
fn div_round_half_away(numer: i128, denom: i128) -> Result<i128> {
    if denom == 0 {
        return Err(MoneyError::DivideByZero { op: "amort::div" });
    }
    // checked_div/checked_rem rather than `/` and `%`: i128::MIN / -1 overflows,
    // and doctrine §5 does not permit a financial path that can panic.
    let quot = numer
        .checked_div(denom)
        .ok_or(MoneyError::Overflow { op: "amort::div" })?;
    let rem = numer
        .checked_rem(denom)
        .ok_or(MoneyError::Overflow { op: "amort::div" })?;
    if rem == 0 {
        return Ok(quot);
    }
    let twice = rem
        .checked_mul(2)
        .ok_or(MoneyError::Overflow { op: "amort::div" })?;
    if twice.abs() >= denom.abs() {
        let step = if (numer < 0) == (denom < 0) { 1 } else { -1 };
        quot.checked_add(step)
            .ok_or(MoneyError::Overflow { op: "amort::div" })
    } else {
        Ok(quot)
    }
}

/// The periodic (monthly) rate as fixed point, from an annual nominal rate.
fn monthly_rate_fp(annual: Bps) -> Result<i128> {
    let numer = i128::from(annual.raw())
        .checked_mul(SCALE)
        .ok_or(MoneyError::Overflow {
            op: "amort::monthly_rate",
        })?;
    let denom = i128::from(Bps::SCALE)
        .checked_mul(i128::from(MONTHS_PER_YEAR))
        .ok_or(MoneyError::Overflow {
            op: "amort::monthly_rate",
        })?;
    numer.checked_div(denom).ok_or(MoneyError::Overflow {
        op: "amort::monthly_rate",
    })
}

/// The level monthly payment that fully amortizes `principal` over `term_months`
/// at annual nominal rate `annual_rate`, compounded monthly.
///
/// A zero rate degrades to straight-line principal repayment rather than
/// dividing by zero.
///
/// # Errors
/// [`MoneyError::NonPositive`] if the term is not positive,
/// [`MoneyError::Overflow`] if the compounding intermediates exceed i128.
pub fn monthly_payment(principal: Money, annual_rate: Bps, term_months: u32) -> Result<Money> {
    if term_months == 0 {
        return Err(MoneyError::NonPositive {
            field: "term_months",
            value: 0,
        });
    }
    if principal.is_zero() {
        return Ok(Money::zero(principal.currency()));
    }

    let rate_fp = monthly_rate_fp(annual_rate)?;

    // Zero-interest loan: principal spread evenly across the term.
    if rate_fp == 0 {
        return principal.div_int(i64::from(term_months));
    }

    let one_plus = SCALE.checked_add(rate_fp).ok_or(MoneyError::Overflow {
        op: "amort::one_plus",
    })?;
    let growth = pow_fp(one_plus, term_months)?;
    let growth_less_one = growth.checked_sub(SCALE).ok_or(MoneyError::Overflow {
        op: "amort::growth",
    })?;
    if growth_less_one <= 0 {
        return Err(MoneyError::Overflow {
            op: "amort::growth_underflow",
        });
    }

    // payment = L * r * (1+r)^n / ((1+r)^n - 1)
    // Held in i128 with a single rounding at the end, so the schedule agrees
    // with a lender's amortization table to the cent.
    let numer = i128::from(principal.minor())
        .checked_mul(rate_fp)
        .ok_or(MoneyError::Overflow { op: "amort::numer" })?
        .checked_mul(growth)
        .ok_or(MoneyError::Overflow { op: "amort::numer" })?;
    let denom = SCALE
        .checked_mul(growth_less_one)
        .ok_or(MoneyError::Overflow { op: "amort::denom" })?;

    let minor = div_round_half_away(numer, denom)?;
    let minor = i64::try_from(minor).map_err(|_| MoneyError::Overflow {
        op: "amort::payment",
    })?;
    Ok(Money::from_minor(minor, principal.currency()))
}

/// Total debt service over twelve months.
///
/// # Errors
/// As [`monthly_payment`], plus [`MoneyError::Overflow`].
pub fn annual_debt_service(principal: Money, annual_rate: Bps, term_months: u32) -> Result<Money> {
    monthly_payment(principal, annual_rate, term_months)?.mul_int(MONTHS_PER_YEAR)
}

/// Remaining balance after `elapsed_months` payments.
///
/// Computed from the closed-form balance rather than by iterating the schedule,
/// so a 30-year hold is the same cost as a 1-year hold.
///
/// # Errors
/// [`MoneyError::Overflow`] or [`MoneyError::NonPositive`].
pub fn remaining_balance(
    principal: Money,
    annual_rate: Bps,
    term_months: u32,
    elapsed_months: u32,
) -> Result<Money> {
    if elapsed_months >= term_months {
        return Ok(Money::zero(principal.currency()));
    }
    let payment = monthly_payment(principal, annual_rate, term_months)?;
    let rate_fp = monthly_rate_fp(annual_rate)?;

    if rate_fp == 0 {
        let paid = payment.mul_int(i64::from(elapsed_months))?;
        return principal.sub(paid);
    }

    let one_plus = SCALE.checked_add(rate_fp).ok_or(MoneyError::Overflow {
        op: "amort::one_plus",
    })?;
    let growth = pow_fp(one_plus, elapsed_months)?;
    let growth_less_one = growth.checked_sub(SCALE).ok_or(MoneyError::Overflow {
        op: "amort::balance",
    })?;

    // balance = L*(1+r)^k - P*((1+r)^k - 1)/r
    //
    // Both terms are put over the common denominator SCALE*r and rounded once.
    // Rounding each term separately admits +/-1 cent of error, which is exactly
    // the kind of quiet drift doctrine §2 calls worse than useless.
    let grown = i128::from(principal.minor())
        .checked_mul(growth)
        .ok_or(MoneyError::Overflow {
            op: "amort::balance",
        })?
        .checked_mul(rate_fp)
        .ok_or(MoneyError::Overflow {
            op: "amort::balance",
        })?;
    let repaid = i128::from(payment.minor())
        .checked_mul(growth_less_one)
        .ok_or(MoneyError::Overflow {
            op: "amort::balance",
        })?
        .checked_mul(SCALE)
        .ok_or(MoneyError::Overflow {
            op: "amort::balance",
        })?;
    let numer = grown.checked_sub(repaid).ok_or(MoneyError::Overflow {
        op: "amort::balance",
    })?;
    let denom = SCALE.checked_mul(rate_fp).ok_or(MoneyError::Overflow {
        op: "amort::balance",
    })?;

    let minor = div_round_half_away(numer, denom)?;
    let minor = i64::try_from(minor).map_err(|_| MoneyError::Overflow {
        op: "amort::balance",
    })?;
    Ok(Money::from_minor(minor.max(0), principal.currency()))
}

/// Loan principal implied by a purchase price and a down payment rate.
///
/// # Errors
/// [`MoneyError::Overflow`].
pub fn principal_from_ltv(price: Money, down_payment_rate: Bps) -> Result<Money> {
    let down = price.apply(down_payment_rate)?;
    price.sub(down)
}

/// Convenience: USD zero.
#[must_use]
pub const fn usd_zero() -> Money {
    Money::zero(Currency::Usd)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usd(minor: i64) -> Money {
        Money::from_minor(minor, Currency::Usd)
    }

    /// Hand-verified against a standard amortization table:
    /// $250,000.00 at 7.00% nominal, 360 months -> $1,663.26/mo.
    #[test]
    fn payment_matches_hand_verified_thirty_year_fixed() {
        let payment = monthly_payment(usd(25_000_000), Bps::from_raw(700), 360).unwrap();
        assert_eq!(payment, usd(166_326), "got {payment}");
    }

    /// $400,000.00 at 6.50% nominal, 360 months -> $2,528.27/mo.
    #[test]
    fn payment_matches_hand_verified_second_case() {
        let payment = monthly_payment(usd(40_000_000), Bps::from_raw(650), 360).unwrap();
        assert_eq!(payment, usd(252_827), "got {payment}");
    }

    /// $100,000.00 at 5.00% nominal, 180 months -> $790.79/mo.
    #[test]
    fn payment_matches_hand_verified_fifteen_year() {
        let payment = monthly_payment(usd(10_000_000), Bps::from_raw(500), 180).unwrap();
        assert_eq!(payment, usd(79_079), "got {payment}");
    }

    #[test]
    fn zero_rate_is_straight_line_not_a_division_by_zero() {
        let payment = monthly_payment(usd(1_200_000), Bps::ZERO, 12).unwrap();
        assert_eq!(payment, usd(100_000));
    }

    #[test]
    fn zero_principal_pays_nothing() {
        assert_eq!(
            monthly_payment(usd(0), Bps::from_raw(700), 360).unwrap(),
            usd(0)
        );
    }

    #[test]
    fn zero_term_is_rejected() {
        assert!(monthly_payment(usd(1_000), Bps::from_raw(700), 0).is_err());
    }

    #[test]
    fn balance_at_origination_is_the_full_principal() {
        let bal = remaining_balance(usd(25_000_000), Bps::from_raw(700), 360, 0).unwrap();
        assert_eq!(bal, usd(25_000_000));
    }

    #[test]
    fn balance_at_maturity_is_zero() {
        let bal = remaining_balance(usd(25_000_000), Bps::from_raw(700), 360, 360).unwrap();
        assert_eq!(bal, usd(0));
    }

    /// $250k / 7% / 360mo after 60 payments -> $235,328.71 remaining.
    /// Early amortization is nearly all interest; this catches a sign or
    /// scale error that a maturity-only test would miss.
    #[test]
    fn balance_after_five_years_is_barely_paid_down() {
        let bal = remaining_balance(usd(25_000_000), Bps::from_raw(700), 360, 60).unwrap();
        assert_eq!(bal, usd(23_532_871), "got {bal}");
    }

    #[test]
    fn ltv_splits_price_into_down_payment_and_principal() {
        // 20% down on $300,000 -> $240,000 borrowed.
        let principal = principal_from_ltv(usd(30_000_000), Bps::from_raw(2_000)).unwrap();
        assert_eq!(principal, usd(24_000_000));
    }

    #[test]
    fn annual_debt_service_is_twelve_payments() {
        let annual = annual_debt_service(usd(25_000_000), Bps::from_raw(700), 360).unwrap();
        assert_eq!(annual, usd(166_326 * 12));
    }

    #[test]
    fn a_higher_rate_always_costs_more() {
        let low = monthly_payment(usd(25_000_000), Bps::from_raw(500), 360).unwrap();
        let high = monthly_payment(usd(25_000_000), Bps::from_raw(700), 360).unwrap();
        assert!(high.minor() > low.minor());
    }

    #[test]
    fn a_billion_dollar_loan_does_not_overflow() {
        let payment = monthly_payment(usd(100_000_000_000), Bps::from_raw(700), 360).unwrap();
        assert!(payment.is_positive());
    }
}
