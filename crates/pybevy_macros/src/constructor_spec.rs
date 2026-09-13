use proc_macro2::TokenStream;
use syn::{
    Attribute, Error, Expr, FnArg, GenericArgument, Ident, ImplItem, ItemImpl, Lit, LitStr, Pat,
    PathArguments, Result, Token, Type, UnOp,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
};

struct Options {
    name: LitStr,
    keyword_only: Vec<Ident>,
    conflicts: Vec<(Vec<Ident>, Vec<Ident>)>,
    complete: Vec<Vec<Ident>>,
}

impl Parse for Options {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let name = input.parse()?;
        let mut keyword_only = None;
        let mut conflicts = Vec::new();
        let mut complete = Vec::new();
        while !input.is_empty() {
            input.parse::<Token![,]>()?;
            if input.is_empty() {
                break;
            }
            let key: Ident = input.parse()?;
            match key.to_string().as_str() {
                "keyword_only" if keyword_only.is_none() => {
                    keyword_only = Some(parse_members(input)?)
                }
                "complete" => complete.push(parse_members(input)?),
                "conflicts" => {
                    let groups;
                    syn::parenthesized!(groups in input);
                    let left = parse_members(&groups)?;
                    groups.parse::<Token![,]>()?;
                    let right = parse_members(&groups)?;
                    if groups.peek(Token![,]) {
                        groups.parse::<Token![,]>()?;
                    }
                    if !groups.is_empty() {
                        return Err(groups.error("conflicts requires exactly two groups"));
                    }
                    conflicts.push((left, right));
                }
                _ => {
                    return Err(Error::new_spanned(
                        key,
                        "expected keyword_only (once), conflicts, or complete",
                    ));
                }
            }
        }
        Ok(Self {
            name,
            keyword_only: keyword_only.unwrap_or_default(),
            conflicts,
            complete,
        })
    }
}

fn parse_members(input: ParseStream<'_>) -> Result<Vec<Ident>> {
    let members;
    syn::parenthesized!(members in input);
    Ok(Punctuated::<Ident, Token![,]>::parse_terminated(&members)?
        .into_iter()
        .collect())
}

pub struct ParameterSpec {
    pub name: Ident,
    pub value_type: Type,
    pub default: Option<Expr>,
    pub default_text: Option<String>,
    pub optional: bool,
    pub expected: String,
}

pub struct ConstructorSpec {
    pub name: LitStr,
    pub parameters: Vec<ParameterSpec>,
    pub keyword_start: usize,
    pub conflicts: Vec<(Vec<Ident>, Vec<Ident>)>,
    pub complete: Vec<Vec<Ident>>,
    pub method_index: usize,
}

pub fn has_name(attr: &Attribute, name: &str) -> bool {
    attr.path().segments.last().is_some_and(|s| s.ident == name)
}

