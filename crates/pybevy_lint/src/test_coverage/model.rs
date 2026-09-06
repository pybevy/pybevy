use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fmt,
    path::{Path, PathBuf},
};

use crate::{
    model::PyClassDef,
    python_parser::module_symbols::{StubReexport, StubSymbolDef, StubSymbolKind},
};

/// Canonical fully-qualified path to one public Python class.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ApiPath(String);

impl ApiPath {
    pub fn new(module: &str, class_name: &str) -> Self {
        Self(format!("{}.{}", canonical_module(module), class_name))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn module(&self) -> &str {
        self.0.rsplit_once('.').map_or("", |(module, _)| module)
    }

    pub fn from_canonical(path: &str) -> Option<Self> {
        (path == "pybevy" || path.starts_with("pybevy.")).then(|| Self(path.to_string()))
    }
}

impl fmt::Display for ApiPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

fn canonical_module(module: &str) -> String {
    let module = module
        .strip_suffix(".__init__")
        .or_else(|| (module == "__init__").then_some(""))
        .unwrap_or(module);
    if module.is_empty() {
        "pybevy".to_string()
    } else if module == "pybevy" || module.starts_with("pybevy.") {
        module.to_string()
    } else {
        format!("pybevy.{module}")
    }
}

fn is_public(name: &str) -> bool {
    !name.starts_with('_')
}

/// Stub-derived public class catalog used to resolve test references.
#[derive(Debug, Default)]
pub struct ApiCatalog {
    classes: BTreeMap<ApiPath, PyClassDef>,
    paths_by_name: HashMap<String, Vec<ApiPath>>,
    symbols: BTreeMap<ApiPath, StubSymbolKind>,
    symbol_paths_by_name: HashMap<String, Vec<ApiPath>>,
    wildcard_symbols: BTreeMap<String, (ApiPath, StubSymbolKind)>,
}

impl ApiCatalog {
    pub fn from_stub_classes(classes: Vec<PyClassDef>) -> Self {
        let mut catalog = Self::default();
        for mut class in classes {
            if !is_public(&class.python_name) {
                continue;
            }
            let Some(module) = class.module_path.as_deref() else {
                continue;
            };
            class.methods.retain(|method| is_public(&method.name));
            class
                .static_methods
                .retain(|method| is_public(&method.name));
            class
                .properties
                .retain(|property| is_public(&property.name));
            class
                .class_attrs
                .retain(|attribute| is_public(&attribute.name));
            class
                .enum_variants
                .retain(|variant| is_public(&variant.name));
            class.nested_types.retain(|name| is_public(name));

            let path = ApiPath::new(module, &class.python_name);
            catalog
                .paths_by_name
                .entry(class.python_name.clone())
                .or_default()
                .push(path.clone());
            catalog.symbols.insert(path.clone(), StubSymbolKind::Class);
            catalog.classes.insert(path, class);
        }
        for paths in catalog.paths_by_name.values_mut() {
            paths.sort();
            paths.dedup();
        }
        catalog.rebuild_symbol_name_index();
        catalog
    }

    pub fn add_stub_symbols(&mut self, symbols: Vec<StubSymbolDef>) {
        for symbol in symbols {
            let path = ApiPath::new(&symbol.module_path, &symbol.name);
            self.symbols.insert(path, symbol.kind);
        }
        self.rebuild_symbol_name_index();
    }

    pub fn add_wildcard_reexports(&mut self, reexports: Vec<StubReexport>) {
        for reexport in reexports {
            if let Some((path, kind)) =
                self.resolve_exact_symbol(&reexport.module, &reexport.symbol)
            {
                self.wildcard_symbols.insert(reexport.name, (path, kind));
            }
        }
    }

    pub fn wildcard_symbols(&self) -> impl Iterator<Item = (&str, &ApiPath, StubSymbolKind)> {
        self.wildcard_symbols
            .iter()
            .map(|(name, (path, kind))| (name.as_str(), path, *kind))
    }

    pub fn classes(&self) -> impl Iterator<Item = (&ApiPath, &PyClassDef)> {
        self.classes.iter()
    }

