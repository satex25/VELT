//! The pure underwriting engine.
//!
//! Doctrine §5: *The computation engine is pure. No I/O. No clock. No
//! randomness. No ambient config. Deterministic in, deterministic out.* This
//! crate depends only on `velt-money` and `velt-provenance`; it cannot open a
//! socket, read a file, or ask what time it is.
//!
//! Doctrine §5: *Every computed number carries full provenance.* Every public
//! function here returns [`Traced`], so an untraced number is not something a
//! caller can obtain.
//!
//! # Model
//!
//! ```text
//! GPR   = scheduled rent x 12                  gross potential rent
//! EGI   = GPR x (1 - vacancy) + other income   effective gross income
//! OpEx  = taxes + insurance + mgmt + maint + reserves + utilities + hoa
//! NOI   = EGI - OpEx                           net operating income
//! ADS   = amortized payment x 12               annual debt service
//! CFBT  = NOI - ADS                            cash flow before tax
//! ```
//!
//! Management, maintenance, and reserves are expressed as rates against EGI
//! rather than as fixed amounts, because that is how they are quoted and how
//! they scale with the rent roll.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod amort;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use velt_money::{Bps, Currency, Money, MoneyError};
use velt_provenance::{Traced, derive};

/// Errors produced by the engine.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EngineError {
    /// Underlying money arithmetic failed.
    #[error(transparent)]
    Money(#[from] MoneyError),
    /// An input violated a modelling precondition.
    #[error("invalid input for {field}: {detail}")]
    InvalidInput {
        /// The offending field.
        field: &'static str,
        /// Why it is invalid.
        detail: &'static str,
    },
}

/// Result alias for engine operations.
pub type Result<T> = std::result::Result<T, EngineError>;

/// The rent roll and income assumptions for a property.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct IncomeInputs {
    /// Monthly scheduled rent across all units.
    ///
    /// For a voucher-eligible underwrite this is the HUD Fair Market Rent for
    /// the unit mix, which is why FMR is the headline metric: it is the input
    /// the entire income side hangs from.
    pub monthly_scheduled_rent: Money,
    /// Annual income from sources other than rent (laundry, parking, storage).
    pub other_annual_income: Money,
    /// Economic vacancy, as a rate. Includes physical vacancy plus credit loss.
    pub vacancy_rate: Bps,
}

/// The operating expense assumptions for a property.
///
/// Rate-based expenses are applied to effective gross income, not to gross
/// potential rent — you do not pay a management fee on rent you did not collect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ExpenseInputs {
    /// Annual property tax.
    pub annual_property_tax: Money,
    /// Annual hazard and liability insurance.
    pub annual_insurance: Money,
    /// Annual HOA or condo fees.
    pub annual_hoa: Money,
    /// Annual owner-paid utilities.
    pub annual_utilities: Money,
    /// Other fixed annual operating costs.
    pub annual_other_fixed: Money,
    /// Property management fee, as a rate on EGI.
    pub management_rate: Bps,
    /// Repairs and maintenance, as a rate on EGI.
    pub maintenance_rate: Bps,
    /// Capital reserves, as a rate on EGI.
    pub capex_reserve_rate: Bps,
}

/// Acquisition and financing terms.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct DealInputs {
    /// Contract purchase price.
    pub purchase_price: Money,
    /// Closing costs paid at acquisition.
    pub closing_costs: Money,
    /// Rehab or capital improvement budget funded at acquisition.
    pub rehab_budget: Money,
    /// Down payment, as a rate on purchase price.
    pub down_payment_rate: Bps,
    /// Annual nominal interest rate.
    pub interest_rate: Bps,
    /// Amortization term in months.
    pub term_months: u32,
}

/// The full input set for an underwrite.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct UnderwriteInputs {
    /// Income side.
    pub income: IncomeInputs,
    /// Expense side.
    pub expenses: ExpenseInputs,
    /// Acquisition and debt.
    pub deal: DealInputs,
}

impl UnderwriteInputs {
    /// The currency this underwrite is denominated in.
    #[must_use]
    pub const fn currency(&self) -> Currency {
        self.deal.purchase_price.currency()
    }
}

/// A completed underwrite. Every field carries its derivation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Underwrite {
    /// Gross potential rent (annual).
    pub gross_potential_rent: Traced<Money>,
    /// Vacancy and credit loss (annual).
    pub vacancy_loss: Traced<Money>,
    /// Effective gross income (annual).
    pub effective_gross_income: Traced<Money>,
    /// Total operating expenses (annual).
    pub operating_expenses: Traced<Money>,
    /// Net operating income (annual).
    pub net_operating_income: Traced<Money>,
    /// Annual debt service.
    pub annual_debt_service: Traced<Money>,
    /// Cash flow before tax (annual).
    pub cash_flow_before_tax: Traced<Money>,
    /// Total cash required to close.
    pub total_cash_invested: Traced<Money>,
    /// Loan principal.
    pub loan_amount: Traced<Money>,
    /// Going-in capitalization rate: NOI / purchase price.
    pub cap_rate: Traced<Bps>,
    /// Debt service coverage ratio, in basis points (12_500 == 1.25x).
    pub dscr: Traced<Bps>,
    /// Cash-on-cash return: CFBT / total cash invested.
    pub cash_on_cash: Traced<Bps>,
    /// Operating expense ratio: OpEx / EGI.
    pub expense_ratio: Traced<Bps>,
    /// Gross rent multiplier, in basis points (price / GPR).
    pub gross_rent_multiplier: Traced<Bps>,
    /// Break-even occupancy: the occupancy at which CFBT reaches zero.
    pub break_even_occupancy: Traced<Bps>,
}

