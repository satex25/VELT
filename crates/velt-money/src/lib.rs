//! Integer-minor-unit money and rate primitives for VELT.
//!
//! Doctrine §5: *Money is integer minor units. No `f64` anywhere in a financial
//! path.* This crate is the only place money is represented, and it represents
//! it as [`i64`] minor units (cents for USD) with a [`Currency`] tag.
//!
//! Every fallible operation returns [`Result`]; nothing here panics, saturates,
//! or silently wraps. An overflow in an underwriting calculation is a bug that
//! must surface, not a number that must be guessed.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use serde::{Deserialize, Serialize};
use std::fmt;
use utoipa::ToSchema;

/// Errors produced by money and rate arithmetic.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MoneyError {
    /// Arithmetic exceeded the range of the underlying integer.
    #[error("money arithmetic overflowed: {op}")]
    Overflow {
        /// The operation that overflowed.
        op: &'static str,
    },
    /// Two operands carried different currencies.
    #[error("currency mismatch: {lhs} vs {rhs}")]
    CurrencyMismatch {
        /// Currency of the left operand.
        lhs: Currency,
        /// Currency of the right operand.
        rhs: Currency,
    },
    /// A divisor was zero.
    #[error("division by zero: {op}")]
    DivideByZero {
        /// The operation that divided by zero.
        op: &'static str,
    },
    /// A value that must be positive was zero or negative.
    #[error("expected a positive value for {field}, got {value}")]
    NonPositive {
        /// Field name for the offending value.
        field: &'static str,
        /// The offending value.
        value: i64,
    },
}

/// Result alias for money arithmetic.
pub type Result<T> = std::result::Result<T, MoneyError>;

/// ISO-4217 currencies VELT supports.
///
/// VELT covers overseas acquisition, so currency is explicit on every amount
/// and mixing currencies is a hard error rather than an implicit conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "UPPERCASE")]
#[non_exhaustive]
pub enum Currency {
    /// United States dollar.
    Usd,
    /// Euro.
    Eur,
    /// Pound sterling.
    Gbp,
    /// Canadian dollar.
    Cad,
    /// Mexican peso.
    Mxn,
}

impl Currency {
    /// Number of decimal places in the currency's normal written form.
    ///
    /// All currencies currently supported are 2-exponent; the method exists so
    /// that adding a 0-exponent currency (JPY) or 3-exponent (KWD) is a data
    /// change rather than an audit of every call site.
    #[must_use]
    pub const fn exponent(self) -> u32 {
        match self {
            Self::Usd | Self::Eur | Self::Gbp | Self::Cad | Self::Mxn => 2,
        }
    }

    /// Number of minor units in one major unit (100 for a 2-exponent currency).
    #[must_use]
    pub const fn minor_units_per_major(self) -> i64 {
        match self.exponent() {
            0 => 1,
            1 => 10,
            2 => 100,
            3 => 1_000,
            _ => 1_000,
        }
    }

    /// ISO-4217 alphabetic code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Usd => "USD",
            Self::Eur => "EUR",
            Self::Gbp => "GBP",
            Self::Cad => "CAD",
            Self::Mxn => "MXN",
        }
    }
}

impl fmt::Display for Currency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

/// A monetary amount, stored as signed integer minor units.
///
/// `Money` is deliberately not `Add`/`Sub`/`Mul`: every operation is fallible
/// (currency mismatch, overflow) and the doctrine forbids silently-wrong money.
/// Use [`Money::add`], [`Money::sub`], [`Money::mul_int`], [`Money::apply`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
pub struct Money {
    /// Signed amount in minor units (cents for USD).
    minor: i64,
    /// Currency of this amount.
    currency: Currency,
}

impl Money {
    /// Construct from minor units (cents).
    #[must_use]
    pub const fn from_minor(minor: i64, currency: Currency) -> Self {
        Self { minor, currency }
    }

    /// Construct from whole major units (dollars).
    ///
    /// # Errors
    /// Returns [`MoneyError::Overflow`] if the amount does not fit in `i64`
    /// minor units.
    pub fn from_major(major: i64, currency: Currency) -> Result<Self> {
        let minor = major
            .checked_mul(currency.minor_units_per_major())
            .ok_or(MoneyError::Overflow { op: "from_major" })?;
        Ok(Self { minor, currency })
    }

    /// Zero in the given currency.
    #[must_use]
    pub const fn zero(currency: Currency) -> Self {
        Self { minor: 0, currency }
    }

    /// The raw minor-unit amount.
    #[must_use]
    pub const fn minor(self) -> i64 {
        self.minor
    }

