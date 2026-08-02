//! The connector boundary: the only place external data enters VELT.
//!
//! Two doctrine constraints are enforced here as code rather than as policy.
//!
//! §5 — *Every external datum carries source, trust tier, fetch timestamp, and
//! confidence.* [`Datum<T>`] has no constructor that omits them.
//!
//! §5 — *Fair Housing: no demographic neighborhood scoring, no proxy metrics
//! derived from protected characteristics. If a data source ships such a field,
//! drop it at the connector boundary.* [`FairHousingFilter`] is that drop, and
//! [`Connector::ingest`] runs it on every record before the record is
//! representable as a [`Datum`].

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use utoipa::ToSchema;
use velt_provenance::{Origin, Traced};

/// Errors raised at the connector boundary.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConnectorError {
    /// A record contained a field forbidden by the Fair Housing constraint.
    ///
    /// This is a hard error, not a warning: a source that ships demographic
    /// scoring must be handled explicitly by adding the field to the connector's
    /// declared drop list, so that the decision is visible in a diff.
    #[error("fair housing violation: source '{source_id}' shipped forbidden field '{field}' ({reason}); declare it in the connector's drop list or remove the source")]
    FairHousing {
        /// Connector that produced the record.
        source_id: &'static str,
        /// The forbidden field name.
        field: String,
        /// Why the field is forbidden.
        reason: &'static str,
    },
    /// The source returned a value the connector could not parse.
    #[error("source '{source_id}' returned unusable value for '{field}': {detail}")]
    Unusable {
        /// Connector that produced the record.
        source_id: &'static str,
        /// Field being parsed.
        field: String,
        /// Parse detail.
        detail: String,
    },
    /// The connector has no documented rights posture.
    ///
    /// Doctrine §5: *Data rights are checked before scraping, not after.* A
    /// connector without a [`RightsPosture`] cannot be constructed.
    #[error(
        "connector '{source_id}' has no documented rights posture; doctrine §5 forbids fetching"
    )]
    NoRightsPosture {
        /// Connector identifier.
        source_id: &'static str,
    },
}

/// Result alias for connector operations.
pub type Result<T> = std::result::Result<T, ConnectorError>;

/// How much a datum from this source can be relied on.
///
/// Trust tier is a property of the *source*, not of the value, and it travels
/// with the value all the way to the terminal so the operator can see whether a
/// cap rate rests on a published HUD schedule or on a scraped listing blurb.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum TrustTier {
    /// Published by the authority that defines the number (HUD FMR schedules,
    /// county assessor rolls, recorded deeds).
    Authoritative,
    /// A licensed commercial feed under contract.
    Licensed,
    /// Self-reported by a market participant (listing agent remarks, owner pro
    /// formas). Directionally useful, frequently optimistic.
    Reported,
    /// Derived by VELT from other data (a comp-based rent estimate).
    Derived,
    /// Entered by the operator.
    Operator,
}

impl TrustTier {
    /// Stable lowercase identifier used in provenance and the UI.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Authoritative => "authoritative",
            Self::Licensed => "licensed",
            Self::Reported => "reported",
            Self::Derived => "derived",
            Self::Operator => "operator",
        }
    }

    /// Whether a value at this tier may be used without an explicit operator
    /// acknowledgement. `Reported` data can move a deal decision, so the
    /// terminal marks it and requires acknowledgement before it feeds a
    /// committed underwrite.
    #[must_use]
    pub const fn is_unattested(self) -> bool {
        matches!(self, Self::Reported)
    }
}

/// Confidence in a specific value, in percent, independent of source tier.
///
/// A HUD schedule is `Authoritative` but a *stale* HUD schedule is authoritative
/// with low confidence. Tier answers "who said it"; confidence answers "how much
/// should this particular number move a decision".
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, ToSchema,
)]
pub struct Confidence(u8);

impl Confidence {
    /// Full confidence.
    pub const FULL: Self = Self(100);

    /// Construct, clamping to 0..=100.
    #[must_use]
    pub const fn new(pct: u8) -> Self {
        Self(if pct > 100 { 100 } else { pct })
    }

    /// Confidence as a percentage.
    #[must_use]
    pub const fn percent(self) -> u8 {
        self.0
    }
}

/// The rights posture for a data source, written before any connector code.
///
/// Doctrine §5 makes this mandatory: a [`Connector`] cannot be implemented
/// without returning one, so "we'll document the rights later" is not a state
/// the type system permits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct RightsPosture {
    /// URL of the terms of service reviewed.
    pub terms_url: String,
    /// Date the terms were reviewed, RFC-3339.
    pub reviewed_at: String,
    /// Whether robots.txt permits the access pattern used.
    pub robots_permits: bool,
    /// Requests per minute the connector will not exceed.
    pub rate_limit_per_min: u32,
    /// User-Agent string identifying VELT honestly.
    pub user_agent: String,
    /// Whether the source is subject to MLS/IDX redistribution rules.
    pub mls_idx_governed: bool,
    /// Plain-language summary of what this source permits and forbids.
    pub summary: String,
}