/// Run a full underwrite.
///
/// Pure: the same inputs always produce the same outputs, including the same
/// provenance tree.
///
/// # Errors
/// [`EngineError::InvalidInput`] if an input violates a modelling precondition,
/// or [`EngineError::Money`] on overflow or currency mismatch.
pub fn underwrite(inputs: &UnderwriteInputs, traced: &TracedInputs) -> Result<Underwrite> {
    let currency = inputs.currency();

    if !inputs.deal.purchase_price.is_positive() {
        return Err(EngineError::InvalidInput {
            field: "purchase_price",
            detail: "must be greater than zero",
        });
    }
    if inputs.income.vacancy_rate.raw() < 0 || inputs.income.vacancy_rate >= Bps::ONE {
        return Err(EngineError::InvalidInput {
            field: "vacancy_rate",
            detail: "must be in [0%, 100%)",
        });
    }

    // --- Income -----------------------------------------------------------
    let gpr_value = inputs
        .income
        .monthly_scheduled_rent
        .mul_int(amort::MONTHS_PER_YEAR)?;
    let gross_potential_rent = derive(
        "Gross potential rent",
        "gpr",
        &[&traced.monthly_rent.trace],
        gpr_value,
    );

    let vacancy_value = gpr_value.apply(inputs.income.vacancy_rate)?;
    let vacancy_loss = derive(
        "Vacancy & credit loss",
        "vacancy_loss",
        &[&gross_potential_rent.trace, &traced.vacancy_rate.trace],
        vacancy_value,
    );

    let egi_value = gpr_value
        .sub(vacancy_value)?
        .add(inputs.income.other_annual_income)?;
    let effective_gross_income = derive(
        "Effective gross income",
        "egi",
        &[
            &gross_potential_rent.trace,
            &vacancy_loss.trace,
            &traced.other_income.trace,
        ],
        egi_value,
    );

    // --- Expenses ---------------------------------------------------------
    // Rate-based expenses bill against collected income, not scheduled income.
    let management = egi_value.apply(inputs.expenses.management_rate)?;
    let maintenance = egi_value.apply(inputs.expenses.maintenance_rate)?;
    let reserves = egi_value.apply(inputs.expenses.capex_reserve_rate)?;

    let opex_value = Money::sum(
        [
            inputs.expenses.annual_property_tax,
            inputs.expenses.annual_insurance,
            inputs.expenses.annual_hoa,
            inputs.expenses.annual_utilities,
            inputs.expenses.annual_other_fixed,
            management,
            maintenance,
            reserves,
        ],
        currency,
    )?;
    let operating_expenses = derive(
        "Operating expenses",
        "opex",
        &[
            &traced.property_tax.trace,
            &traced.insurance.trace,
            &traced.fixed_other.trace,
            &effective_gross_income.trace,
            &traced.management_rate.trace,
            &traced.maintenance_rate.trace,
            &traced.reserve_rate.trace,
        ],
        opex_value,
    );

    // --- NOI --------------------------------------------------------------
    let noi_value = egi_value.sub(opex_value)?;
    let net_operating_income = derive(
        "Net operating income",
        "noi",
        &[&effective_gross_income.trace, &operating_expenses.trace],
        noi_value,
    );

    // --- Debt -------------------------------------------------------------
    let loan_value =
        amort::principal_from_ltv(inputs.deal.purchase_price, inputs.deal.down_payment_rate)?;
    let loan_amount = derive(
        "Loan amount",
        "loan",
        &[
            &traced.purchase_price.trace,
            &traced.down_payment_rate.trace,
        ],
        loan_value,
    );

    let ads_value = amort::annual_debt_service(
        loan_value,
        inputs.deal.interest_rate,
        inputs.deal.term_months,
    )?;
    let annual_debt_service = derive(
        "Annual debt service",
        "ads",
        &[
            &loan_amount.trace,
            &traced.interest_rate.trace,
            &traced.term_months.trace,
        ],
        ads_value,
    );

    let cfbt_value = noi_value.sub(ads_value)?;
    let cash_flow_before_tax = derive(
        "Cash flow before tax",
        "cfbt",
        &[&net_operating_income.trace, &annual_debt_service.trace],
        cfbt_value,
    );

    // --- Basis ------------------------------------------------------------
    let down_payment = inputs
        .deal
        .purchase_price
        .apply(inputs.deal.down_payment_rate)?;
    let cash_value = Money::sum(
        [
            down_payment,
            inputs.deal.closing_costs,
            inputs.deal.rehab_budget,
        ],
        currency,
    )?;
    let total_cash_invested = derive(
        "Total cash invested",
        "cash_invested",
        &[
            &traced.purchase_price.trace,
            &traced.down_payment_rate.trace,
            &traced.closing_costs.trace,
            &traced.rehab_budget.trace,
        ],
        cash_value,
    );

    // --- Ratios -----------------------------------------------------------
    let cap_value = noi_value.ratio_to(inputs.deal.purchase_price)?;
    let cap_rate = derive(
        "Cap rate",
        "cap_rate",
        &[&net_operating_income.trace, &traced.purchase_price.trace],
        cap_value,
    );

    let dscr_value = if ads_value.is_positive() {
        noi_value.ratio_to(ads_value)?
    } else {
        // No debt means coverage is undefined rather than infinite. Zero here
        // would read as "fails coverage", which is the opposite of the truth,
        // so an all-cash deal reports the sentinel below and the terminal
        // renders it as "n/a".
        Bps::from_raw(i64::MAX)
    };
    let dscr = derive(
        "DSCR",
        "dscr",
        &[&net_operating_income.trace, &annual_debt_service.trace],
        dscr_value,
    );

    let coc_value = if cash_value.is_positive() {
        cfbt_value.ratio_to(cash_value)?
    } else {
        return Err(EngineError::InvalidInput {
            field: "total_cash_invested",
            detail: "must be greater than zero to compute cash-on-cash",
        });
    };
    let cash_on_cash = derive(
        "Cash-on-cash",
        "coc",
        &[&cash_flow_before_tax.trace, &total_cash_invested.trace],
        coc_value,
    );

    let expense_ratio_value = if egi_value.is_positive() {
        opex_value.ratio_to(egi_value)?
    } else {
        Bps::ZERO
    };
    let expense_ratio = derive(
        "Expense ratio",
        "expense_ratio",
        &[&operating_expenses.trace, &effective_gross_income.trace],
        expense_ratio_value,
    );

    let grm_value = if gpr_value.is_positive() {
        inputs.deal.purchase_price.ratio_to(gpr_value)?
    } else {
        Bps::ZERO
    };
    let gross_rent_multiplier = derive(
        "Gross rent multiplier",
        "grm",
        &[&traced.purchase_price.trace, &gross_potential_rent.trace],
        grm_value,
    );

    // Break-even occupancy = (OpEx + ADS) / GPR. Below this collection rate the
    // deal burns cash; it is the single number that tells an operator how much
    // room the rent roll has before the deal stops paying for itself.
    let breakeven_value = if gpr_value.is_positive() {
        opex_value.add(ads_value)?.ratio_to(gpr_value)?
    } else {
        Bps::ZERO
    };
    let break_even_occupancy = derive(
        "Break-even occupancy",
        "break_even",
        &[
            &operating_expenses.trace,
            &annual_debt_service.trace,
            &gross_potential_rent.trace,
        ],
        breakeven_value,
    );

    Ok(Underwrite {
        gross_potential_rent,
        vacancy_loss,
        effective_gross_income,
        operating_expenses,
        net_operating_income,
        annual_debt_service,
        cash_flow_before_tax,
        total_cash_invested,
        loan_amount,
        cap_rate,
        dscr,
        cash_on_cash,
        expense_ratio,
        gross_rent_multiplier,
        break_even_occupancy,
    })
}