    /// The currency tag.
    #[must_use]
    pub const fn currency(self) -> Currency {
        self.currency
    }

    /// True if the amount is exactly zero.
    #[must_use]
    pub const fn is_zero(self) -> bool {
        self.minor == 0
    }

    /// True if the amount is strictly greater than zero.
    #[must_use]
    pub const fn is_positive(self) -> bool {
        self.minor > 0
    }

    /// True if the amount is strictly less than zero.
    #[must_use]
    pub const fn is_negative(self) -> bool {
        self.minor < 0
    }

    fn same_currency(self, rhs: Self) -> Result<()> {
        if self.currency == rhs.currency {
            Ok(())
        } else {
            Err(MoneyError::CurrencyMismatch {
                lhs: self.currency,
                rhs: rhs.currency,
            })
        }
    }

    /// Checked addition. Currencies must match.
    ///
    /// # Errors
    /// [`MoneyError::CurrencyMismatch`] or [`MoneyError::Overflow`].
    /// Not `std::ops::Add`: that trait is infallible, and money addition here
    /// can fail on currency mismatch or overflow. Doctrine §5 forbids money
    /// arithmetic that cannot report being wrong.
    #[allow(clippy::should_implement_trait)]
    pub fn add(self, rhs: Self) -> Result<Self> {
        self.same_currency(rhs)?;
        let minor = self
            .minor
            .checked_add(rhs.minor)
            .ok_or(MoneyError::Overflow { op: "add" })?;
        Ok(Self {
            minor,
            currency: self.currency,
        })
    }

    /// Checked subtraction. Currencies must match.
    ///
    /// # Errors
    /// [`MoneyError::CurrencyMismatch`] or [`MoneyError::Overflow`].
    /// Not `std::ops::Add`: that trait is infallible, and money addition here
    /// can fail on currency mismatch or overflow. Doctrine §5 forbids money
    /// arithmetic that cannot report being wrong.
    #[allow(clippy::should_implement_trait)]
    pub fn sub(self, rhs: Self) -> Result<Self> {
        self.same_currency(rhs)?;
        let minor = self
            .minor
            .checked_sub(rhs.minor)
            .ok_or(MoneyError::Overflow { op: "sub" })?;
        Ok(Self {
            minor,
            currency: self.currency,
        })
    }

    /// Checked negation.
    ///
    /// # Errors
    /// [`MoneyError::Overflow`] at `i64::MIN`.
    /// Not `std::ops::Add`: that trait is infallible, and money addition here
    /// can fail on currency mismatch or overflow. Doctrine §5 forbids money
    /// arithmetic that cannot report being wrong.
    #[allow(clippy::should_implement_trait)]
    pub fn neg(self) -> Result<Self> {
        let minor = self
            .minor
            .checked_neg()
            .ok_or(MoneyError::Overflow { op: "neg" })?;
        Ok(Self {
            minor,
            currency: self.currency,
        })
    }

    /// Multiply by an integer scalar (e.g. 12 months).
    ///
    /// # Errors
    /// [`MoneyError::Overflow`].
    pub fn mul_int(self, n: i64) -> Result<Self> {
        let minor = self
            .minor
            .checked_mul(n)
            .ok_or(MoneyError::Overflow { op: "mul_int" })?;
        Ok(Self {
            minor,
            currency: self.currency,
        })
    }

    /// Sum an iterator of amounts, requiring a currency for the empty case.
    ///
    /// # Errors
    /// [`MoneyError::CurrencyMismatch`] or [`MoneyError::Overflow`].
    pub fn sum<I: IntoIterator<Item = Self>>(items: I, currency: Currency) -> Result<Self> {
        items
            .into_iter()
            .try_fold(Self::zero(currency), |acc, item| acc.add(item))
    }

    /// Apply a rate, rounding half-away-from-zero to the nearest minor unit.
    ///
    /// This is the single rounding policy in VELT. It is applied once, here, so
    /// that a rounding change is a one-line diff with one snapshot-test delta
    /// rather than a hunt through the engine.
    ///
    /// # Errors
    /// [`MoneyError::Overflow`] if the intermediate product exceeds `i128`
    /// range or the result exceeds `i64`.
    pub fn apply(self, rate: Bps) -> Result<Self> {
        let numer = i128::from(self.minor)
            .checked_mul(i128::from(rate.raw()))
            .ok_or(MoneyError::Overflow { op: "apply" })?;
        let denom = i128::from(Bps::SCALE);
        let minor = div_round_half_away(numer, denom, "apply")?;
        let minor = i64::try_from(minor).map_err(|_| MoneyError::Overflow { op: "apply" })?;
        Ok(Self {
            minor,
            currency: self.currency,
        })
    }