/// A single external value with its full boundary metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Datum<T> {
    /// The value.
    pub value: T,
    /// Stable connector identifier, e.g. `hud.fmr`.
    pub source_id: String,
    /// Trust tier of the source.
    pub trust_tier: TrustTier,
    /// RFC-3339 timestamp captured at fetch time.
    pub fetched_at: String,
    /// Confidence in this specific value.
    pub confidence: Confidence,
}

impl<T> Datum<T> {
    /// Construct a datum. All four metadata fields are required by signature;
    /// there is no partial constructor (doctrine §5).
    #[must_use]
    pub fn new(
        value: T,
        source_id: impl Into<String>,
        trust_tier: TrustTier,
        fetched_at: impl Into<String>,
        confidence: Confidence,
    ) -> Self {
        Self {
            value,
            source_id: source_id.into(),
            trust_tier,
            fetched_at: fetched_at.into(),
            confidence,
        }
    }

    /// Convert into an engine input, carrying the boundary metadata into the
    /// provenance tree. This is the only supported way for external data to
    /// reach the engine.
    #[must_use]
    pub fn into_traced(self, label: impl Into<String>) -> Traced<T> {
        Traced::leaf(
            self.value,
            label,
            Origin::External {
                source_id: self.source_id,
                trust_tier: self.trust_tier.as_str().to_owned(),
                fetched_at: self.fetched_at,
            },
        )
    }
}

/// Field names that may never cross the connector boundary, with the reason.
///
/// Doctrine §5 forbids demographic neighborhood scoring and any proxy metric
/// derived from a protected characteristic. This list is matched
/// case-insensitively as a substring against every incoming field name, so
/// `neighborhood_crime_index_2025` is caught by the `crime` entry.
pub const FORBIDDEN_FIELD_MARKERS: &[(&str, &str)] = &[
    ("race", "protected characteristic"),
    ("ethnic", "protected characteristic"),
    ("religio", "protected characteristic"),
    ("national_origin", "protected characteristic"),
    ("nationality", "protected characteristic"),
    ("familial_status", "protected characteristic"),
    ("disabilit", "protected characteristic"),
    ("handicap", "protected characteristic"),
    ("sex_", "protected characteristic"),
    ("gender", "protected characteristic"),
    (
        "crime",
        "correlates with protected characteristics; doctrine §5 bans crime indices",
    ),
    ("safety_score", "crime proxy"),
    ("school_rating", "banned as a neighborhood-quality proxy"),
    ("school_score", "banned as a neighborhood-quality proxy"),
    ("school_grade", "banned as a neighborhood-quality proxy"),
    (
        "neighborhood_score",
        "composite neighborhood quality metric",
    ),
    (
        "neighborhood_grade",
        "composite neighborhood quality metric",
    ),
    ("area_quality", "composite neighborhood quality metric"),
    ("area_score", "composite neighborhood quality metric"),
    ("desirability", "composite neighborhood quality metric"),
    ("demographic", "demographic inference"),
    ("median_age", "demographic inference"),
    ("household_composition", "familial status proxy"),
    ("language_spoken", "national origin proxy"),
    (
        "median_household_income",
        "income-by-area is a redlining proxy at ZIP granularity",
    ),
];

/// Rejects records containing fields forbidden by the Fair Housing constraint.
///
/// The filter is deliberately conservative and errors rather than silently
/// stripping: a source that starts shipping a crime index should stop the
/// pipeline and force an explicit decision, not quietly lose a column.
#[derive(Debug, Clone)]
pub struct FairHousingFilter {
    source_id: &'static str,
    /// Fields the connector explicitly acknowledges and drops.
    declared_drops: Vec<String>,
}

impl FairHousingFilter {
    /// Construct a filter for a connector.
    #[must_use]
    pub fn new(source_id: &'static str) -> Self {
        Self {
            source_id,
            declared_drops: Vec::new(),
        }
    }

    /// Declare a field the source ships that VELT drops on purpose.
    ///
    /// Declaring a drop is how a connector says "we know this source emits a
    /// school rating and we discard it" — visible in the diff, reviewable.
    #[must_use]
    pub fn declaring_drop(mut self, field: impl Into<String>) -> Self {
        self.declared_drops.push(field.into().to_lowercase());
        self
    }

    /// Check a single field name.
    ///
    /// # Errors
    /// [`ConnectorError::FairHousing`] if the field matches a forbidden marker
    /// and has not been explicitly declared as a drop.
    pub fn check_field(&self, field: &str) -> Result<()> {
        let lowered = field.to_lowercase();
        if self.declared_drops.contains(&lowered) {
            return Ok(());
        }
        for (marker, reason) in FORBIDDEN_FIELD_MARKERS {
            if lowered.contains(marker) {
                return Err(ConnectorError::FairHousing {
                    source_id: self.source_id,
                    field: field.to_owned(),
                    reason,
                });
            }
        }
        Ok(())
    }