    pub fn class_path(&self, path: &str) -> Option<ApiPath> {
        let path = ApiPath::from_canonical(path)?;
        self.classes.contains_key(&path).then_some(path)
    }

    pub fn symbol_path(&self, path: &str) -> Option<(ApiPath, StubSymbolKind)> {
        let path = ApiPath::from_canonical(path)?;
        self.symbols.get(&path).map(|kind| (path, *kind))
    }

    pub fn has_member(&self, class_path: &ApiPath, member: &MemberRef) -> bool {
        let Some(class) = self.classes.get(class_path) else {
            return false;
        };
        match member {
            MemberRef::Constructor => class.constructor.is_some(),
            MemberRef::Property(name) => class
                .properties
                .iter()
                .any(|property| property.name == *name && property.has_getter),
            MemberRef::PropertySetter(name) => class
                .properties
                .iter()
                .any(|property| property.name == *name && property.has_setter),
            MemberRef::Method(name) => class.methods.iter().any(|method| method.name == *name),
            MemberRef::StaticMethod(name) => class
                .static_methods
                .iter()
                .any(|method| method.name == *name),
            MemberRef::ClassAttr(name) => class
                .class_attrs
                .iter()
                .any(|attribute| attribute.name == *name),
            MemberRef::EnumVariant(name) => class
                .enum_variants
                .iter()
                .any(|variant| variant.name == *name),
            MemberRef::NestedType(name) => class.nested_types.contains(name),
        }
    }

    pub fn class_read_member(&self, class_path: &ApiPath, name: &str) -> MemberRef {
        let Some(class) = self.classes.get(class_path) else {
            return MemberRef::ClassAttr(name.to_string());
        };
        if class.nested_types.iter().any(|nested| nested == name) {
            MemberRef::NestedType(name.to_string())
        } else if class
            .enum_variants
            .iter()
            .any(|variant| variant.name == name)
        {
            MemberRef::EnumVariant(name.to_string())
        } else {
            MemberRef::ClassAttr(name.to_string())
        }
    }

    pub fn len(&self) -> usize {
        self.classes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.classes.is_empty()
    }

    pub fn resolve_import(&self, module: &str, symbol: &str) -> Option<ApiPath> {
        let exact = ApiPath::new(module, symbol);
        if self.classes.contains_key(&exact) {
            return Some(exact);
        }
        self.resolve_unique_name(symbol)
    }

    pub fn resolve_symbol(&self, module: &str, symbol: &str) -> Option<(ApiPath, StubSymbolKind)> {
        let exact = ApiPath::new(module, symbol);
        if let Some(kind) = self.symbols.get(&exact) {
            return Some((exact, *kind));
        }
        let paths = self.symbol_paths_by_name.get(symbol)?;
        if paths.len() != 1 {
            return None;
        }
        let path = paths[0].clone();
        Some((path.clone(), self.symbols[&path]))
    }

    pub fn resolve_exact_symbol(
        &self,
        module: &str,
        symbol: &str,
    ) -> Option<(ApiPath, StubSymbolKind)> {
        let path = ApiPath::new(module, symbol);
        self.symbols.get(&path).map(|kind| (path, *kind))
    }

    pub fn has_module(&self, module: &str) -> bool {
        let module = canonical_module(module);
        self.symbols.keys().any(|path| path.module() == module)
    }

    pub fn unique_symbols(&self) -> impl Iterator<Item = (&str, &ApiPath, StubSymbolKind)> {
        self.symbol_paths_by_name
            .iter()
            .filter(|(_, paths)| paths.len() == 1)
            .map(|(name, paths)| (name.as_str(), &paths[0], self.symbols[&paths[0]]))
    }

    pub fn module_symbols(&self) -> impl Iterator<Item = (&ApiPath, StubSymbolKind)> {
        self.symbols
            .iter()
            .filter(|(_, kind)| **kind != StubSymbolKind::Class)
            .map(|(path, kind)| (path, *kind))
    }

    pub fn resolve_unique_name(&self, symbol: &str) -> Option<ApiPath> {
        let paths = self.paths_by_name.get(symbol)?;
        (paths.len() == 1).then(|| paths[0].clone())
    }