    /// Divide by an integer scalar, rounding half-away-from-zero.
    ///
    /// # Errors
    /// [`MoneyError::DivideByZero`] or [`MoneyError::Overflow`].
    pub fn div_int(self, n: i64) -> Result<Self> {
        if n == 0 {
            return Err(MoneyError::DivideByZero { op: "div_int" });
        }
        let minor = div_round_half_away(i128::from(self.minor), i128::from(n), "div_int")?;
        let minor = i64::try_from(minor).map_err(|_| MoneyError::Overflow { op: "div_int" })?;
        Ok(Self {
            minor,
            currency: self.currency,
        })
    }

    /// Express `self` as a rate of `base` in basis points.
    ///
    /// This is how every ratio metric in VELT (cap rate, cash-on-cash, DSCR) is
    /// produced: integer numerator over integer denominator, scaled to bps, with
    /// one rounding step.
    ///
    /// # Errors
    /// [`MoneyError::CurrencyMismatch`], [`MoneyError::NonPositive`] if `base`
    /// is not positive, or [`MoneyError::Overflow`].
    pub fn ratio_to(self, base: Self) -> Result<Bps> {
        self.same_currency(base)?;
        if !base.is_positive() {
            return Err(MoneyError::NonPositive {
                field: "ratio_to base",
                value: base.minor,
            });
        }
        let numer = i128::from(self.minor)
            .checked_mul(i128::from(Bps::SCALE))
            .ok_or(MoneyError::Overflow { op: "ratio_to" })?;
        let raw = div_round_half_away(numer, i128::from(base.minor), "ratio_to")?;
        let raw = i64::try_from(raw).map_err(|_| MoneyError::Overflow { op: "ratio_to" })?;
        Ok(Bps::from_raw(raw))
    }
}

impl fmt::Display for Money {
    /// Renders with the currency's full precision and no locale grouping.
    /// Presentation formatting belongs in the terminal UI, not in the type.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sign = if self.minor < 0 { "-" } else { "" };
        let abs = self.minor.unsigned_abs();
        let per = self.currency.minor_units_per_major().unsigned_abs().max(1);
        let major = abs.checked_div(per).unwrap_or(0);
        let minor = abs.checked_rem(per).unwrap_or(0);
        let width = usize::try_from(self.currency.exponent()).unwrap_or(2);
        write!(
            f,
            "{sign}{major}.{minor:0width$} {}",
            self.currency.code(),
            width = width
        )
    }
}

/// A rate in basis points scaled by [`Bps::SCALE`] (1 bp = 1/100 of a percent).
///
/// Rates are integers for the same reason money is: an interest rate that
/// drifts by a float ULP produces a payment schedule that is wrong by dollars
/// over 30 years.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, ToSchema,
)]
pub struct Bps(i64);

impl Bps {
    /// Basis points in 1.0 (100%). A rate of 5.25% is `Bps::from_raw(525)`.
    pub const SCALE: i64 = 10_000;

    /// Zero rate.
    pub const ZERO: Self = Self(0);

    /// 100%.
    pub const ONE: Self = Self(Self::SCALE);

    /// Construct from a raw basis-point count.
    #[must_use]
    pub const fn from_raw(raw: i64) -> Self {
        Self(raw)
    }

    /// Construct from whole percentage points (7% -> 700 bp).
    ///
    /// # Errors
    /// [`MoneyError::Overflow`].
    pub fn from_percent(pct: i64) -> Result<Self> {
        pct.checked_mul(100).map(Self).ok_or(MoneyError::Overflow {
            op: "Bps::from_percent",
        })
    }

    /// The raw basis-point count.
    #[must_use]
    pub const fn raw(self) -> i64 {
        self.0
    }

    /// True if strictly greater than zero.
    #[must_use]
    pub const fn is_positive(self) -> bool {
        self.0 > 0
    }

    /// Checked addition.
    ///
    /// # Errors
    /// [`MoneyError::Overflow`].
    /// Not `std::ops::Add`: that trait is infallible, and money addition here
    /// can fail on currency mismatch or overflow. Doctrine §5 forbids money
    /// arithmetic that cannot report being wrong.
    #[allow(clippy::should_implement_trait)]
    pub fn add(self, rhs: Self) -> Result<Self> {
        self.0
            .checked_add(rhs.0)
            .map(Self)
            .ok_or(MoneyError::Overflow { op: "Bps::add" })
    }