/// The traced leaves for an underwrite.
///
/// Inputs arrive already traced — from a [`velt_connector::Datum`] for external
/// data or from operator entry — so the engine never invents an origin. It is a
/// separate struct from [`UnderwriteInputs`] so that the numeric model and the
/// provenance plumbing can be read independently.
#[derive(Debug, Clone)]
pub struct TracedInputs {
    /// Monthly scheduled rent (typically HUD FMR).
    pub monthly_rent: Traced<Money>,
    /// Other annual income.
    pub other_income: Traced<Money>,
    /// Vacancy rate.
    pub vacancy_rate: Traced<Bps>,
    /// Annual property tax.
    pub property_tax: Traced<Money>,
    /// Annual insurance.
    pub insurance: Traced<Money>,
    /// Aggregated fixed other expenses (HOA, utilities, other).
    pub fixed_other: Traced<Money>,
    /// Management rate.
    pub management_rate: Traced<Bps>,
    /// Maintenance rate.
    pub maintenance_rate: Traced<Bps>,
    /// Capital reserve rate.
    pub reserve_rate: Traced<Bps>,
    /// Purchase price.
    pub purchase_price: Traced<Money>,
    /// Closing costs.
    pub closing_costs: Traced<Money>,
    /// Rehab budget.
    pub rehab_budget: Traced<Money>,
    /// Down payment rate.
    pub down_payment_rate: Traced<Bps>,
    /// Interest rate.
    pub interest_rate: Traced<Bps>,
    /// Term in months.
    pub term_months: Traced<u32>,
}

#[cfg(test)]
mod tests;
