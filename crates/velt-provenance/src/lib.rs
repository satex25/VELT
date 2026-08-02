//! Provenance tracing for every computed number in VELT.
//!
//! Doctrine §5: *Every computed number carries full provenance. If you cannot
//! trace a number to its inputs and their sources, it does not render.*
//!
//! The mechanism is [`Traced<T>`] — a value paired with the [`Trace`] tree that
//! produced it. Engine functions take `Traced` inputs and return `Traced`
//! outputs, so the tree is assembled as a side effect of doing the arithmetic
//! rather than as a separate bookkeeping pass that can drift from reality.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::sync::Arc;
use utoipa::ToSchema;

/// Serde adapters for `Arc<Trace>`.
///
/// Serde's blanket `Arc<T>` impl forces `'de: 'static`, which propagates into
/// the generic [`Traced<T>`] and makes borrowed deserialization impossible.
/// Round-tripping through `Trace` itself avoids the bound entirely; the wire
/// format is unchanged because `Arc` is transparent.
mod arc_trace {
    use super::Trace;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::sync::Arc;

    pub fn serialize<S: Serializer>(value: &Arc<Trace>, ser: S) -> Result<S::Ok, S::Error> {
        Trace::serialize(value, ser)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(de: D) -> Result<Arc<Trace>, D::Error> {
        Trace::deserialize(de).map(Arc::new)
    }
}

/// Serde adapters for `Vec<Arc<Trace>>`. See [`arc_trace`].
mod arc_trace_vec {
    use super::Trace;
    use serde::{Deserialize, Deserializer, Serializer};
    use std::sync::Arc;

    #[allow(clippy::ptr_arg)]
    pub fn serialize<S: Serializer>(value: &Vec<Arc<Trace>>, ser: S) -> Result<S::Ok, S::Error> {
        ser.collect_seq(value.iter().map(AsRef::<Trace>::as_ref))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(de: D) -> Result<Vec<Arc<Trace>>, D::Error> {
        Ok(Vec::<Trace>::deserialize(de)?
            .into_iter()
            .map(Arc::new)
            .collect())
    }
}

/// Where a leaf value entered the system.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Origin {
    /// Supplied directly by the operator in the terminal.
    UserInput {
        /// Field the operator typed into.
        field: String,
    },
    /// Retrieved from an external source through a connector.
    ///
    /// `source_id` is the connector's stable identifier and `fetched_at` is an
    /// RFC-3339 timestamp captured at the connector boundary — the engine never
    /// reads a clock (doctrine §5).
    External {
        /// Stable connector identifier, e.g. `hud.fmr`.
        source_id: String,
        /// Trust tier recorded by the connector, as a string to keep this crate
        /// dependency-free; the typed form lives in `velt-connector`.
        trust_tier: String,
        /// RFC-3339 timestamp of the fetch.
        fetched_at: String,
    },
    /// A constant fixed by the model itself (e.g. 12 months in a year).
    Constant {
        /// Human-readable name of the constant.
        name: String,
    },
    /// An assumption the operator has not overridden, carrying its default.
    Assumption {
        /// Name of the assumption, e.g. `vacancy_rate`.
        name: String,
        /// Why this default was chosen.
        rationale: String,
    },
}

/// The derivation tree for a computed value.
///
/// `Arc` is used for the child edges so that a widely-reused input (purchase
/// price appears in half the metrics) is stored once and shared, keeping a full
/// underwriting trace cheap enough to attach to every number without thought.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "node", rename_all = "snake_case")]
pub enum Trace {
    /// A value that entered the system from outside the engine.
    Leaf {
        /// Display label for this input.
        label: String,
        /// Where it came from.
        origin: Origin,
    },
    /// A value derived from other traced values.
    Derived {
        /// Display label for the computed value.
        label: String,
        /// Name of the operation, e.g. `noi`, `dscr`, `cap_rate`.
        ///
        /// `Cow` rather than `&'static str`: a borrowed `&'static str` field
        /// forces `'de: 'static` on the whole type and blocks borrowed
        /// deserialization. `Cow::Borrowed` on construction keeps this
        /// allocation-free on the hot path.
        op: Cow<'static, str>,
        /// The traces of the inputs, in argument order.
        ///
        /// `Arc` is transparent over the wire; the OpenAPI schema describes the
        /// shape the TypeScript client actually receives.
        #[schema(no_recursion, value_type = Vec<Trace>)]
        #[serde(with = "arc_trace_vec")]
        inputs: Vec<Arc<Trace>>,
    },
}

impl Trace {
    /// Construct a leaf node.
    #[must_use]
    pub fn leaf(label: impl Into<String>, origin: Origin) -> Arc<Self> {
        Arc::new(Self::Leaf {
            label: label.into(),
            origin,
        })
    }