    /// Checked subtraction.
    ///
    /// # Errors
    /// [`MoneyError::Overflow`].
    /// Not `std::ops::Add`: that trait is infallible, and money addition here
    /// can fail on currency mismatch or overflow. Doctrine §5 forbids money
    /// arithmetic that cannot report being wrong.
    #[allow(clippy::should_implement_trait)]
    pub fn sub(self, rhs: Self) -> Result<Self> {
        self.0
            .checked_sub(rhs.0)
            .map(Self)
            .ok_or(MoneyError::Overflow { op: "Bps::sub" })
    }

    /// The complement of this rate (`1 - self`), e.g. 1 - vacancy.
    ///
    /// # Errors
    /// [`MoneyError::Overflow`].
    pub fn complement(self) -> Result<Self> {
        Self::ONE.sub(self)
    }

    /// Divide this rate by an integer (e.g. an annual rate to monthly).
    ///
    /// # Errors
    /// [`MoneyError::DivideByZero`] or [`MoneyError::Overflow`].
    pub fn div_int(self, n: i64) -> Result<Self> {
        if n == 0 {
            return Err(MoneyError::DivideByZero { op: "Bps::div_int" });
        }
        let raw = div_round_half_away(i128::from(self.0), i128::from(n), "Bps::div_int")?;
        i64::try_from(raw)
            .map(Self)
            .map_err(|_| MoneyError::Overflow { op: "Bps::div_int" })
    }
}

impl fmt::Display for Bps {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sign = if self.0 < 0 { "-" } else { "" };
        let abs = self.0.unsigned_abs();
        let whole = abs.checked_div(100).unwrap_or(0);
        let frac = abs.checked_rem(100).unwrap_or(0);
        write!(f, "{sign}{whole}.{frac:02}%")
    }
}