impl ConstructorSpec {
    pub fn parse(tokens: TokenStream, item: &ItemImpl) -> Result<Self> {
        let options: Options = syn::parse2(tokens)?;
        if item.trait_.is_some() || !item.generics.params.is_empty() {
            return Err(Error::new_spanned(
                item,
                "pyconstructor requires an inherent, non-generic impl",
            ));
        }
        if !item.attrs.iter().any(|a| has_name(a, "pymethods")) {
            return Err(Error::new_spanned(
                item,
                "put pyconstructor before #[pymethods]",
            ));
        }
        let methods: Vec<_> = item
            .items
            .iter()
            .enumerate()
            .filter_map(|(i, member)| match member {
                ImplItem::Fn(method) if method.attrs.iter().any(|a| has_name(a, "new")) => {
                    Some((i, method))
                }
                _ => None,
            })
            .collect();
        let [(method_index, method)] = methods.as_slice() else {
            return Err(Error::new_spanned(
                item,
                "pyconstructor requires exactly one #[new] method",
            ));
        };
        if method.attrs.iter().any(|a| has_name(a, "pyo3")) {
            return Err(Error::new_spanned(
                method,
                "pyconstructor generates the PyO3 signature; remove #[pyo3(...)]",
            ));
        }
        if !method.sig.generics.params.is_empty() || method.sig.asyncness.is_some() {
            return Err(Error::new_spanned(
                &method.sig,
                "pyconstructor requires a non-generic synchronous method",
            ));
        }
        let keyword_start = method
            .sig
            .inputs
            .len()
            .checked_sub(options.keyword_only.len())
            .ok_or_else(|| {
                Error::new_spanned(method, "keyword_only must name a parameter suffix")
            })?;
        let mut parameters = Vec::new();
        for (i, arg) in method.sig.inputs.iter().enumerate() {
            let FnArg::Typed(arg) = arg else {
                return Err(Error::new_spanned(
                    arg,
                    "constructor parameters cannot include self",
                ));
            };
            let Pat::Ident(pat) = &*arg.pat else {
                return Err(Error::new_spanned(
                    &arg.pat,
                    "constructor parameters must be plain identifiers",
                ));
            };
            if pat.by_ref.is_some()
                || pat.mutability.is_some()
                || pat.subpat.is_some()
                || pat.ident.to_string().starts_with("r#")
            {
                return Err(Error::new_spanned(
                    pat,
                    "constructor parameters must be plain identifiers",
                ));
            }
            let name = pat.ident.clone();
            if matches!(
                name.to_string().as_str(),
                "self_" | "pyself" | "slf" | "py" | "_py" | "cls" | "_cls"
            ) {
                return Err(Error::new_spanned(name, "reserved PyO3 parameter name"));
            }
            if parameters.iter().any(|p: &ParameterSpec| p.name == name) {
                return Err(Error::new_spanned(name, "duplicate constructor parameter"));
            }
            if i >= keyword_start && options.keyword_only[i - keyword_start] != name {
                return Err(Error::new_spanned(
                    name,
                    "keyword_only must match the parameter suffix in order",
                ));
            }
            let inner = option_inner(&arg.ty);
            let value_type = inner.unwrap_or(&arg.ty);
            if option_inner(value_type).is_some() {
                return Err(Error::new_spanned(
                    value_type,
                    "nested Option/nullable inputs require a handwritten binding",
                ));
            }
            let mut default = None;
            let mut expected = None;
            for attr in &arg.attrs {
                if has_name(attr, "default") && default.is_none() && inner.is_none() {
                    default = Some(constructor_default(attr, value_type)?);
                } else if has_name(attr, "expected") && expected.is_none() {
                    expected = Some(attr.parse_args::<LitStr>()?.value());
                } else {
                    return Err(Error::new_spanned(
                        attr,
                        "expected #[expected(\"Python type\")] or one #[default(...)] on a non-Option parameter",
                    ));
                }
            }
            let expected = expected
                .or_else(|| scalar_type_name(value_type).map(str::to_owned))
                .filter(|name| !name.trim().is_empty())
                .ok_or_else(|| {
                    Error::new_spanned(
                        value_type,
                        "non-scalar inputs need #[expected(\"Python type\")]",
                    )
                })?;
            let (default, default_text) = default.unzip();
            parameters.push(ParameterSpec {
                name,
                value_type: value_type.clone(),
                default,
                default_text,
                optional: inner.is_some(),
                expected,
            });
        }
        for (left, right) in &options.conflicts {
            validate_group(left, &parameters, 1)?;
            validate_group(right, &parameters, 1)?;
            if left.iter().any(|name| right.contains(name)) {
                return Err(Error::new_spanned(
                    method,
                    "conflicting groups cannot share a parameter",
                ));
            }
        }
        for group in &options.complete {
            validate_group(group, &parameters, 2)?;
        }
        Ok(Self {
            name: options.name,
            parameters,
            keyword_start,
            conflicts: options.conflicts,
            complete: options.complete,
            method_index: *method_index,
        })
    }

    pub fn text_signature(&self) -> String {
        let mut parts = Vec::new();
        for (i, parameter) in self.parameters.iter().enumerate() {
            if i == self.keyword_start {
                parts.push("*".to_owned());
            }
            let name = &parameter.name;
            parts.push(match &parameter.default_text {
                Some(default) => format!("{name}={default}"),
                None if parameter.optional => format!("{name}=..."),
                None => name.to_string(),
            });
        }
        format!("({})", parts.join(", "))
    }
}

