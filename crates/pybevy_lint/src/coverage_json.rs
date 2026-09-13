//! Machine-readable rendering of the API coverage report.
//!
//! The table is written for a person scanning modules; this is written for a
//! dashboard or a trend line. It deliberately does not serialise the internal
//! report structs: those change with the comparison's needs, and a consumer
//! pinned to them would break on every refactor. The shape below is the
//! contract, and `schema_version` is what a consumer checks.

use serde::Serialize;

use crate::{
    bevy_audit::{BEVY_REVISION, BEVY_TAG, CARGO_PUBLIC_API_VERSION},
    bevy_parser,
    comparison::CoverageReport,
};

pub const SCHEMA_VERSION: u32 = 1;

/// Matched-out-of-total, with the percentage a reader would otherwise compute.
#[derive(Debug, Serialize)]
pub struct Ratio {
    pub matched: usize,
    pub total: usize,
    pub percent: f64,
}

impl Ratio {
    fn new(matched: usize, total: usize) -> Self {
        let percent = if total == 0 {
            100.0
        } else {
            (matched as f64 / total as f64) * 100.0
        };
        Self {
            matched,
            total,
            // Two decimals: enough to see movement, few enough that an
            // unchanged run produces an unchanged file.
            percent: (percent * 100.0).round() / 100.0,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Provenance {
    pub bevy_tag: &'static str,
    pub bevy_revision: &'static str,
    pub cargo_public_api: &'static str,
    pub extractor_flags: String,
}

#[derive(Debug, Serialize)]
pub struct ModuleCoverage {
    /// PyBevy module, absent when the Bevy crate has no mapped module.
    pub module: Option<String>,
    pub bevy_crate: String,
    pub types: Ratio,
    /// Members are counted only within implemented types.
    pub methods: Ratio,
    pub fields: Ratio,
    pub variants: Ratio,
    pub signature_mismatches: usize,
    pub extra_methods: usize,
    pub extra_variants: usize,
}

#[derive(Debug, Serialize)]
pub struct Totals {
    pub types: Ratio,
    pub methods: Ratio,
    pub fields: Ratio,
    pub variants: Ratio,
    pub signature_mismatches: usize,
    pub extra_methods: usize,
    pub extra_variants: usize,
    pub mismatched_variants: usize,
    pub enum_representation_mismatches: usize,
}

#[derive(Debug, Serialize)]
pub struct CoverageDocument {
    pub schema_version: u32,
    pub provenance: Provenance,
    pub totals: Totals,
    /// Sorted by module name so a diff between runs shows real movement.
    pub modules: Vec<ModuleCoverage>,
}

/// Render the report in the documented JSON shape.
pub fn render(report: &CoverageReport) -> CoverageDocument {
    let mut modules: Vec<ModuleCoverage> = report
        .crates
        .iter()
        .map(|(crate_name, coverage)| {
            let totals = coverage.implemented_totals();
            ModuleCoverage {
                module: coverage.pybevy_module.clone(),
                bevy_crate: crate_name.clone(),
                types: Ratio::new(coverage.matched_count, coverage.bevy_type_count),
                methods: Ratio::new(totals.matched_methods, totals.bevy_methods),
                fields: Ratio::new(totals.matched_fields, totals.bevy_fields),
                variants: Ratio::new(totals.matched_variants, totals.bevy_variants),
                signature_mismatches: totals.signature_mismatches,
                extra_methods: totals.extra_methods,
                extra_variants: totals.extra_variants,
            }
        })
        .collect();
    modules.sort_by(|left, right| {
        left.module
            .cmp(&right.module)
            .then_with(|| left.bevy_crate.cmp(&right.bevy_crate))
    });

    CoverageDocument {
        schema_version: SCHEMA_VERSION,
        provenance: Provenance {
            bevy_tag: BEVY_TAG,
            bevy_revision: BEVY_REVISION,
            cargo_public_api: CARGO_PUBLIC_API_VERSION,
            extractor_flags: bevy_parser::public_api_flags(),
        },
        totals: Totals {
            types: Ratio::new(report.matched_types, report.total_bevy_types),
            methods: Ratio::new(
                report.implemented_type_matched_methods,
                report.implemented_type_bevy_methods,
            ),
            fields: Ratio::new(report.matched_fields, report.total_bevy_fields),
            variants: Ratio::new(report.matched_variants, report.total_bevy_variants),
            signature_mismatches: report.signature_mismatches,
            extra_methods: report.extra_methods,
            extra_variants: report.extra_variants,
            mismatched_variants: report.mismatched_variants,
            enum_representation_mismatches: report.enum_representation_mismatches,
        },
        modules,
    }
}