/// Integer division rounding half away from zero.
///
/// The one rounding primitive in VELT — this function and no other copy.
/// Half-away-from-zero is chosen over banker's rounding because it matches the
/// convention lenders and HUD schedules use in published figures, so
/// hand-verified fixtures agree to the cent.
///
/// Public because `velt-engine`'s fixed-point amortization needs exactly this
/// rounding and previously carried its own duplicate. Two copies meant a
/// rounding change was a two-crate edit that could silently diverge, and in
/// practice one copy was sign-tested and the other was not. Do not reintroduce
/// a second implementation; call this.
///
/// `op` labels the operation in any error, so a caller keeps its own error
/// vocabulary rather than reporting a generic rounding failure.
///
/// # Errors
/// [`MoneyError::DivideByZero`] if `denom` is zero, or [`MoneyError::Overflow`]
/// if the correction step leaves `i128` range.
pub fn div_round_half_away(numer: i128, denom: i128, op: &'static str) -> Result<i128> {
    if denom == 0 {
        return Err(MoneyError::DivideByZero { op });
    }
    // checked_div/checked_rem rather than `/` and `%`: i128::MIN / -1 overflows,
    // and doctrine §5 does not permit a financial path that can panic.
    let quot = numer
        .checked_div(denom)
        .ok_or(MoneyError::Overflow { op })?;
    let rem = numer
        .checked_rem(denom)
        .ok_or(MoneyError::Overflow { op })?;
    if rem == 0 {
        return Ok(quot);
    }
    let twice = rem.checked_mul(2).ok_or(MoneyError::Overflow { op })?;
    if twice.abs() >= denom.abs() {
        let step = if (numer < 0) == (denom < 0) { 1 } else { -1 };
        quot.checked_add(step).ok_or(MoneyError::Overflow { op })
    } else {
        Ok(quot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const USD: Currency = Currency::Usd;

    fn usd(minor: i64) -> Money {
        Money::from_minor(minor, USD)
    }

    #[test]
    fn from_major_scales_to_minor_units() {
        assert_eq!(Money::from_major(1_500, USD).unwrap().minor(), 150_000);
    }

    #[test]
    fn addition_requires_matching_currency() {
        let a = usd(100);
        let b = Money::from_minor(100, Currency::Eur);
        assert_eq!(
            a.add(b),
            Err(MoneyError::CurrencyMismatch {
                lhs: Currency::Usd,
                rhs: Currency::Eur
            })
        );
    }

    #[test]
    fn addition_overflow_is_an_error_not_a_wrap() {
        assert_eq!(
            usd(i64::MAX).add(usd(1)),
            Err(MoneyError::Overflow { op: "add" })
        );
    }

    #[test]
    fn apply_rate_rounds_half_away_from_zero() {
        // 1000 cents at 5.00% = 50 cents exactly.
        assert_eq!(usd(1_000).apply(Bps::from_raw(500)).unwrap(), usd(50));
        // 1005 cents at 50.00% = 502.5 -> 503 (half away from zero).
        assert_eq!(usd(1_005).apply(Bps::from_raw(5_000)).unwrap(), usd(503));
        // Negative amount rounds away from zero in the negative direction.
        assert_eq!(usd(-1_005).apply(Bps::from_raw(5_000)).unwrap(), usd(-503));
    }

    #[test]
    fn ratio_to_produces_basis_points() {
        // 7,200 / 100,000 = 7.20% = 720 bp
        let noi = usd(720_000);
        let price = usd(10_000_000);
        assert_eq!(noi.ratio_to(price).unwrap(), Bps::from_raw(720));
    }

    #[test]
    fn ratio_to_rejects_non_positive_base() {
        assert_eq!(
            usd(100).ratio_to(usd(0)),
            Err(MoneyError::NonPositive {
                field: "ratio_to base",
                value: 0
            })
        );
    }

    #[test]
    fn complement_of_vacancy_is_occupancy() {
        let vacancy = Bps::from_raw(750); // 7.5%
        assert_eq!(vacancy.complement().unwrap(), Bps::from_raw(9_250));
    }

    #[test]
    fn display_is_exact_and_unambiguous() {
        assert_eq!(usd(150_000).to_string(), "1500.00 USD");
        assert_eq!(usd(-5).to_string(), "-0.05 USD");
        assert_eq!(Bps::from_raw(725).to_string(), "7.25%");
    }

    #[test]
    fn sum_of_empty_is_zero_in_the_stated_currency() {
        let empty: Vec<Money> = vec![];
        assert_eq!(Money::sum(empty, USD).unwrap(), Money::zero(USD));
    }

    #[test]
    fn div_round_half_away_matches_hand_verified_cases() {
        assert_eq!(div_round_half_away(5, 2, "t").unwrap(), 3);
        assert_eq!(div_round_half_away(-5, 2, "t").unwrap(), -3);
        assert_eq!(div_round_half_away(5, -2, "t").unwrap(), -3);
        assert_eq!(div_round_half_away(-5, -2, "t").unwrap(), 3);
        assert_eq!(div_round_half_away(4, 2, "t").unwrap(), 2);
        assert_eq!(div_round_half_away(1, 3, "t").unwrap(), 0);
    }

    #[test]
    fn the_op_label_is_carried_into_the_error() {
        assert_eq!(
            div_round_half_away(1, 0, "caller_name"),
            Err(MoneyError::DivideByZero { op: "caller_name" })
        );
    }

    /// `cargo mutants` replaced each of these predicates with a constant and
    /// flipped each comparison on 2026-08-02; thirteen mutants survived because
    /// nothing called them directly. They guard real branches — `ratio_to`
    /// rejects a non-positive base with `is_positive` — so a predicate that
    /// silently answers backwards mis-states a cap rate rather than failing.
    ///
    /// Each predicate is asserted at negative, zero and positive, because a
    /// single case cannot distinguish `<` from `<=` or `==`.
    #[test]
    fn money_sign_predicates_are_asserted_at_every_boundary() {
        assert!(usd(0).is_zero());
        assert!(!usd(1).is_zero());
        assert!(!usd(-1).is_zero());

        assert!(usd(1).is_positive());
        assert!(!usd(0).is_positive(), "zero is not positive");
        assert!(!usd(-1).is_positive());

        assert!(usd(-1).is_negative());
        assert!(!usd(0).is_negative(), "zero is not negative");
        assert!(!usd(1).is_negative());
    }

    #[test]
    fn bps_is_positive_only_strictly_above_zero() {
        assert!(Bps::from_raw(1).is_positive());
        assert!(!Bps::ZERO.is_positive(), "zero is not positive");
        assert!(!Bps::from_raw(-1).is_positive());
    }

    /// Both assertions are load-bearing. Mutating the guard to `n != 0` still
    /// produces a `DivideByZero` for `n == 0`, because the rounding primitive
    /// rejects it a line later with the same label — so only the non-zero case
    /// distinguishes the mutant from the original.
    #[test]
    fn div_int_rejects_a_zero_divisor_and_divides_otherwise() {
        assert_eq!(
            usd(100).div_int(0),
            Err(MoneyError::DivideByZero { op: "div_int" })
        );
        assert_eq!(usd(100).div_int(4).unwrap(), usd(25));
    }

    #[test]
    fn bps_div_int_rejects_a_zero_divisor_and_divides_otherwise() {
        assert_eq!(
            Bps::from_raw(1_200).div_int(0),
            Err(MoneyError::DivideByZero { op: "Bps::div_int" })
        );
        assert_eq!(
            Bps::from_raw(1_200).div_int(12).unwrap(),
            Bps::from_raw(100)
        );
    }
}
