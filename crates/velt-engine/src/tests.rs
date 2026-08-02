//! Engine tests.
//!
//! Doctrine §9: *Tests pass, including snapshot tests to the cent on any
//! financial path.* Every expected value below was produced by an independent
//! 60-digit decimal implementation of the same formulas and is asserted
//! exactly — no tolerance windows, because a tolerance window is how a
//! systematic error survives a test suite.

use super::*;
use velt_provenance::{Origin, Traced};

const USD: Currency = Currency::Usd;

fn usd(minor: i64) -> Money {
    Money::from_minor(minor, USD)
}

fn operator_money(minor: i64, field: &str) -> Traced<Money> {
    Traced::leaf(
        usd(minor),
        field,
        Origin::UserInput {
            field: field.to_owned(),
        },
    )
}

fn operator_bps(raw: i64, field: &str) -> Traced<Bps> {
    Traced::leaf(
        Bps::from_raw(raw),
        field,
        Origin::UserInput {
            field: field.to_owned(),
        },
    )
}

/// A voucher-eligible single-family rental underwritten off the HUD Fair Market
/// Rent for the unit. The rent leaf is deliberately marked `External` so the
/// provenance assertions exercise the real path rather than an operator entry.
fn section8_sfr() -> (UnderwriteInputs, TracedInputs) {
    let inputs = UnderwriteInputs {
        income: IncomeInputs {
            monthly_scheduled_rent: usd(185_000),
            other_annual_income: usd(0),
            vacancy_rate: Bps::from_raw(700),
        },
        expenses: ExpenseInputs {
            annual_property_tax: usd(320_000),
            annual_insurance: usd(145_000),
            annual_hoa: usd(0),
            annual_utilities: usd(0),
            annual_other_fixed: usd(0),
            management_rate: Bps::from_raw(800),
            maintenance_rate: Bps::from_raw(500),
            capex_reserve_rate: Bps::from_raw(500),
        },
        deal: DealInputs {
            purchase_price: usd(21_500_000),
            closing_costs: usd(645_000),
            rehab_budget: usd(1_200_000),
            down_payment_rate: Bps::from_raw(2_500),
            interest_rate: Bps::from_raw(725),
            term_months: 360,
        },
    };

    let traced = TracedInputs {
        monthly_rent: Traced::leaf(
            usd(185_000),
            "HUD FMR 3BR",
            Origin::External {
                source_id: "hud.fmr".into(),
                trust_tier: "authoritative".into(),
                fetched_at: "2026-08-01T00:00:00Z".into(),
            },
        ),
        other_income: operator_money(0, "other_income"),
        vacancy_rate: Traced::leaf(
            Bps::from_raw(700),
            "Vacancy",
            Origin::Assumption {
                name: "vacancy_rate".into(),
                rationale: "7% economic vacancy default for voucher tenancy".into(),
            },
        ),
        property_tax: Traced::leaf(
            usd(320_000),
            "Property tax",
            Origin::External {
                source_id: "county.assessor".into(),
                trust_tier: "authoritative".into(),
                fetched_at: "2026-08-01T00:00:00Z".into(),
            },
        ),
        insurance: operator_money(145_000, "insurance"),
        fixed_other: operator_money(0, "fixed_other"),
        management_rate: operator_bps(800, "management_rate"),
        maintenance_rate: operator_bps(500, "maintenance_rate"),
        reserve_rate: operator_bps(500, "capex_reserve_rate"),
        purchase_price: operator_money(21_500_000, "purchase_price"),
        closing_costs: operator_money(645_000, "closing_costs"),
        rehab_budget: operator_money(1_200_000, "rehab_budget"),
        down_payment_rate: operator_bps(2_500, "down_payment_rate"),
        interest_rate: operator_bps(725, "interest_rate"),
        term_months: Traced::leaf(
            360_u32,
            "Term",
            Origin::UserInput {
                field: "term_months".into(),
            },
        ),
    };

    (inputs, traced)
}

#[test]
fn section8_sfr_matches_hand_verified_fixture_to_the_cent() {
    let (inputs, traced) = section8_sfr();
    let uw = underwrite(&inputs, &traced).unwrap();

    assert_eq!(uw.gross_potential_rent.value, usd(2_220_000), "GPR");
    assert_eq!(uw.vacancy_loss.value, usd(155_400), "vacancy");
    assert_eq!(uw.effective_gross_income.value, usd(2_064_600), "EGI");
    assert_eq!(uw.operating_expenses.value, usd(836_628), "OpEx");
    assert_eq!(uw.net_operating_income.value, usd(1_227_972), "NOI");
    assert_eq!(uw.loan_amount.value, usd(16_125_000), "loan");
    assert_eq!(uw.annual_debt_service.value, usd(1_320_012), "ADS");
    assert_eq!(uw.cash_flow_before_tax.value, usd(-92_040), "CFBT");
    assert_eq!(
        uw.total_cash_invested.value,
        usd(7_220_000),
        "cash invested"
    );

    assert_eq!(uw.cap_rate.value, Bps::from_raw(571), "cap rate");
    assert_eq!(uw.dscr.value, Bps::from_raw(9_303), "DSCR");
    assert_eq!(uw.cash_on_cash.value, Bps::from_raw(-127), "CoC");
    assert_eq!(
        uw.expense_ratio.value,
        Bps::from_raw(4_052),
        "expense ratio"
    );
    assert_eq!(uw.gross_rent_multiplier.value, Bps::from_raw(96_847), "GRM");
    assert_eq!(
        uw.break_even_occupancy.value,
        Bps::from_raw(9_715),
        "break-even"
    );
}