    pub fn resolve_method_return(
        &self,
        owner: &ApiPath,
        method_name: &str,
        is_static: bool,
    ) -> Option<ApiPath> {
        let class = self.classes.get(owner)?;
        let methods = if is_static {
            &class.static_methods
        } else {
            &class.methods
        };
        let annotation = methods
            .iter()
            .find(|method| method.name == method_name)?
            .return_type
            .as_deref()?;
        self.resolve_return_annotation(owner, annotation)
    }

    pub fn resolve_class_attr_type(&self, owner: &ApiPath, attr_name: &str) -> Option<ApiPath> {
        let annotation = self
            .classes
            .get(owner)?
            .class_attrs
            .iter()
            .find(|attribute| attribute.name == attr_name)?
            .attr_type
            .as_deref()?;
        self.resolve_return_annotation(owner, annotation)
    }

    pub fn resolve_property_type(&self, owner: &ApiPath, property_name: &str) -> Option<ApiPath> {
        let annotation = self
            .classes
            .get(owner)?
            .properties
            .iter()
            .find(|property| property.name == property_name && property.has_getter)?
            .property_type
            .as_deref()?;
        self.resolve_return_annotation(owner, annotation)
    }

    pub fn unique_paths(&self) -> impl Iterator<Item = (&str, &ApiPath)> {
        self.paths_by_name
            .iter()
            .filter_map(|(name, paths)| (paths.len() == 1).then_some((name.as_str(), &paths[0])))
    }

    pub fn retain(&mut self, mut predicate: impl FnMut(&ApiPath, &PyClassDef) -> bool) {
        self.classes.retain(|path, class| predicate(path, class));
        self.paths_by_name.clear();
        for (path, class) in &self.classes {
            self.paths_by_name
                .entry(class.python_name.clone())
                .or_default()
                .push(path.clone());
        }
    }

    fn rebuild_symbol_name_index(&mut self) {
        self.symbol_paths_by_name.clear();
        for path in self.symbols.keys() {
            let name = path.as_str().rsplit('.').next().unwrap_or_default();
            self.symbol_paths_by_name
                .entry(name.to_string())
                .or_default()
                .push(path.clone());
        }
        for paths in self.symbol_paths_by_name.values_mut() {
            paths.sort();
            paths.dedup();
        }
    }

