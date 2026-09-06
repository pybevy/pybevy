use std::collections::HashSet;

use super::model::{
    ApiCatalog, ClassTestCoverage, MemberRef, ModuleSymbolCoverage, SymbolUseKind,
    TestCoverageReport, TestUsage,
};
use crate::{
    config::TestCoverageConfig,
    output::{Diagnostic, DiagnosticCode},
    python_parser::module_symbols::StubSymbolKind,
};

pub fn analyze_coverage(
    catalog: &ApiCatalog,
    test_usage: &TestUsage,
    config: &TestCoverageConfig,
) -> TestCoverageReport {
    let mut classes = Vec::new();
    let mut total_classes = 0;
    let mut exercised_classes = 0;
    let mut total_members = 0;
    let mut exercised_members = 0;
    let mut executed_members = 0;
    let mut asserted_members = 0;

    for (class_path, api_class) in catalog.classes() {
        let class_name = &api_class.python_name;
        if config.is_excluded(class_path.as_str(), class_name) {
            continue;
        }
        total_classes += 1;
        let class_usage = test_usage.classes.get(class_path);
        let is_exercised = class_usage.is_some();
        if is_exercised {
            exercised_classes += 1;
        }
        let used: HashSet<&MemberRef> = class_usage
            .map(|usage| usage.members.iter().collect())
            .unwrap_or_default();
        let executed: HashSet<&MemberRef> = class_usage
            .map(|usage| usage.executed_members.iter().collect())
            .unwrap_or_default();
        let asserted: HashSet<&MemberRef> = class_usage
            .map(|usage| usage.asserted_members.iter().collect())
            .unwrap_or_default();
        let mut yes = Vec::new();
        let mut no = Vec::new();
        let mut ran = Vec::new();
        let mut checked = Vec::new();

        let has_constructor = api_class.constructor.is_some();
        let constructor_exercised = used.contains(&MemberRef::Constructor);
        if has_constructor {
            classify("__init__ [call]", constructor_exercised, &mut yes, &mut no);
            classify_positive(
                "__init__ [call]",
                executed.contains(&MemberRef::Constructor),
                &mut ran,
            );
            classify_positive(
                "__init__ [call]",
                asserted.contains(&MemberRef::Constructor),
                &mut checked,
            );
        }
        for property in &api_class.properties {
            if config.is_member_excluded(class_path.as_str(), class_name, &property.name) {
                continue;
            }
            if property.has_getter {
                classify(
                    &format!("{} [read]", property.name),
                    used.contains(&MemberRef::Property(property.name.clone())),
                    &mut yes,
                    &mut no,
                );
                classify_positive(
                    &format!("{} [read]", property.name),
                    executed.contains(&MemberRef::Property(property.name.clone())),
                    &mut ran,
                );
                classify_positive(
                    &format!("{} [read]", property.name),
                    asserted.contains(&MemberRef::Property(property.name.clone())),
                    &mut checked,
                );
            }
            if property.has_setter {
                classify(
                    &format!("{} [write]", property.name),
                    used.contains(&MemberRef::PropertySetter(property.name.clone())),
                    &mut yes,
                    &mut no,
                );
                classify_positive(
                    &format!("{} [write]", property.name),
                    executed.contains(&MemberRef::PropertySetter(property.name.clone())),
                    &mut ran,
                );
                classify_positive(
                    &format!("{} [write]", property.name),
                    asserted.contains(&MemberRef::PropertySetter(property.name.clone())),
                    &mut checked,
                );
            }
        }
        for method in &api_class.methods {
            if config.is_member_excluded(class_path.as_str(), class_name, &method.name) {
                continue;
            }
            classify(
                &format!("{} [call]", method.name),
                used.contains(&MemberRef::Method(method.name.clone())),
                &mut yes,
                &mut no,
            );
            classify_positive(
                &format!("{} [call]", method.name),
                executed.contains(&MemberRef::Method(method.name.clone())),
                &mut ran,
            );
            classify_positive(
                &format!("{} [call]", method.name),
                asserted.contains(&MemberRef::Method(method.name.clone())),
                &mut checked,
            );
        }
        for method in &api_class.static_methods {
            if config.is_member_excluded(class_path.as_str(), class_name, &method.name) {
                continue;
            }
            classify(
                &format!("{} [call]", method.name),
                used.contains(&MemberRef::StaticMethod(method.name.clone())),
                &mut yes,
                &mut no,
            );
            classify_positive(
                &format!("{} [call]", method.name),
                executed.contains(&MemberRef::StaticMethod(method.name.clone())),
                &mut ran,
            );
            classify_positive(
                &format!("{} [call]", method.name),
                asserted.contains(&MemberRef::StaticMethod(method.name.clone())),
                &mut checked,
            );
        }
        for attribute in &api_class.class_attrs {
            if config.is_member_excluded(class_path.as_str(), class_name, &attribute.name) {
                continue;
            }
            classify(
                &format!("{} [read]", attribute.name),
                used.contains(&MemberRef::ClassAttr(attribute.name.clone())),
                &mut yes,
                &mut no,
            );
            classify_positive(
                &format!("{} [read]", attribute.name),
                executed.contains(&MemberRef::ClassAttr(attribute.name.clone())),
                &mut ran,
            );
            classify_positive(
                &format!("{} [read]", attribute.name),
                asserted.contains(&MemberRef::ClassAttr(attribute.name.clone())),
                &mut checked,
            );
        }
        for variant in &api_class.enum_variants {
            if config.is_member_excluded(class_path.as_str(), class_name, &variant.name) {
                continue;
            }
            classify(
                &format!("{} [read]", variant.name),
                used.contains(&MemberRef::EnumVariant(variant.name.clone())),
                &mut yes,
                &mut no,
            );
            classify_positive(
                &format!("{} [read]", variant.name),
                executed.contains(&MemberRef::EnumVariant(variant.name.clone())),
                &mut ran,
            );
            classify_positive(
                &format!("{} [read]", variant.name),
                asserted.contains(&MemberRef::EnumVariant(variant.name.clone())),
                &mut checked,
            );
        }
        for nested_type in &api_class.nested_types {
            classify(
                &format!("{nested_type} [type]"),
                used.contains(&MemberRef::NestedType(nested_type.clone())),
                &mut yes,
                &mut no,
            );
            classify_positive(
                &format!("{nested_type} [type]"),
                executed.contains(&MemberRef::NestedType(nested_type.clone())),
                &mut ran,
            );
            classify_positive(
                &format!("{nested_type} [type]"),
                asserted.contains(&MemberRef::NestedType(nested_type.clone())),
                &mut checked,
            );
        }

        let total = yes.len() + no.len();
        let exercised = yes.len();
        total_members += total;
        exercised_members += exercised;
        executed_members += ran.len();
        asserted_members += checked.len();
        let coverage_pct = if total == 0 {
            if is_exercised { 100.0 } else { 0.0 }
        } else {
            (exercised as f32 / total as f32) * 100.0
        };
        let mut evidence_files = class_usage
            .map(|usage| usage.files.iter().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        evidence_files.sort();
        classes.push(ClassTestCoverage {
            class_path: class_path.clone(),
            class_name: class_name.clone(),
            module_path: class_path.module().to_string(),
            is_exercised,
            constructor_exercised,
            has_constructor,
            exercised_members: yes,
            executed_members: ran,
            asserted_members: checked,
            unexercised_members: no,
            total_members: total,
            coverage_pct,
            evidence_files,
        });
    }

    let module_symbols = catalog
        .module_symbols()
        .map(|(path, kind)| {
            let usage = test_usage.symbols.get(path);
            let is_exercised = usage.is_some_and(|usage| match kind {
                StubSymbolKind::Function => usage.kinds.contains(&SymbolUseKind::Call),
                StubSymbolKind::Alias | StubSymbolKind::Constant => !usage.kinds.is_empty(),
                StubSymbolKind::Class => unreachable!("classes are not module-symbol rows"),
            });
            let mut evidence_files = usage
                .map(|usage| usage.files.iter().cloned().collect::<Vec<_>>())
                .unwrap_or_default();
            evidence_files.sort();
            ModuleSymbolCoverage {
                path: path.clone(),
                kind,
                is_exercised,
                is_executed: usage.is_some_and(|usage| match kind {
                    StubSymbolKind::Function => usage.executed_kinds.contains(&SymbolUseKind::Call),
                    StubSymbolKind::Alias | StubSymbolKind::Constant => {
                        usage.executed_kinds.contains(&SymbolUseKind::Read)
                    }
                    StubSymbolKind::Class => false,
                }),
                is_asserted: usage.is_some_and(|usage| match kind {
                    StubSymbolKind::Function => usage.asserted_kinds.contains(&SymbolUseKind::Call),
                    StubSymbolKind::Alias | StubSymbolKind::Constant => {
                        usage.asserted_kinds.contains(&SymbolUseKind::Read)
                    }
                    StubSymbolKind::Class => false,
                }),
                evidence_files,
            }
        })
        .collect::<Vec<_>>();
    let exercised_module_symbols = module_symbols
        .iter()
        .filter(|symbol| symbol.is_exercised)
        .count();

    TestCoverageReport {
        classes,
        total_module_symbols: module_symbols.len(),
        exercised_module_symbols,
        module_symbols,
        total_classes,
        exercised_classes,
        total_members,
        exercised_members,
        executed_members,
        asserted_members,
        execution_data_loaded: test_usage.execution_data_loaded,
        unresolved: test_usage.unresolved.iter().cloned().collect(),
    }
}

fn classify(display: &str, exercised: bool, yes: &mut Vec<String>, no: &mut Vec<String>) {
    if exercised {
        yes.push(display.to_string());
    } else {
        no.push(display.to_string());
    }
}

fn classify_positive(display: &str, present: bool, values: &mut Vec<String>) {
    if present {
        values.push(display.to_string());
    }
}

pub fn generate_diagnostics(report: &TestCoverageReport) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for class in &report.classes {
        if !class.is_exercised {
            diagnostics.push(Diagnostic::info(
                DiagnosticCode::T001,
                format!("`{}` has no test exercise evidence", class.class_path),
            ));
            continue;
        }
        if class.has_constructor && !class.constructor_exercised {
            diagnostics.push(Diagnostic::info(
                DiagnosticCode::T002,
                format!("`{}.__init__` is not exercised", class.class_path),
            ));
        }
        for member in &class.unexercised_members {
            if member != "__init__ [call]" {
                diagnostics.push(Diagnostic::info(
                    DiagnosticCode::T003,
                    format!("`{}.{}` is not exercised", class.class_path, member),
                ));
            }
        }
    }
    diagnostics
}