/// The fixture deal does not cover its debt. The engine must say so plainly
/// rather than rounding a 0.93x coverage into something that reads as fine.
#[test]
fn a_deal_that_fails_coverage_reports_negative_cash_flow() {
    let (inputs, traced) = section8_sfr();
    let uw = underwrite(&inputs, &traced).unwrap();
    assert!(uw.cash_flow_before_tax.value.is_negative());
    assert!(uw.dscr.value < Bps::ONE, "DSCR below 1.00x");
    assert!(uw.cash_on_cash.value.raw() < 0);
    assert!(uw.break_even_occupancy.value < Bps::ONE);
}

#[test]
fn the_engine_is_deterministic() {
    let (inputs, traced) = section8_sfr();
    let a = underwrite(&inputs, &traced).unwrap();
    let b = underwrite(&inputs, &traced).unwrap();
    assert_eq!(
        a, b,
        "identical inputs must produce identical output and provenance"
    );
}

#[test]
fn every_output_traces_back_to_its_external_sources() {
    let (inputs, traced) = section8_sfr();
    let uw = underwrite(&inputs, &traced).unwrap();

    // NOI depends on both the HUD rent and the assessor tax roll.
    assert_eq!(
        uw.net_operating_income.trace.external_sources(),
        vec!["county.assessor", "hud.fmr"]
    );
    // Cap rate inherits NOI's sources.
    assert_eq!(
        uw.cap_rate.trace.external_sources(),
        vec!["county.assessor", "hud.fmr"]
    );
    // Debt service is financed on operator terms and touches no external source.
    assert!(uw.annual_debt_service.trace.external_sources().is_empty());
}

#[test]
fn no_computed_number_is_a_bare_leaf() {
    let (inputs, traced) = section8_sfr();
    let uw = underwrite(&inputs, &traced).unwrap();
    for (name, trace) in [
        ("NOI", &uw.net_operating_income.trace),
        ("cap_rate", &uw.cap_rate.trace),
        ("dscr", &uw.dscr.trace),
        ("coc", &uw.cash_on_cash.trace),
        ("cfbt", &uw.cash_flow_before_tax.trace),
        ("break_even", &uw.break_even_occupancy.trace),
    ] {
        assert!(trace.depth() > 1, "{name} rendered without a derivation");
    }
}

#[test]
fn provenance_renders_a_readable_derivation() {
    let (inputs, traced) = section8_sfr();
    let uw = underwrite(&inputs, &traced).unwrap();
    let rendered = uw.net_operating_income.trace.render();
    assert!(rendered.starts_with("Net operating income  = noi\n"));
    assert!(rendered.contains("HUD FMR 3BR  <- hud.fmr[authoritative]"));
    assert!(rendered.contains("Vacancy  <- assume:vacancy_rate"));
}

#[test]
fn an_all_cash_deal_reports_undefined_coverage_rather_than_zero() {
    let (mut inputs, mut traced) = section8_sfr();
    inputs.deal.down_payment_rate = Bps::ONE;
    traced.down_payment_rate = operator_bps(Bps::SCALE, "down_payment_rate");
    let uw = underwrite(&inputs, &traced).unwrap();

    assert_eq!(uw.loan_amount.value, usd(0));
    assert_eq!(uw.annual_debt_service.value, usd(0));
    assert_eq!(
        uw.dscr.value,
        Bps::from_raw(i64::MAX),
        "no debt -> undefined, not 0"
    );
    // With no debt the same property is cash-flow positive.
    assert!(uw.cash_flow_before_tax.value.is_positive());
}

#[test]
fn a_zero_price_is_rejected_rather_than_dividing_by_zero() {
    let (mut inputs, traced) = section8_sfr();
    inputs.deal.purchase_price = usd(0);
    assert_eq!(
        underwrite(&inputs, &traced),
        Err(EngineError::InvalidInput {
            field: "purchase_price",
            detail: "must be greater than zero"
        })
    );
}

#[test]
fn full_vacancy_is_rejected_as_out_of_model() {
    let (mut inputs, traced) = section8_sfr();
    inputs.income.vacancy_rate = Bps::ONE;
    assert!(matches!(
        underwrite(&inputs, &traced),
        Err(EngineError::InvalidInput {
            field: "vacancy_rate",
            ..
        })
    ));
}