fn validate_group(group: &[Ident], parameters: &[ParameterSpec], minimum: usize) -> Result<()> {
    if group.len() < minimum {
        return Err(Error::new(
            proc_macro2::Span::call_site(),
            format!("group requires at least {minimum} parameters"),
        ));
    }
    let mut previous = None;
    for name in group {
        let index = parameters
            .iter()
            .position(|p| p.name == *name)
            .ok_or_else(|| Error::new_spanned(name, "unknown constructor parameter in group"))?;
        if previous.is_some_and(|previous| previous >= index) {
            return Err(Error::new_spanned(
                name,
                "group parameters must be unique and in declaration order",
            ));
        }
        previous = Some(index);
    }
    Ok(())
}

fn scalar_type_name(ty: &Type) -> Option<&'static str> {
    let Type::Path(path) = ty else { return None };
    match path.path.get_ident()?.to_string().as_str() {
        "f32" | "f64" => Some("float"),
        "bool" => Some("bool"),
        "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "u8" | "u16" | "u32" | "u64" | "u128"
        | "usize" => Some("int"),
        _ => None,
    }
}

fn option_inner(ty: &Type) -> Option<&Type> {
    let Type::Path(path) = ty else { return None };
    let segment = path.path.segments.last()?;
    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        return None;
    };
    match (
        segment.ident.to_string().as_str(),
        args.args.len(),
        args.args.first(),
    ) {
        ("Option", 1, Some(GenericArgument::Type(inner))) => Some(inner),
        _ => None,
    }
}

fn constructor_default(attr: &Attribute, value_type: &Type) -> Result<(Expr, String)> {
    attr.parse_args_with(|input: ParseStream<'_>| {
        let value: Expr = input.parse()?;
        if !input.is_empty() {
            input.parse::<Token![,]>()?;
        }
        if input.is_empty() {
            let display = default_text(&value)?;
            return Ok((value, display));
        }
        let key: Ident = input.parse()?;
        if key != "text" {
            return Err(Error::new_spanned(
                key,
                "expected text = \"Python literal\"",
            ));
        }
        input.parse::<Token![=]>()?;
        let text: LitStr = input.parse()?;
        if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
        }
        let scalar = scalar_type_name(value_type).ok_or_else(|| {
            Error::new_spanned(value_type, "computed defaults require a scalar parameter")
        })?;
        let display = text.value();
        let valid = if scalar == "bool" {
            matches!(display.as_str(), "True" | "False")
        } else {
            syn::parse_str::<Expr>(&display)
                .ok()
                .and_then(|expr| default_text(&expr).ok())
                .is_some_and(|canonical| canonical == display)
                && (scalar != "int" || !display.contains(['.', 'e', 'E']))
        };
        if !valid {
            return Err(Error::new_spanned(
                text,
                "text must be a Python decimal numeric or boolean literal",
            ));
        }
        Ok((syn::parse_quote!(const { #value }), display))
    })
}

fn default_text(expr: &Expr) -> Result<String> {
    match expr {
        Expr::Lit(lit) if let Lit::Float(value) = &lit.lit => Ok(value.base10_digits().to_owned()),
        Expr::Lit(lit) if let Lit::Int(value) = &lit.lit => Ok(value.base10_digits().to_owned()),
        Expr::Lit(lit) if let Lit::Bool(value) = &lit.lit => {
            Ok(if value.value { "True" } else { "False" }.to_owned())
        }
        Expr::Unary(unary)
            if matches!(unary.op, UnOp::Neg(_))
                && matches!(&*unary.expr, Expr::Lit(lit) if matches!(lit.lit, Lit::Int(_) | Lit::Float(_))) =>
        {
            Ok(format!("-{}", default_text(&unary.expr)?))
        }
        _ => Err(Error::new_spanned(
            expr,
            "default must be a numeric or bool literal",
        )),
    }
}
