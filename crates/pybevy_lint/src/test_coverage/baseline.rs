use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::model::{ApiCatalog, MemberRef, SymbolUseKind, TestUsage};
use crate::python_parser::module_symbols::StubSymbolKind;

const SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct OperationKey {
    pub path: String,
    pub kind: OperationKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationKind {
    TypeReference,
    ModuleFunctionCall,
    ModuleSymbolRead,
    ConstructorCall,
    PropertyRead,
    PropertyWrite,
    MethodCall,
    StaticMethodCall,
    ClassAttributeRead,
    EnumVariantRead,
}

impl OperationKind {
    fn is_behavior_bearing(self) -> bool {
        matches!(
            self,
            Self::ModuleFunctionCall
                | Self::ConstructorCall
                | Self::PropertyRead
                | Self::PropertyWrite
                | Self::MethodCall
                | Self::StaticMethodCall
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BaselineStatus {
    Exercised,
    Debt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BaselineEntry {
    #[serde(flatten)]
    pub operation: OperationKey,
    pub status: BaselineStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExerciseBaseline {
    pub schema_version: u32,
    pub operations: Vec<BaselineEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExerciseException {
    #[serde(flatten)]
    pub operation: OperationKey,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExerciseExceptions {
    pub schema_version: u32,
    pub exceptions: Vec<ExerciseException>,
}

#[derive(Debug, Default)]
pub struct BaselineCheck {
    pub errors: Vec<String>,
}

pub fn format_debt_summary(path: &Path) -> Result<String> {
    let baseline: ExerciseBaseline = serde_json::from_str(
        &fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?,
    )
    .with_context(|| format!("Failed to parse {}", path.display()))?;
    Ok(format_debt_summary_from(&baseline))
}

fn format_debt_summary_from(baseline: &ExerciseBaseline) -> String {
    let mut groups = BTreeMap::<(String, &'static str, OperationKind), usize>::new();
    for entry in &baseline.operations {
        if entry.status != BaselineStatus::Debt {
            continue;
        }
        let module = entry
            .operation
            .path
            .strip_prefix("pybevy.")
            .and_then(|path| path.split('.').next())
            .unwrap_or("<root>")
            .to_string();
        *groups
            .entry((
                module.clone(),
                operation_risk(&module, entry.operation.kind),
                entry.operation.kind,
            ))
            .or_default() += 1;
    }

    let mut output = String::from(
        "\nReviewed API exercise debt\n\nModule                Risk      Operation kind              Count\n",
    );
    output.push_str("-----------------------------------------------------------------\n");
    for ((module, risk, kind), count) in groups {
        output.push_str(&format!(
            "{module:<21} {risk:<9} {:<27} {count:>5}\n",
            operation_kind(kind),
        ));
    }
    let total = baseline
        .operations
        .iter()
        .filter(|entry| entry.status == BaselineStatus::Debt)
        .count();
    output.push_str("-----------------------------------------------------------------\n");
    output.push_str(&format!("Total reviewed debt: {total}\n"));
    output
}

fn operation_risk(module: &str, kind: OperationKind) -> &'static str {
    if kind == OperationKind::PropertyWrite
        || matches!(module, "app" | "ecs" | "assets" | "host" | "project")
    {
        "high"
    } else if matches!(
        kind,
        OperationKind::ModuleFunctionCall
            | OperationKind::ConstructorCall
            | OperationKind::MethodCall
            | OperationKind::StaticMethodCall
    ) || matches!(module, "render" | "image" | "mesh" | "window" | "input")
    {
        "medium"
    } else {
        "standard"
    }
}

impl BaselineCheck {
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }
}

pub fn update_baseline(
    catalog: &ApiCatalog,
    usage: &TestUsage,
    baseline_path: &Path,
    exceptions_path: &Path,
) -> Result<()> {
    reject_unresolved(usage)?;
    let current = collect_gated_operations(catalog, usage);
    let exceptions = read_exceptions(exceptions_path)?;
    let exception_keys = validate_exceptions(&exceptions, &current)?;
    let operations = current
        .into_iter()
        .filter(|(operation, _)| !exception_keys.contains(operation))
        .map(|(operation, exercised)| BaselineEntry {
            operation,
            status: if exercised {
                BaselineStatus::Exercised
            } else {
                BaselineStatus::Debt
            },
        })
        .collect();
    let baseline = ExerciseBaseline {
        schema_version: SCHEMA_VERSION,
        operations,
    };
    if let Some(parent) = baseline_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        baseline_path,
        serde_json::to_string_pretty(&baseline)? + "\n",
    )
    .with_context(|| format!("Failed to write {}", baseline_path.display()))?;
    Ok(())
}

pub fn check_baseline(
    catalog: &ApiCatalog,
    usage: &TestUsage,
    baseline_path: &Path,
    exceptions_path: &Path,
) -> Result<BaselineCheck> {
    let baseline: ExerciseBaseline = serde_json::from_str(
        &fs::read_to_string(baseline_path)
            .with_context(|| format!("Failed to read {}", baseline_path.display()))?,
    )
    .with_context(|| format!("Failed to parse {}", baseline_path.display()))?;
    let exceptions = read_exceptions(exceptions_path)?;
    let current = collect_gated_operations(catalog, usage);
    let mut errors = Vec::new();

    for reference in &usage.unresolved {
        errors.push(format!(
            "unresolved PyBevy reference: {}:{}: `{}` ({})",
            reference.file.display(),
            reference.line,
            reference.expression,
            reference.reason,
        ));
    }

    if baseline.schema_version != SCHEMA_VERSION {
        errors.push(format!(
            "baseline schema version is {}, expected {SCHEMA_VERSION}",
            baseline.schema_version
        ));
    }
    if exceptions.schema_version != SCHEMA_VERSION {
        errors.push(format!(
            "exception schema version is {}, expected {SCHEMA_VERSION}",
            exceptions.schema_version
        ));
    }

    validate_sorted_unique_baseline(&baseline, &mut errors);
    validate_sorted_unique_exceptions(&exceptions, &mut errors);

    let baseline_entries = baseline
        .operations
        .iter()
        .map(|entry| (entry.operation.clone(), entry.status))
        .collect::<BTreeMap<_, _>>();
    let exception_entries = exceptions
        .exceptions
        .iter()
        .map(|entry| (entry.operation.clone(), entry.reason.as_str()))
        .collect::<BTreeMap<_, _>>();

    for (operation, exercised) in &current {
        if let Some(reason) = exception_entries.get(operation) {
            if reason.trim().is_empty() {
                errors.push(format_operation("exception has an empty reason", operation));
            }
            if *exercised {
                errors.push(format_operation(
                    "exception is stale: operation is exercised",
                    operation,
                ));
            }
            continue;
        }
        match baseline_entries.get(operation) {
            None => errors.push(format_operation(
                "new operation is missing from the baseline",
                operation,
            )),
            Some(BaselineStatus::Exercised) if !exercised => {
                errors.push(format_operation(
                    "previously exercised operation lost all evidence",
                    operation,
                ));
            }
            Some(BaselineStatus::Debt) if *exercised => {
                errors.push(format_operation(
                    "debt is stale: operation now has evidence; update the baseline",
                    operation,
                ));
            }
            _ => {}
        }
    }

    for operation in baseline_entries.keys() {
        if !current.contains_key(operation) {
            errors.push(format_operation(
                "baseline operation is no longer in the stub contract",
                operation,
            ));
        }
        if exception_entries.contains_key(operation) {
            errors.push(format_operation(
                "operation appears in both baseline and exceptions",
                operation,
            ));
        }
    }
    for operation in exception_entries.keys() {
        if !current.contains_key(operation) {
            errors.push(format_operation(
                "exception is stale: path left the stub contract",
                operation,
            ));
        }
    }

    errors.sort();
    errors.dedup();
    Ok(BaselineCheck { errors })
}

fn reject_unresolved(usage: &TestUsage) -> Result<()> {
    if usage.unresolved.is_empty() {
        return Ok(());
    }
    let details = usage
        .unresolved
        .iter()
        .map(|reference| {
            format!(
                "{}:{}: `{}` ({})",
                reference.file.display(),
                reference.line,
                reference.expression,
                reference.reason,
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    anyhow::bail!("cannot update baseline with unresolved PyBevy references:\n{details}")
}

fn collect_operations(catalog: &ApiCatalog, usage: &TestUsage) -> BTreeMap<OperationKey, bool> {
    let mut operations = BTreeMap::new();
    for (class_path, class) in catalog.classes() {
        let class_usage = usage.classes.get(class_path);
        operations.insert(
            OperationKey {
                path: class_path.to_string(),
                kind: OperationKind::TypeReference,
            },
            class_usage.is_some(),
        );
        let mut insert_member = |name: &str, kind: OperationKind, member: MemberRef| {
            operations.insert(
                OperationKey {
                    path: if name.is_empty() {
                        class_path.to_string()
                    } else {
                        format!("{class_path}.{name}")
                    },
                    kind,
                },
                class_usage.is_some_and(|usage| usage.members.contains(&member)),
            );
        };
        if class.constructor.is_some() {
            insert_member("", OperationKind::ConstructorCall, MemberRef::Constructor);
        }
        for property in &class.properties {
            if property.has_getter {
                insert_member(
                    &property.name,
                    OperationKind::PropertyRead,
                    MemberRef::Property(property.name.clone()),
                );
            }
            if property.has_setter {
                insert_member(
                    &property.name,
                    OperationKind::PropertyWrite,
                    MemberRef::PropertySetter(property.name.clone()),
                );
            }
        }
        for method in &class.methods {
            insert_member(
                &method.name,
                OperationKind::MethodCall,
                MemberRef::Method(method.name.clone()),
            );
        }
        for method in &class.static_methods {
            insert_member(
                &method.name,
                OperationKind::StaticMethodCall,
                MemberRef::StaticMethod(method.name.clone()),
            );
        }
        for attribute in &class.class_attrs {
            insert_member(
                &attribute.name,
                OperationKind::ClassAttributeRead,
                MemberRef::ClassAttr(attribute.name.clone()),
            );
        }
        for variant in &class.enum_variants {
            insert_member(
                &variant.name,
                OperationKind::EnumVariantRead,
                MemberRef::EnumVariant(variant.name.clone()),
            );
        }
        for nested_type in &class.nested_types {
            insert_member(
                nested_type,
                OperationKind::TypeReference,
                MemberRef::NestedType(nested_type.clone()),
            );
        }
    }
    for (path, kind) in catalog.module_symbols() {
        let symbol_usage = usage.symbols.get(path);
        let (operation_kind, exercised) = match kind {
            StubSymbolKind::Function => (
                OperationKind::ModuleFunctionCall,
                symbol_usage.is_some_and(|usage| usage.kinds.contains(&SymbolUseKind::Call)),
            ),
            StubSymbolKind::Alias | StubSymbolKind::Constant => (
                OperationKind::ModuleSymbolRead,
                symbol_usage.is_some_and(|usage| !usage.kinds.is_empty()),
            ),
            StubSymbolKind::Class => continue,
        };
        operations.insert(
            OperationKey {
                path: path.to_string(),
                kind: operation_kind,
            },
            exercised,
        );
    }
    operations
}

fn collect_gated_operations(
    catalog: &ApiCatalog,
    usage: &TestUsage,
) -> BTreeMap<OperationKey, bool> {
    collect_operations(catalog, usage)
        .into_iter()
        .filter(|(operation, _)| operation.kind.is_behavior_bearing())
        .collect()
}

fn read_exceptions(path: &Path) -> Result<ExerciseExceptions> {
    if !path.exists() {
        return Ok(ExerciseExceptions {
            schema_version: SCHEMA_VERSION,
            exceptions: Vec::new(),
        });
    }
    serde_json::from_str(&fs::read_to_string(path)?)
        .with_context(|| format!("Failed to parse {}", path.display()))
}

fn validate_exceptions(
    exceptions: &ExerciseExceptions,
    current: &BTreeMap<OperationKey, bool>,
) -> Result<BTreeSet<OperationKey>> {
    let mut errors = Vec::new();
    validate_sorted_unique_exceptions(exceptions, &mut errors);
    for exception in &exceptions.exceptions {
        if exception.reason.trim().is_empty() {
            errors.push(format_operation(
                "exception has an empty reason",
                &exception.operation,
            ));
        }
        match current.get(&exception.operation) {
            None => errors.push(format_operation(
                "exception path is not in the stub contract",
                &exception.operation,
            )),
            Some(true) => errors.push(format_operation(
                "exception is stale: operation is exercised",
                &exception.operation,
            )),
            Some(false) => {}
        }
    }
    if !errors.is_empty() {
        anyhow::bail!(errors.join("\n"));
    }
    Ok(exceptions
        .exceptions
        .iter()
        .map(|exception| exception.operation.clone())
        .collect())
}

fn validate_sorted_unique_baseline(baseline: &ExerciseBaseline, errors: &mut Vec<String>) {
    let keys = baseline
        .operations
        .iter()
        .map(|entry| &entry.operation)
        .collect::<Vec<_>>();
    if !keys.windows(2).all(|window| window[0] < window[1]) {
        errors.push("baseline operations must be strictly sorted and unique".to_string());
    }
}

fn validate_sorted_unique_exceptions(exceptions: &ExerciseExceptions, errors: &mut Vec<String>) {
    let keys = exceptions
        .exceptions
        .iter()
        .map(|entry| &entry.operation)
        .collect::<Vec<_>>();
    if !keys.windows(2).all(|window| window[0] < window[1]) {
        errors.push("exceptions must be strictly sorted and unique".to_string());
    }
}

fn format_operation(message: &str, operation: &OperationKey) -> String {
    format!(
        "{message}: {} [{}]",
        operation.path,
        operation_kind(operation.kind)
    )
}

fn operation_kind(kind: OperationKind) -> &'static str {
    match kind {
        OperationKind::TypeReference => "type_reference",
        OperationKind::ModuleFunctionCall => "module_function_call",
        OperationKind::ModuleSymbolRead => "module_symbol_read",
        OperationKind::ConstructorCall => "constructor_call",
        OperationKind::PropertyRead => "property_read",
        OperationKind::PropertyWrite => "property_write",
        OperationKind::MethodCall => "method_call",
        OperationKind::StaticMethodCall => "static_method_call",
        OperationKind::ClassAttributeRead => "class_attribute_read",
        OperationKind::EnumVariantRead => "enum_variant_read",
    }
}