    /// Construct a derived node.
    #[must_use]
    pub fn derived(
        label: impl Into<String>,
        op: &'static str,
        inputs: Vec<Arc<Self>>,
    ) -> Arc<Self> {
        Arc::new(Self::Derived {
            label: label.into(),
            op: Cow::Borrowed(op),
            inputs,
        })
    }

    /// The display label of this node.
    #[must_use]
    pub fn label(&self) -> &str {
        match self {
            Self::Leaf { label, .. } | Self::Derived { label, .. } => label,
        }
    }

    /// Total number of nodes in the tree, counting shared nodes once per edge.
    #[must_use]
    pub fn node_count(&self) -> usize {
        match self {
            Self::Leaf { .. } => 1,
            Self::Derived { inputs, .. } => {
                1_usize.saturating_add(inputs.iter().map(|i| i.node_count()).sum::<usize>())
            }
        }
    }

    /// Depth of the tree; a leaf has depth 1.
    #[must_use]
    pub fn depth(&self) -> usize {
        match self {
            Self::Leaf { .. } => 1,
            Self::Derived { inputs, .. } => {
                1_usize.saturating_add(inputs.iter().map(|i| i.depth()).max().unwrap_or(0))
            }
        }
    }

    /// Every distinct external `source_id` this value depends on.
    ///
    /// This is what the terminal renders in the provenance pane and what the
    /// data-rights audit consumes: a number cannot be shown without disclosing
    /// which external sources it rests on.
    #[must_use]
    pub fn external_sources(&self) -> Vec<&str> {
        let mut out = Vec::new();
        self.collect_sources(&mut out);
        out.sort_unstable();
        out.dedup();
        out
    }

    fn collect_sources<'a>(&'a self, out: &mut Vec<&'a str>) {
        match self {
            Self::Leaf {
                origin: Origin::External { source_id, .. },
                ..
            } => {
                out.push(source_id.as_str());
            }
            Self::Leaf { .. } => {}
            Self::Derived { inputs, .. } => {
                for input in inputs {
                    input.collect_sources(out);
                }
            }
        }
    }

    /// Render the tree as indented text for the terminal provenance pane.
    #[must_use]
    pub fn render(&self) -> String {
        let mut buf = String::new();
        self.render_into(&mut buf, 0);
        buf
    }

    fn render_into(&self, buf: &mut String, depth: usize) {
        for _ in 0..depth {
            buf.push_str("  ");
        }
        match self {
            Self::Leaf { label, origin } => {
                let src = match origin {
                    Origin::UserInput { field } => format!("input:{field}"),
                    Origin::External {
                        source_id,
                        trust_tier,
                        ..
                    } => {
                        format!("{source_id}[{trust_tier}]")
                    }
                    Origin::Constant { name } => format!("const:{name}"),
                    Origin::Assumption { name, .. } => format!("assume:{name}"),
                };
                buf.push_str(&format!("{label}  <- {src}\n"));
            }
            Self::Derived { label, op, inputs } => {
                buf.push_str(&format!("{label}  = {op}\n"));
                for input in inputs {
                    input.render_into(buf, depth.saturating_add(1));
                }
            }
        }
    }
}

/// A value carrying the trace that produced it.
///
/// The engine's public API is expressed entirely in `Traced<T>`, which makes
/// "compute a number without provenance" unrepresentable rather than merely
/// discouraged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(bound(
    serialize = "T: serde::Serialize",
    deserialize = "T: serde::de::DeserializeOwned"
))]
pub struct Traced<T> {
    /// The computed value.
    pub value: T,
    /// The derivation of the value.
    #[schema(no_recursion, value_type = Trace)]
    #[serde(with = "arc_trace")]
    pub trace: Arc<Trace>,
}