    /// Filter a raw record, removing declared drops and rejecting undeclared
    /// forbidden fields.
    ///
    /// # Errors
    /// [`ConnectorError::FairHousing`] on the first undeclared forbidden field.
    pub fn filter(&self, record: BTreeMap<String, String>) -> Result<BTreeMap<String, String>> {
        let mut out = BTreeMap::new();
        for (key, value) in record {
            let lowered = key.to_lowercase();
            if self.declared_drops.contains(&lowered) {
                continue;
            }
            self.check_field(&key)?;
            out.insert(key, value);
        }
        Ok(out)
    }
}

/// A source of external data.
///
/// Implementing this trait requires declaring a [`RightsPosture`] and a
/// [`TrustTier`]; `ingest` is provided and always runs the Fair Housing filter,
/// so no implementor can accidentally bypass it.
pub trait Connector {
    /// Stable identifier, e.g. `hud.fmr`.
    fn source_id(&self) -> &'static str;

    /// Trust tier of everything this connector emits.
    fn trust_tier(&self) -> TrustTier;

    /// The documented rights posture. Written before connector code exists.
    fn rights_posture(&self) -> &RightsPosture;

    /// Fields this connector knowingly discards.
    fn declared_drops(&self) -> &[&'static str] {
        &[]
    }

    /// Run a raw record through the Fair Housing boundary.
    ///
    /// # Errors
    /// [`ConnectorError::FairHousing`] if the record carries an undeclared
    /// forbidden field.
    fn ingest(&self, record: BTreeMap<String, String>) -> Result<BTreeMap<String, String>> {
        let mut filter = FairHousingFilter::new(self.source_id());
        for drop in self.declared_drops() {
            filter = filter.declaring_drop(*drop);
        }
        filter.filter(record)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    struct TestSource {
        posture: RightsPosture,
    }

    impl TestSource {
        fn new() -> Self {
            Self {
                posture: RightsPosture {
                    terms_url: "https://example.test/terms".into(),
                    reviewed_at: "2026-08-01T00:00:00Z".into(),
                    robots_permits: true,
                    rate_limit_per_min: 30,
                    user_agent: "VELT/0.1 (+contact)".into(),
                    mls_idx_governed: false,
                    summary: "Test fixture source.".into(),
                },
            }
        }
    }

    impl Connector for TestSource {
        fn source_id(&self) -> &'static str {
            "test.source"
        }
        fn trust_tier(&self) -> TrustTier {
            TrustTier::Reported
        }
        fn rights_posture(&self) -> &RightsPosture {
            &self.posture
        }
        fn declared_drops(&self) -> &[&'static str] {
            &["school_rating"]
        }
    }

    #[test]
    fn permitted_fields_pass_through() {
        let src = TestSource::new();
        let out = src
            .ingest(record(&[
                ("bedrooms", "3"),
                ("list_price", "249000"),
                ("sqft", "1450"),
            ]))
            .unwrap();
        assert_eq!(out.len(), 3);
    }

    #[test]
    fn a_declared_drop_is_silently_removed() {
        let src = TestSource::new();
        let out = src
            .ingest(record(&[("bedrooms", "3"), ("school_rating", "8")]))
            .unwrap();
        assert_eq!(out.len(), 1);
        assert!(!out.contains_key("school_rating"));
    }

    #[test]
    fn an_undeclared_crime_index_stops_the_pipeline() {
        let src = TestSource::new();
        let err = src
            .ingest(record(&[
                ("bedrooms", "3"),
                ("neighborhood_crime_index_2025", "42"),
            ]))
            .unwrap_err();
        assert!(matches!(err, ConnectorError::FairHousing { .. }));
    }

    #[test]
    fn every_forbidden_marker_is_actually_caught() {
        let filter = FairHousingFilter::new("test.source");
        for (marker, _) in FORBIDDEN_FIELD_MARKERS {
            let field = format!("listing_{marker}_value");
            assert!(
                filter.check_field(&field).is_err(),
                "marker `{marker}` did not reject field `{field}`"
            );
        }
    }

    #[test]
    fn matching_is_case_insensitive() {
        let filter = FairHousingFilter::new("test.source");
        assert!(filter.check_field("Neighborhood_Score").is_err());
        assert!(filter.check_field("MEDIAN_HOUSEHOLD_INCOME").is_err());
    }

    #[test]
    fn a_datum_carries_its_boundary_metadata_into_provenance() {
        let datum = Datum::new(
            185_000_i64,
            "hud.fmr",
            TrustTier::Authoritative,
            "2026-08-01T00:00:00Z",
            Confidence::FULL,
        );
        let traced = datum.into_traced("HUD FMR 2BR");
        assert_eq!(traced.trace.external_sources(), vec!["hud.fmr"]);
    }

    #[test]
    fn confidence_clamps_above_one_hundred() {
        assert_eq!(Confidence::new(250).percent(), 100);
    }
}