/// The guard is `raw() < 0 || >= ONE`. Only the upper bound was tested, so
/// `cargo mutants` turned `< 0` into `== 0` and `<= 0` on 2026-08-02 and both
/// survived. The lower bound needs two cases: a negative rate must be rejected,
/// and exactly zero must be accepted.
///
/// This matters beyond the lint. A negative vacancy rate makes the occupancy
/// complement exceed 100%, so effective gross income would exceed scheduled
/// rent — the model would report collecting more than it billed.
#[test]
fn a_negative_vacancy_rate_is_rejected() {
    let (mut inputs, traced) = section8_sfr();
    inputs.income.vacancy_rate = Bps::from_raw(-1);
    assert!(matches!(
        underwrite(&inputs, &traced),
        Err(EngineError::InvalidInput {
            field: "vacancy_rate",
            ..
        })
    ));
}

/// Zero vacancy is unrealistic but in-model, and it is the boundary that
/// separates `< 0` from `<= 0`.
#[test]
fn a_zero_vacancy_rate_is_accepted() {
    let (mut inputs, traced) = section8_sfr();
    inputs.income.vacancy_rate = Bps::ZERO;
    assert!(
        underwrite(&inputs, &traced).is_ok(),
        "zero vacancy is in-model"
    );
}

#[test]
fn mixing_currencies_is_an_error_not_a_silent_conversion() {
    let (mut inputs, traced) = section8_sfr();
    inputs.expenses.annual_insurance = Money::from_minor(145_000, Currency::Eur);
    assert!(matches!(
        underwrite(&inputs, &traced),
        Err(EngineError::Money(_))
    ));
}

#[test]
fn the_accounting_identity_holds() {
    let (inputs, traced) = section8_sfr();
    let uw = underwrite(&inputs, &traced).unwrap();

    // EGI = GPR - vacancy + other
    let egi = uw
        .gross_potential_rent
        .value
        .sub(uw.vacancy_loss.value)
        .unwrap()
        .add(inputs.income.other_annual_income)
        .unwrap();
    assert_eq!(egi, uw.effective_gross_income.value);

    // NOI = EGI - OpEx
    assert_eq!(
        uw.effective_gross_income
            .value
            .sub(uw.operating_expenses.value)
            .unwrap(),
        uw.net_operating_income.value
    );

    // CFBT = NOI - ADS
    assert_eq!(
        uw.net_operating_income
            .value
            .sub(uw.annual_debt_service.value)
            .unwrap(),
        uw.cash_flow_before_tax.value
    );
}

mod properties {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// Raising the price can never raise the cap rate.
        #[test]
        fn cap_rate_is_monotonically_decreasing_in_price(
            price in 5_000_000_i64..500_000_000_i64,
            bump  in 100_000_i64..5_000_000_i64,
        ) {
            let (mut inputs, mut traced) = section8_sfr();
            inputs.deal.purchase_price = usd(price);
            traced.purchase_price = operator_money(price, "purchase_price");
            let low = underwrite(&inputs, &traced).unwrap();

            let higher = price.saturating_add(bump);
            inputs.deal.purchase_price = usd(higher);
            traced.purchase_price = operator_money(higher, "purchase_price");
            let high = underwrite(&inputs, &traced).unwrap();

            prop_assert!(high.cap_rate.value <= low.cap_rate.value);
        }

        /// More vacancy can never produce more NOI.
        #[test]
        fn noi_is_monotonically_decreasing_in_vacancy(
            vacancy in 0_i64..4_000_i64,
            bump    in 1_i64..2_000_i64,
        ) {
            let (mut inputs, mut traced) = section8_sfr();
            inputs.income.vacancy_rate = Bps::from_raw(vacancy);
            traced.vacancy_rate = operator_bps(vacancy, "vacancy_rate");
            let low = underwrite(&inputs, &traced).unwrap();

            let worse = vacancy.saturating_add(bump);
            inputs.income.vacancy_rate = Bps::from_raw(worse);
            traced.vacancy_rate = operator_bps(worse, "vacancy_rate");
            let high = underwrite(&inputs, &traced).unwrap();

            prop_assert!(high.net_operating_income.value.minor() <= low.net_operating_income.value.minor());
        }

        /// The engine never panics on any plausible input combination.
        #[test]
        fn arbitrary_plausible_inputs_never_panic(
            rent     in 0_i64..1_000_000_i64,
            price    in 1_i64..1_000_000_000_i64,
            vacancy  in 0_i64..9_999_i64,
            rate     in 0_i64..5_000_i64,
            term     in 1_u32..600_u32,
        ) {
            let (mut inputs, mut traced) = section8_sfr();
            inputs.income.monthly_scheduled_rent = usd(rent);
            inputs.deal.purchase_price = usd(price);
            inputs.income.vacancy_rate = Bps::from_raw(vacancy);
            inputs.deal.interest_rate = Bps::from_raw(rate);
            inputs.deal.term_months = term;
            traced.monthly_rent = operator_money(rent, "rent");
            traced.purchase_price = operator_money(price, "purchase_price");
            traced.vacancy_rate = operator_bps(vacancy, "vacancy_rate");
            traced.interest_rate = operator_bps(rate, "interest_rate");

            let _ = underwrite(&inputs, &traced);
        }
    }
}