    fn resolve_return_annotation(&self, owner: &ApiPath, annotation: &str) -> Option<ApiPath> {
        let mut annotation = annotation.trim().trim_matches(['\'', '"']);
        if annotation == "Self" {
            return Some(owner.clone());
        }
        for wrapper in ["Optional", "ClassVar"] {
            if let Some(inner) = annotation
                .strip_prefix(wrapper)
                .and_then(|value| value.strip_prefix('['))
                .and_then(|value| value.strip_suffix(']'))
            {
                annotation = inner.trim();
            }
        }
        if annotation.contains('|') {
            let mut concrete = annotation
                .split('|')
                .map(str::trim)
                .filter(|part| !matches!(*part, "None" | "NoneType"));
            annotation = concrete.next()?;
            if concrete.next().is_some() {
                return None;
            }
        }
        if let Some((outer, _)) = annotation.split_once('[') {
            if !annotation.ends_with(']') {
                return None;
            }
            annotation = outer.trim();
        }
        if annotation.contains('[') || annotation.contains(',') {
            return None;
        }
        let name = annotation.rsplit('.').next()?;
        let exact = ApiPath::new(owner.module(), name);
        if self.classes.contains_key(&exact) {
            Some(exact)
        } else {
            self.resolve_unique_name(name)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MemberRef {
    Constructor,
    Property(String),
    PropertySetter(String),
    Method(String),
    StaticMethod(String),
    ClassAttr(String),
    EnumVariant(String),
    NestedType(String),
}

impl MemberRef {
    pub fn name(&self) -> &str {
        match self {
            MemberRef::Constructor => "__init__",
            MemberRef::Property(name)
            | MemberRef::PropertySetter(name)
            | MemberRef::Method(name)
            | MemberRef::StaticMethod(name)
            | MemberRef::ClassAttr(name)
            | MemberRef::EnumVariant(name)
            | MemberRef::NestedType(name) => name,
        }
    }

    pub fn kind_str(&self) -> &'static str {
        match self {
            MemberRef::Constructor => "constructor_call",
            MemberRef::Property(_) => "property_read",
            MemberRef::PropertySetter(_) => "property_write",
            MemberRef::Method(_) => "method_call",
            MemberRef::StaticMethod(_) => "static_method_call",
            MemberRef::ClassAttr(_) => "class_attribute_read",
            MemberRef::EnumVariant(_) => "enum_variant_read",
            MemberRef::NestedType(_) => "nested_type_reference",
        }
    }
}

#[derive(Debug, Default)]
pub struct ClassUsage {
    /// Informational evidence only; baseline equality must not depend on paths.
    pub files: HashSet<PathBuf>,
    pub members: HashSet<MemberRef>,
    pub executed_members: HashSet<MemberRef>,
    pub asserted_members: HashSet<MemberRef>,
    pub member_files: HashMap<MemberRef, HashSet<PathBuf>>,
    pub member_sites: HashMap<MemberRef, HashSet<EvidenceSite>>,
    pub assertion_sites: HashMap<MemberRef, HashSet<EvidenceSite>>,
}

#[derive(Debug, Default)]
pub struct TestUsage {
    pub classes: HashMap<ApiPath, ClassUsage>,
    pub symbols: HashMap<ApiPath, SymbolUsage>,
    pub unresolved: BTreeSet<UnresolvedReference>,
    pub execution_data_loaded: bool,
}

impl TestUsage {
    pub fn record_class(&mut self, class_path: &ApiPath, file: &Path) {
        self.classes
            .entry(class_path.clone())
            .or_default()
            .files
            .insert(file.to_path_buf());
    }

    pub fn record_member(&mut self, class_path: &ApiPath, member: MemberRef, file: &Path) {
        self.record_member_at(class_path, member, file, 0);
    }

    pub fn record_member_at(
        &mut self,
        class_path: &ApiPath,
        member: MemberRef,
        file: &Path,
        line: usize,
    ) {
        self.record_member_site(class_path, member, file, line, true);
    }

    pub fn record_member_site(
        &mut self,
        class_path: &ApiPath,
        member: MemberRef,
        file: &Path,
        line: usize,
        execution_eligible: bool,
    ) {
        let entry = self.classes.entry(class_path.clone()).or_default();
        entry.files.insert(file.to_path_buf());
        entry.members.insert(member.clone());
        entry
            .member_files
            .entry(member.clone())
            .or_default()
            .insert(file.to_path_buf());
        if line > 0 {
            entry
                .member_sites
                .entry(member)
                .or_default()
                .insert(EvidenceSite {
                    file: file.to_path_buf(),
                    line,
                    execution_eligible,
                });
        }
    }

    pub fn record_unresolved(
        &mut self,
        expression: impl Into<String>,
        reason: impl Into<String>,
        file: &Path,
        line: usize,
    ) {
        self.unresolved.insert(UnresolvedReference {
            file: file.to_path_buf(),
            line,
            expression: expression.into(),
            reason: reason.into(),
        });
    }

    pub fn record_symbol(&mut self, path: &ApiPath, kind: SymbolUseKind, file: &Path) {
        self.record_symbol_at(path, kind, file, 0);
    }

    pub fn record_symbol_at(
        &mut self,
        path: &ApiPath,
        kind: SymbolUseKind,
        file: &Path,
        line: usize,
    ) {
        self.record_symbol_site(path, kind, file, line, true);
    }

    pub fn record_symbol_site(
        &mut self,
        path: &ApiPath,
        kind: SymbolUseKind,
        file: &Path,
        line: usize,
        execution_eligible: bool,
    ) {
        let usage = self.symbols.entry(path.clone()).or_default();
        usage.files.insert(file.to_path_buf());
        usage.kinds.insert(kind);
        if line > 0 {
            usage.sites.insert(EvidenceSite {
                file: file.to_path_buf(),
                line,
                execution_eligible,
            });
        }
    }

    pub fn merge_referenced(&mut self, incoming: TestUsage) {
        self.merge(incoming, false);
    }

    pub fn merge_as_asserted(&mut self, asserted: TestUsage) {
        self.merge(asserted, true);
    }

    pub fn operation_count(&self) -> usize {
        self.classes
            .values()
            .map(|usage| usage.members.len())
            .sum::<usize>()
            + self
                .symbols
                .values()
                .map(|usage| usage.kinds.len())
                .sum::<usize>()
    }

    fn merge(&mut self, incoming_usage: TestUsage, assertion_linked: bool) {
        for (path, incoming) in incoming_usage.classes {
            let entry = self.classes.entry(path).or_default();
            entry.files.extend(incoming.files);
            entry.members.extend(incoming.members.iter().cloned());
            for (member, files) in incoming.member_files {
                entry.member_files.entry(member).or_default().extend(files);
            }
            for (member, sites) in incoming.member_sites {
                entry
                    .member_sites
                    .entry(member.clone())
                    .or_default()
                    .extend(sites.iter().cloned());
                if assertion_linked {
                    entry
                        .assertion_sites
                        .entry(member)
                        .or_default()
                        .extend(sites);
                }
            }
        }
        for (path, incoming) in incoming_usage.symbols {
            let entry = self.symbols.entry(path).or_default();
            entry.files.extend(incoming.files);
            entry.kinds.extend(incoming.kinds.iter().copied());
            entry.sites.extend(incoming.sites.iter().cloned());
            if assertion_linked {
                entry.assertion_sites.extend(incoming.sites);
            }
        }
        self.unresolved.extend(incoming_usage.unresolved);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SymbolUseKind {
    Call,
    Read,
}

#[derive(Debug, Default)]
pub struct SymbolUsage {
    pub files: HashSet<PathBuf>,
    pub kinds: HashSet<SymbolUseKind>,
    pub executed_kinds: HashSet<SymbolUseKind>,
    pub asserted_kinds: HashSet<SymbolUseKind>,
    pub sites: HashSet<EvidenceSite>,
    pub assertion_sites: HashSet<EvidenceSite>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EvidenceSite {
    pub file: PathBuf,
    pub line: usize,
    pub execution_eligible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnresolvedReference {
    pub file: PathBuf,
    pub line: usize,
    pub expression: String,
    pub reason: String,
}

#[derive(Debug)]
pub struct ClassTestCoverage {
    pub class_path: ApiPath,
    pub class_name: String,
    pub module_path: String,
    pub is_exercised: bool,
    pub constructor_exercised: bool,
    pub has_constructor: bool,
    pub exercised_members: Vec<String>,
    pub executed_members: Vec<String>,
    pub asserted_members: Vec<String>,
    pub unexercised_members: Vec<String>,
    pub total_members: usize,
    pub coverage_pct: f32,
    pub evidence_files: Vec<PathBuf>,
}

#[derive(Debug)]
pub struct TestCoverageReport {
    pub classes: Vec<ClassTestCoverage>,
    pub module_symbols: Vec<ModuleSymbolCoverage>,
    pub total_classes: usize,
    pub exercised_classes: usize,
    pub total_members: usize,
    pub exercised_members: usize,
    pub executed_members: usize,
    pub asserted_members: usize,
    pub execution_data_loaded: bool,
    pub total_module_symbols: usize,
    pub exercised_module_symbols: usize,
    pub unresolved: Vec<UnresolvedReference>,
}

#[derive(Debug)]
pub struct ModuleSymbolCoverage {
    pub path: ApiPath,
    pub kind: StubSymbolKind,
    pub is_exercised: bool,
    pub is_executed: bool,
    pub is_asserted: bool,
    pub evidence_files: Vec<PathBuf>,
}

impl TestCoverageReport {
    pub fn class_coverage_pct(&self) -> f32 {
        if self.total_classes == 0 {
            return 100.0;
        }
        (self.exercised_classes as f32 / self.total_classes as f32) * 100.0
    }

    pub fn member_coverage_pct(&self) -> f32 {
        if self.total_members == 0 {
            return 100.0;
        }
        (self.exercised_members as f32 / self.total_members as f32) * 100.0
    }
}