impl<T> Traced<T> {
    /// Pair a value with an existing trace.
    #[must_use]
    pub const fn new(value: T, trace: Arc<Trace>) -> Self {
        Self { value, trace }
    }

    /// Introduce a value as a leaf of the trace tree.
    #[must_use]
    pub fn leaf(value: T, label: impl Into<String>, origin: Origin) -> Self {
        Self {
            value,
            trace: Trace::leaf(label, origin),
        }
    }

    /// Introduce a model constant.
    #[must_use]
    pub fn constant(value: T, name: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            value,
            trace: Trace::leaf(name.clone(), Origin::Constant { name }),
        }
    }

    /// Map the value, recording a derivation node with this value as the sole input.
    #[must_use]
    pub fn map<U, F: FnOnce(T) -> U>(
        self,
        label: impl Into<String>,
        op: &'static str,
        f: F,
    ) -> Traced<U> {
        Traced {
            value: f(self.value),
            trace: Trace::derived(label, op, vec![self.trace]),
        }
    }

    /// Borrow the value.
    pub const fn value(&self) -> &T {
        &self.value
    }
}

/// Combine traced inputs into a derived result.
///
/// Used by the engine to record an n-ary operation in one step:
/// `derive("NOI", "noi", &[&egi.trace, &opex.trace], value)`.
#[must_use]
pub fn derive<T>(
    label: impl Into<String>,
    op: &'static str,
    inputs: &[&Arc<Trace>],
    value: T,
) -> Traced<T> {
    let inputs = inputs.iter().map(|t| Arc::clone(t)).collect();
    Traced {
        value,
        trace: Trace::derived(label, op, inputs),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hud_leaf() -> Traced<i64> {
        Traced::leaf(
            185_000,
            "HUD FMR 2BR",
            Origin::External {
                source_id: "hud.fmr".into(),
                trust_tier: "authoritative".into(),
                fetched_at: "2026-08-01T00:00:00Z".into(),
            },
        )
    }

    #[test]
    fn a_leaf_traces_to_its_origin() {
        let fmr = hud_leaf();
        assert_eq!(fmr.trace.external_sources(), vec!["hud.fmr"]);
        assert_eq!(fmr.trace.depth(), 1);
    }

    #[test]
    fn derived_values_accumulate_every_source() {
        let fmr = hud_leaf();
        let tax = Traced::leaf(
            42_000,
            "Property tax",
            Origin::External {
                source_id: "county.assessor".into(),
                trust_tier: "reported".into(),
                fetched_at: "2026-08-01T00:00:00Z".into(),
            },
        );
        let noi = derive("NOI", "noi", &[&fmr.trace, &tax.trace], 143_000_i64);
        assert_eq!(
            noi.trace.external_sources(),
            vec!["county.assessor", "hud.fmr"]
        );
        assert_eq!(noi.trace.depth(), 2);
        assert_eq!(noi.trace.node_count(), 3);
    }

    #[test]
    fn shared_inputs_are_stored_once_and_referenced_twice() {
        let price = Traced::leaf(
            25_000_000_i64,
            "Price",
            Origin::UserInput {
                field: "price".into(),
            },
        );
        let a = derive("A", "a", &[&price.trace], 1_i64);
        let b = derive("B", "b", &[&price.trace], 2_i64);
        let c = derive("C", "c", &[&a.trace, &b.trace], 3_i64);
        assert_eq!(Arc::strong_count(&price.trace), 3);
        assert_eq!(c.trace.node_count(), 5);
    }

    #[test]
    fn render_produces_an_indented_tree() {
        let fmr = hud_leaf();
        let noi = derive("NOI", "noi", &[&fmr.trace], 1_i64);
        let rendered = noi.trace.render();
        assert!(rendered.starts_with("NOI  = noi\n"));
        assert!(rendered.contains("  HUD FMR 2BR  <- hud.fmr[authoritative]"));
    }
}
