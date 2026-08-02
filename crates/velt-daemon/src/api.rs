//! HTTP surface. Rust is the single source of truth for this contract.
//!
//! Doctrine §7: *Rust is the single source of truth. utoipa-generated OpenAPI ->
//! TypeScript client, drift-checked in CI.* Every route and every schema below
//! is annotated so `just openapi` can emit `openapi.json`, and CI fails the
//! build if the checked-in TypeScript client no longer matches.

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::{OpenApi, ToSchema};
use velt_engine::{underwrite, EngineError, TracedInputs, Underwrite, UnderwriteInputs};
use velt_money::{Bps, Currency, Money};
use velt_provenance::{Origin, Traced};

/// Shared daemon state.
#[derive(Debug, Clone)]
pub struct AppState {
    /// Version reported by `/health`, stamped onto every snapshot.
    pub engine_version: &'static str,
}

/// Liveness and version response.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct Health {
    /// Always `"ok"` when the daemon is serving.
    pub status: &'static str,
    /// Engine version, used by the shell to detect a stale sidecar.
    pub engine_version: String,
}

/// Request body for an underwrite.
#[derive(Debug, Deserialize, ToSchema)]
pub struct UnderwriteRequest {
    /// The full input set.
    pub inputs: UnderwriteInputs,
}

/// Error body returned for a rejected underwrite.
#[derive(Debug, Serialize, ToSchema)]
pub struct ApiError {
    /// Machine-readable error code.
    pub code: &'static str,
    /// Human-readable detail.
    pub detail: String,
}

/// Liveness probe.
#[utoipa::path(
    get,
    path = "/health",
    responses((status = 200, description = "Daemon is serving", body = Health))
)]
pub async fn health(State(state): State<Arc<AppState>>) -> Json<Health> {
    Json(Health {
        status: "ok",
        engine_version: state.engine_version.to_owned(),
    })
}

/// Run an underwrite.
#[utoipa::path(
    post,
    path = "/underwrite",
    request_body = UnderwriteRequest,
    responses(
        (status = 200, description = "Underwrite complete", body = Underwrite),
        (status = 422, description = "Input rejected by the engine", body = ApiError)
    )
)]
pub async fn post_underwrite(Json(req): Json<UnderwriteRequest>) -> impl IntoResponse {
    // Inputs arriving over HTTP are operator entries; external data reaches the
    // engine through a connector, which stamps its own origin.
    let traced = operator_traced(&req.inputs);
    match underwrite(&req.inputs, &traced) {
        Ok(result) => (StatusCode::OK, Json(result)).into_response(),
        Err(err) => {
            let code = match err {
                EngineError::InvalidInput { field, .. } => field,
                EngineError::Money(_) => "arithmetic",
            };
            (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(ApiError {
                    code,
                    detail: err.to_string(),
                }),
            )
                .into_response()
        }
    }
}

fn money_leaf(value: Money, field: &'static str) -> Traced<Money> {
    Traced::leaf(
        value,
        field,
        Origin::UserInput {
            field: field.to_owned(),
        },
    )
}

fn bps_leaf(value: Bps, field: &'static str) -> Traced<Bps> {
    Traced::leaf(
        value,
        field,
        Origin::UserInput {
            field: field.to_owned(),
        },
    )
}

fn operator_traced(inputs: &UnderwriteInputs) -> TracedInputs {
    let currency: Currency = inputs.currency();
    let fixed_other = Money::sum(
        [
            inputs.expenses.annual_hoa,
            inputs.expenses.annual_utilities,
            inputs.expenses.annual_other_fixed,
        ],
        currency,
    )
    .unwrap_or_else(|_| Money::zero(currency));

    TracedInputs {
        monthly_rent: money_leaf(
            inputs.income.monthly_scheduled_rent,
            "monthly_scheduled_rent",
        ),
        other_income: money_leaf(inputs.income.other_annual_income, "other_annual_income"),
        vacancy_rate: bps_leaf(inputs.income.vacancy_rate, "vacancy_rate"),
        property_tax: money_leaf(inputs.expenses.annual_property_tax, "annual_property_tax"),
        insurance: money_leaf(inputs.expenses.annual_insurance, "annual_insurance"),
        fixed_other: money_leaf(fixed_other, "fixed_other"),
        management_rate: bps_leaf(inputs.expenses.management_rate, "management_rate"),
        maintenance_rate: bps_leaf(inputs.expenses.maintenance_rate, "maintenance_rate"),
        reserve_rate: bps_leaf(inputs.expenses.capex_reserve_rate, "capex_reserve_rate"),
        purchase_price: money_leaf(inputs.deal.purchase_price, "purchase_price"),
        closing_costs: money_leaf(inputs.deal.closing_costs, "closing_costs"),
        rehab_budget: money_leaf(inputs.deal.rehab_budget, "rehab_budget"),
        down_payment_rate: bps_leaf(inputs.deal.down_payment_rate, "down_payment_rate"),
        interest_rate: bps_leaf(inputs.deal.interest_rate, "interest_rate"),
        term_months: Traced::leaf(
            inputs.deal.term_months,
            "term_months",
            Origin::UserInput {
                field: "term_months".to_owned(),
            },
        ),
    }
}

/// The generated OpenAPI document.
#[derive(OpenApi)]
#[openapi(
    paths(health, post_underwrite),
    components(schemas(Health, UnderwriteRequest, ApiError)),
    info(
        title = "VELT daemon",
        version = "0.1.0",
        description = "Local-first underwriting API. Loopback only."
    )
)]
pub struct ApiDoc;

/// Build the router.
pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/underwrite", post(post_underwrite))
        .with_state(state)
}
