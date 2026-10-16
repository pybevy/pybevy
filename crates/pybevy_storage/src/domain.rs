//! Declarative numeric value domains shared by every write path.
//!
//! A `&'static [FieldDomain]` list states which values a field or parameter
//! accepts; every rule in the list must hold. The checks return a
//! [`DomainViolation`] whose `Display` text is the canonical user-facing
//! message, and adapters raise it as `ValueError`.

use std::{fmt, marker::PhantomData, num::NonZero, ops::Range, time::Duration};

use bevy::math::{IVec2, Quat, UVec2, UVec3, Vec2, Vec3, Vec3A, Vec4};
#[cfg(feature = "text")]
use bevy::text::FontSize;
#[cfg(feature = "ui")]
use bevy::ui::{UiRect, Val};
#[cfg(feature = "pyo3")]
use pyo3::{
    Borrowed, FromPyObject, PyAny, PyClass, PyErr,
    exceptions::{PyOverflowError, PyTypeError, PyValueError},
    types::{PyAnyMethods, PyDelta},
};

use crate::{StorageError, batch_columns::IntegerColumn, float_repr::float_repr};

/// One rule of a value domain. Every rule rejects NaN.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FieldDomain {
    /// Rejects both infinities.
    Finite,
    /// Accepts both infinities.
    NotNan,
    /// Accepts positive infinity, the "unbounded" sentinel.
    FiniteOrPosInf,
    /// `v >= 0`; accepts positive infinity and `-0.0`.
    NonNegative,
    /// Finite and `v > 0`.
    Positive,
    /// `v != 0`; accepts infinities.
    NonZero,
    /// `lo <= v < hi`.
    Range { lo: f64, hi: f64 },
    /// `lo <= v <= hi`.
    RangeInclusive { lo: f64, hi: f64 },
    /// `v >= n`.
    AtLeast(i64),
    /// On a range-shaped value, `start <= end`, or `start < end` when
    /// `strict`; NaN never orders. Single values ignore it.
    Ordered { strict: bool },
}

/// Rejects NaN and both infinities.
pub const FINITE: &[FieldDomain] = &[FieldDomain::Finite];
/// Rejects only NaN.
pub const NOT_NAN: &[FieldDomain] = &[FieldDomain::NotNan];
/// `v >= 0`, positive infinity included.
pub const NON_NEGATIVE: &[FieldDomain] = &[FieldDomain::NonNegative];
/// Finite and `v >= 0`.
pub const FINITE_NON_NEGATIVE: &[FieldDomain] = &[FieldDomain::Finite, FieldDomain::NonNegative];
/// Finite and `v > 0`.
pub const POSITIVE: &[FieldDomain] = &[FieldDomain::Positive];
/// Anything but zero, such as a one-based grid line.
pub const NON_ZERO: &[FieldDomain] = &[FieldDomain::NonZero];
/// A bit index of a `u64` mask.
pub const MASK_BIT_DOMAIN: &[FieldDomain] = &[FieldDomain::Range { lo: 0.0, hi: 64.0 }];

impl FieldDomain {
    fn accepts(self, value: DomainValue) -> bool {
        match value {
            DomainValue::F32(v) => self.accepts_float(f64::from(v)),
            DomainValue::F64(v) => self.accepts_float(v),
            DomainValue::Int(v) => self.accepts_int(v),
        }
    }

    fn accepts_float(self, v: f64) -> bool {
        match self {
            Self::Finite => v.is_finite(),
            Self::NotNan => !v.is_nan(),
            Self::FiniteOrPosInf => v.is_finite() || v == f64::INFINITY,
            Self::NonNegative => v >= 0.0,
            Self::Positive => v.is_finite() && v > 0.0,
            Self::NonZero => !v.is_nan() && v != 0.0,
            Self::Range { lo, hi } => lo <= v && v < hi,
            Self::RangeInclusive { lo, hi } => lo <= v && v <= hi,
            Self::AtLeast(n) => v >= n as f64,
            Self::Ordered { .. } => true,
        }
    }

    fn accepts_int(self, v: i128) -> bool {
        let wide = v as f64;
        match self {
            Self::Finite | Self::NotNan | Self::FiniteOrPosInf => true,
            Self::NonNegative => v >= 0,
            Self::Positive => v > 0,
            Self::NonZero => v != 0,
            Self::Range { lo, hi } => lo <= wide && wide < hi,
            Self::RangeInclusive { lo, hi } => lo <= wide && wide <= hi,
            Self::AtLeast(n) => v >= i128::from(n),
            Self::Ordered { .. } => true,
        }
    }

    /// Whether an integer value can break this rule.
    fn applies_to_int(self) -> bool {
        !matches!(self, Self::Finite | Self::NotNan | Self::FiniteOrPosInf)
    }

    /// The ordering a list of rules requires, if any.
    fn ordering(domains: &[Self]) -> Option<bool> {
        domains.iter().find_map(|domain| match domain {
            Self::Ordered { strict } => Some(*strict),
            _ => None,
        })
    }
}

/// A checked value, kept in its original type for faithful formatting.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DomainValue {
    F32(f32),
    F64(f64),
    Int(i128),
}

impl DomainValue {
    fn is_int(self) -> bool {
        matches!(self, Self::Int(_))
    }

    fn is_nan(self) -> bool {
        match self {
            Self::F32(v) => v.is_nan(),
            Self::F64(v) => v.is_nan(),
            Self::Int(_) => false,
        }
    }

    fn as_f64(self) -> f64 {
        match self {
            Self::F32(v) => f64::from(v),
            Self::F64(v) => v,
            Self::Int(v) => v as f64,
        }
    }

    /// `self <= other`, or `self < other` when `strict`; NaN never orders.
    fn orders_before(self, other: Self, strict: bool) -> bool {
        if let (Self::Int(a), Self::Int(b)) = (self, other) {
            return if strict { a < b } else { a <= b };
        }
        let (a, b) = (self.as_f64(), other.as_f64());
        if strict { a < b } else { a <= b }
    }
}

impl fmt::Display for DomainValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::F32(v) => f.write_str(&float_repr(*v)),
            Self::F64(v) => f.write_str(&float_repr(*v)),
            Self::Int(v) => write!(f, "{v}"),
        }
    }
}

macro_rules! domain_value_from {
    ($($variant:ident: $($ty:ty),+;)+) => {
        $($(
            impl From<$ty> for DomainValue {
                fn from(value: $ty) -> Self {
                    Self::$variant(value.into())
                }
            }
        )+)+
    };
}

domain_value_from! {
    F32: f32;
    F64: f64;
    Int: i8, i16, i32, i64, u8, u16, u32, u64;
}

impl From<usize> for DomainValue {
    fn from(value: usize) -> Self {
        Self::Int(value as i128)
    }
}

macro_rules! domain_value_from_nonzero {
    ($($ty:ty),+) => {
        $(
            impl From<NonZero<$ty>> for DomainValue {
                fn from(value: NonZero<$ty>) -> Self {
                    Self::from(value.get())
                }
            }
        )+
    };
}

domain_value_from_nonzero!(u8, u16, u32, u64, usize);

/// A value shape that can be checked against a domain: a scalar, a vector or
/// quaternion checked per component, or an `Option` whose `None` passes.
pub trait DomainValues {
    fn check_domains(
        &self,
        owner: &'static str,
        field: &'static str,
        domains: &'static [FieldDomain],
    ) -> Result<(), DomainViolation>;
}

macro_rules! scalar_domain_values {
    ($($ty:ty),+) => {
        $(
            impl DomainValues for $ty {
                fn check_domains(
                    &self,
                    owner: &'static str,
                    field: &'static str,
                    domains: &'static [FieldDomain],
                ) -> Result<(), DomainViolation> {
                    check_component(owner, field, None, (*self).into(), domains)
                }
            }
        )+
    };
}

scalar_domain_values!(f32, f64, i8, i16, i32, i64, u8, u16, u32, u64, usize);
scalar_domain_values!(
    NonZero<u8>,
    NonZero<u16>,
    NonZero<u32>,
    NonZero<u64>,
    NonZero<usize>
);

macro_rules! vector_domain_values {
    ($($ty:ty => [$($component:literal),+];)+) => {
        $(
            impl DomainValues for $ty {
                fn check_domains(
                    &self,
                    owner: &'static str,
                    field: &'static str,
                    domains: &'static [FieldDomain],
                ) -> Result<(), DomainViolation> {
                    check_components(owner, field, [$($component),+], self.to_array(), domains)
                        .map(drop)
                }
            }
        )+
    };
}

vector_domain_values! {
    Vec2 => ["x", "y"];
    Vec3 => ["x", "y", "z"];
    Vec3A => ["x", "y", "z"];
    Vec4 => ["x", "y", "z", "w"];
    Quat => ["x", "y", "z", "w"];
    IVec2 => ["x", "y"];
    UVec2 => ["x", "y"];
    UVec3 => ["x", "y", "z"];
}

impl DomainValues for DomainValue {
    fn check_domains(
        &self,
        owner: &'static str,
        field: &'static str,
        domains: &'static [FieldDomain],
    ) -> Result<(), DomainViolation> {
        check_component(owner, field, None, *self, domains)
    }
}

/// The magnitude of a `Val`; `Auto` has none and always passes.
#[cfg(feature = "ui")]
fn val_magnitude(value: Val) -> Option<f32> {
    match value {
        Val::Auto => None,
        Val::Px(v) | Val::Percent(v) | Val::Vw(v) | Val::Vh(v) | Val::VMin(v) | Val::VMax(v) => {
            Some(v)
        }
    }
}

#[cfg(feature = "ui")]
impl DomainValues for Val {
    fn check_domains(
        &self,
        owner: &'static str,
        field: &'static str,
        domains: &'static [FieldDomain],
    ) -> Result<(), DomainViolation> {
        val_magnitude(*self).map_or(Ok(()), |magnitude| {
            check_component(owner, field, None, magnitude.into(), domains)
        })
    }
}

/// Each side's magnitude, named by the side.
#[cfg(feature = "ui")]
impl DomainValues for UiRect {
    fn check_domains(
        &self,
        owner: &'static str,
        field: &'static str,
        domains: &'static [FieldDomain],
    ) -> Result<(), DomainViolation> {
        let sides = [
            ("left", self.left),
            ("right", self.right),
            ("top", self.top),
            ("bottom", self.bottom),
        ];
        for (side, value) in sides {
            if let Some(magnitude) = val_magnitude(value) {
                check_component(owner, field, Some(side), magnitude.into(), domains)?;
            }
        }
        Ok(())
    }
}

#[cfg(feature = "text")]
impl DomainValues for FontSize {
    fn check_domains(
        &self,
        owner: &'static str,
        field: &'static str,
        domains: &'static [FieldDomain],
    ) -> Result<(), DomainViolation> {
        let (FontSize::Px(magnitude)
        | FontSize::Vw(magnitude)
        | FontSize::Vh(magnitude)
        | FontSize::VMin(magnitude)
        | FontSize::VMax(magnitude)
        | FontSize::Rem(magnitude)) = *self;
        check_component(owner, field, None, magnitude.into(), domains)
    }
}

/// A Python float bound for an `f32` field, checked before it narrows so a
/// finite value too large for `f32` is rejected instead of becoming infinity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct F32Input(pub f64);

impl F32Input {
    /// The value as the field stores it.
    pub fn narrowed(self) -> f32 {
        self.0 as f32
    }
}

impl DomainValues for F32Input {
    fn check_domains(
        &self,
        owner: &'static str,
        field: &'static str,
        domains: &'static [FieldDomain],
    ) -> Result<(), DomainViolation> {
        check_f64_as_f32(owner, field, self.0, domains).map(drop)
    }
}

/// An integer read wide from Python, so a value its field's type cannot hold
/// reaches the domain check instead of failing extraction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WideInteger {
    Value(i128),
    /// Wider than 128 bits.
    Beyond,
}

impl fmt::Display for WideInteger {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Value(value) => write!(f, "{value}"),
            Self::Beyond => f.write_str("an integer wider than 128 bits"),
        }
    }
}

/// An integer type a field stores, with the range messages name.
pub trait StoredInteger: Copy + TryFrom<i128> {
    const COLUMN: IntegerColumn;
}

impl StoredInteger for i32 {
    const COLUMN: IntegerColumn = IntegerColumn::I32;
}

impl StoredInteger for i64 {
    const COLUMN: IntegerColumn = IntegerColumn::I64;
}

impl StoredInteger for u32 {
    const COLUMN: IntegerColumn = IntegerColumn::U32;
}

impl StoredInteger for u64 {
    const COLUMN: IntegerColumn = IntegerColumn::U64;
}

/// A Python integer bound for a field of type `T`. The check applies the
/// declared rules to the wide value, then `T`'s range, so `-1` for a positive
/// `u32` reports the rule and `2**40` the 32-bit range, both as `ValueError`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntegerInput<T> {
    value: WideInteger,
    stored: PhantomData<T>,
}

impl<T: StoredInteger> IntegerInput<T> {
    pub fn new(value: i128) -> Self {
        Self {
            value: WideInteger::Value(value),
            stored: PhantomData,
        }
    }

    #[cfg(any(feature = "pyo3", test))]
    fn beyond() -> Self {
        Self {
            value: WideInteger::Beyond,
            stored: PhantomData,
        }
    }

    /// The value as the field stores it; call after a successful check.
    pub fn stored(self) -> T {
        match self.value {
            WideInteger::Value(value) => T::try_from(value)
                .unwrap_or_else(|_| panic!("IntegerInput::stored before its range check")),
            WideInteger::Beyond => panic!("IntegerInput::stored before its range check"),
        }
    }
}

impl<T: StoredInteger> DomainValues for IntegerInput<T> {
    fn check_domains(
        &self,
        owner: &'static str,
        field: &'static str,
        domains: &'static [FieldDomain],
    ) -> Result<(), DomainViolation> {
        if let WideInteger::Value(value) = self.value {
            check_component(owner, field, None, DomainValue::Int(value), domains)?;
            if T::try_from(value).is_ok() {
                return Ok(());
            }
        }
        Err(DomainViolation::new(
            owner,
            field,
            None,
            ViolationKind::IntegerRange {
                column: T::COLUMN,
                value: self.value,
            },
        ))
    }
}

#[cfg(feature = "pyo3")]
impl<T: StoredInteger> FromPyObject<'_, '_> for IntegerInput<T> {
    type Error = PyErr;

    fn extract(value: Borrowed<'_, '_, PyAny>) -> Result<Self, PyErr> {
        match value.extract::<i128>() {
            Ok(value) => Ok(Self::new(value)),
            Err(error) if error.is_instance_of::<PyOverflowError>(value.py()) => Ok(Self::beyond()),
            Err(error) => Err(error),
        }
    }
}

/// The Python types a `Duration` input accepts.
pub const DURATION_INPUT_TYPES: &str =
    "Duration must be a Duration object, float (seconds), or int (seconds)";

impl DomainValues for Duration {
    fn check_domains(
        &self,
        owner: &'static str,
        field: &'static str,
        domains: &'static [FieldDomain],
    ) -> Result<(), DomainViolation> {
        check_component(owner, field, None, self.as_secs_f64().into(), domains)
    }
}

/// A Python duration bound for a `Duration` field. A `timedelta` or integer
/// seconds converts exactly; other seconds are checked before conversion, so a
/// negative, non-finite or too-large value reaches the domain check. Seconds
/// that round to a `Duration` the rules reject, such as a positive subnormal
/// for a `positive` domain, are out of the supported duration range.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DurationInput {
    Exact(Duration),
    /// Integer seconds, reported as an integer.
    WholeSeconds(i128),
    Seconds(f64),
}

impl DurationInput {
    /// The value as the field stores it; call after a successful check.
    pub fn stored(self) -> Duration {
        fn unchecked() -> ! {
            panic!("DurationInput::stored before its range check")
        }
        match self {
            Self::Exact(duration) => duration,
            Self::WholeSeconds(seconds) => {
                Duration::from_secs(u64::try_from(seconds).unwrap_or_else(|_| unchecked()))
            }
            Self::Seconds(seconds) => {
                Duration::try_from_secs_f64(seconds).unwrap_or_else(|_| unchecked())
            }
        }
    }
}

impl DomainValues for DurationInput {
    fn check_domains(
        &self,
        owner: &'static str,
        field: &'static str,
        domains: &'static [FieldDomain],
    ) -> Result<(), DomainViolation> {
        let (value, duration) = match *self {
            Self::Exact(duration) => return duration.check_domains(owner, field, domains),
            Self::WholeSeconds(seconds) => (
                DomainValue::Int(seconds),
                u64::try_from(seconds).ok().map(Duration::from_secs),
            ),
            Self::Seconds(seconds) => (
                DomainValue::F64(seconds),
                Duration::try_from_secs_f64(seconds).ok(),
            ),
        };
        check_component(owner, field, None, value, domains)?;
        if duration.is_some_and(|duration| duration.check_domains(owner, field, domains).is_ok()) {
            return Ok(());
        }
        Err(DomainViolation::new(
            owner,
            field,
            None,
            ViolationKind::DurationRange { value },
        ))
    }
}

#[cfg(feature = "pyo3")]
impl FromPyObject<'_, '_> for DurationInput {
    type Error = PyErr;

    fn extract(value: Borrowed<'_, '_, PyAny>) -> Result<Self, PyErr> {
        if let Ok(duration) = value.extract::<Duration>() {
            return Ok(Self::Exact(duration));
        }
        // Only a negative timedelta fails the exact conversion.
        if let Ok(delta) = value.cast::<PyDelta>() {
            return Ok(Self::Seconds(
                delta.call_method0("total_seconds")?.extract()?,
            ));
        }
        if let Ok(seconds) = value.extract::<i128>() {
            return Ok(Self::WholeSeconds(seconds));
        }
        if let Ok(seconds) = value.extract::<f64>() {
            return Ok(Self::Seconds(seconds));
        }
        Err(PyTypeError::new_err(DURATION_INPUT_TYPES))
    }
}

/// A `timedelta` bound for a `Duration` parameter that takes no seconds. A
/// negative `timedelta` reaches the domain check instead of failing conversion.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimedeltaInput(pub DurationInput);

impl TimedeltaInput {
    /// The value as the field stores it; call after a successful check.
    pub fn stored(self) -> Duration {
        self.0.stored()
    }
}

impl DomainValues for TimedeltaInput {
    fn check_domains(
        &self,
        owner: &'static str,
        field: &'static str,
        domains: &'static [FieldDomain],
    ) -> Result<(), DomainViolation> {
        self.0.check_domains(owner, field, domains)
    }
}

#[cfg(feature = "pyo3")]
impl FromPyObject<'_, '_> for TimedeltaInput {
    type Error = PyErr;

    fn extract(value: Borrowed<'_, '_, PyAny>) -> Result<Self, PyErr> {
        let delta = value.cast::<PyDelta>()?;
        if let Ok(duration) = delta.extract::<Duration>() {
            return Ok(Self(DurationInput::Exact(duration)));
        }
        Ok(Self(DurationInput::Seconds(
            delta.call_method0("total_seconds")?.extract()?,
        )))
    }
}

/// Both ends, named `start` and `end`, then the ordering the rules require.
impl<T: Copy + Into<DomainValue>> DomainValues for Range<T> {
    fn check_domains(
        &self,
        owner: &'static str,
        field: &'static str,
        domains: &'static [FieldDomain],
    ) -> Result<(), DomainViolation> {
        let (start, end) = (self.start.into(), self.end.into());
        check_component(owner, field, Some("start"), start, domains)?;
        check_component(owner, field, Some("end"), end, domains)?;
        let Some(strict) = FieldDomain::ordering(domains) else {
            return Ok(());
        };
        ordering_violation(owner, field, Some("start"), start, "end", end, strict)
    }
}

impl<T: DomainValues> DomainValues for Option<T> {
    fn check_domains(
        &self,
        owner: &'static str,
        field: &'static str,
        domains: &'static [FieldDomain],
    ) -> Result<(), DomainViolation> {
        self.as_ref()
            .map_or(Ok(()), |value| value.check_domains(owner, field, domains))
    }
}

/// One declared field domain, as registered for write paths that do not go
/// through the wrapper's own setters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FieldDomainEntry {
    pub owner: &'static str,
    pub field: &'static str,
    pub domains: &'static [FieldDomain],
}

/// What a [`DomainViolation`] reports.
#[derive(Debug, Clone, PartialEq)]
pub enum ViolationKind {
    /// The value breaks at least one rule of `domains`.
    Domain {
        domains: &'static [FieldDomain],
        value: DomainValue,
    },
    /// A finite `f64` falls outside the range an `f32` can represent.
    F32Range { value: f64 },
    /// An integer falls outside the range of the type the field stores.
    IntegerRange {
        column: IntegerColumn,
        value: WideInteger,
    },
    /// Seconds that no `Duration` can represent.
    DurationRange { value: DomainValue },
    /// The field must not exceed `greater`, or must be less than it when
    /// `strict`.
    Ordering {
        greater: &'static str,
        strict: bool,
        value: DomainValue,
        greater_value: DomainValue,
    },
}

/// The context of a rejected value, enough to produce the canonical message.
#[derive(Debug, Clone, PartialEq)]
pub struct ViolationDetails {
    /// Python-facing owner type, or empty for a free parameter.
    pub owner: &'static str,
    pub field: &'static str,
    /// Vector or range component, such as `x` or `start`.
    pub component: Option<&'static str>,
    /// Batch or column row of the first violation.
    pub row: Option<usize>,
    pub kind: ViolationKind,
}

/// A rejected value. Boxed so that successful checks stay small.
#[derive(Debug, Clone, PartialEq)]
pub struct DomainViolation(Box<ViolationDetails>);

impl DomainViolation {
    fn new(
        owner: &'static str,
        field: &'static str,
        component: Option<&'static str>,
        kind: ViolationKind,
    ) -> Self {
        Self(Box::new(ViolationDetails {
            owner,
            field,
            component,
            row: None,
            kind,
        }))
    }

    pub fn details(&self) -> &ViolationDetails {
        &self.0
    }

    /// Attach the row of a batch or column write.
    pub fn at_row(mut self, row: usize) -> Self {
        self.0.row = Some(row);
        self
    }
}

impl fmt::Display for DomainViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let details = self.details();
        if !details.owner.is_empty() {
            write!(f, "{}.", details.owner)?;
        }
        f.write_str(details.field)?;
        if let Some(component) = details.component {
            write!(f, ".{component}")?;
        }
        match &details.kind {
            ViolationKind::Domain { domains, value } => {
                f.write_str(" must be ")?;
                write_requirements(f, domains, value.is_int())?;
                write!(f, " (got {value}")?;
            }
            ViolationKind::F32Range { value } => write!(
                f,
                " must be within the 32-bit float range (got {}",
                float_repr(*value)
            )?,
            ViolationKind::IntegerRange { column, value } => write!(
                f,
                " must be within the {} range (got {value}",
                column.range_name()
            )?,
            ViolationKind::DurationRange { value } => write!(
                f,
                " must be within the supported duration range (got {value}"
            )?,
            ViolationKind::Ordering {
                greater,
                strict,
                value,
                greater_value,
            } => {
                let (relation, shown) = if *strict {
                    ("be less than", ">=")
                } else {
                    ("not exceed", ">")
                };
                // A range's end is named with its field: `depth.start ... depth.end`.
                if details.component.is_some() {
                    write!(
                        f,
                        " must {relation} {}.{greater} (got {value}",
                        details.field
                    )?;
                } else {
                    write!(f, " must {relation} {greater} (got {value}")?;
                }
                if value.is_nan() || greater_value.is_nan() {
                    write!(f, " and {greater_value}")?;
                } else {
                    write!(f, " {shown} {greater_value}")?;
                }
            }
        }
        if let Some(row) = details.row {
            write!(f, " at row {row}")?;
        }
        f.write_str(")")
    }
}

fn write_requirements(
    f: &mut fmt::Formatter<'_>,
    domains: &[FieldDomain],
    int: bool,
) -> fmt::Result {
    let has_positive = domains.contains(&FieldDomain::Positive);
    let mut first = true;
    for &domain in domains {
        let implied = domain == FieldDomain::Finite && has_positive;
        let ordering = matches!(domain, FieldDomain::Ordered { .. });
        if (int && !domain.applies_to_int()) || implied || ordering {
            continue;
        }
        if !first {
            f.write_str(" and ")?;
        }
        first = false;
        match domain {
            FieldDomain::Finite => f.write_str("finite")?,
            FieldDomain::NotNan => f.write_str("a number, not NaN")?,
            FieldDomain::FiniteOrPosInf => f.write_str("finite or positive infinity")?,
            FieldDomain::NonNegative => f.write_str("non-negative")?,
            FieldDomain::Positive if int => f.write_str("greater than zero")?,
            FieldDomain::Positive => f.write_str("finite and greater than zero")?,
            FieldDomain::NonZero => f.write_str("non-zero")?,
            FieldDomain::Range { lo, hi } => {
                write!(f, "in [{}, {})", bound(lo, int), bound(hi, int))?;
            }
            FieldDomain::RangeInclusive { lo, hi } => {
                write!(f, "in [{}, {}]", bound(lo, int), bound(hi, int))?;
            }
            FieldDomain::AtLeast(n) => write!(f, "at least {n}")?,
            FieldDomain::Ordered { .. } => {}
        }
    }
    Ok(())
}

fn bound(value: f64, int: bool) -> String {
    if int && value.fract() == 0.0 && value.abs() < 1e18 {
        format!("{}", value as i64)
    } else {
        float_repr(value)
    }
}

impl std::error::Error for DomainViolation {}

impl From<DomainViolation> for StorageError {
    fn from(violation: DomainViolation) -> Self {
        StorageError::InvalidValue(violation.to_string())
    }
}

#[cfg(feature = "pyo3")]
impl From<DomainViolation> for PyErr {
    fn from(violation: DomainViolation) -> Self {
        PyValueError::new_err(violation.to_string())
    }
}

fn domain_violation(
    owner: &'static str,
    field: &'static str,
    component: Option<&'static str>,
    domains: &'static [FieldDomain],
    value: DomainValue,
) -> DomainViolation {
    DomainViolation::new(
        owner,
        field,
        component,
        ViolationKind::Domain { domains, value },
    )
}

fn check_component(
    owner: &'static str,
    field: &'static str,
    component: Option<&'static str>,
    value: DomainValue,
    domains: &'static [FieldDomain],
) -> Result<(), DomainViolation> {
    if domains.iter().all(|domain| domain.accepts(value)) {
        Ok(())
    } else {
        Err(domain_violation(owner, field, component, domains, value))
    }
}

/// Check a value of any supported shape against a domain.
pub fn check<T: DomainValues>(
    owner: &'static str,
    field: &'static str,
    value: T,
    domains: &'static [FieldDomain],
) -> Result<T, DomainViolation> {
    value.check_domains(owner, field, domains)?;
    Ok(value)
}

/// [`check`] with the owner named by `O`'s Python class name.
#[cfg(feature = "pyo3")]
pub fn check_for<O: PyClass, T: DomainValues>(
    field: &'static str,
    value: T,
    domains: &'static [FieldDomain],
) -> Result<T, DomainViolation> {
    check(<O as PyClass>::NAME, field, value, domains)
}

/// Check a value of any supported shape in place.
pub fn check_ref<T: DomainValues + ?Sized>(
    owner: &'static str,
    field: &'static str,
    value: &T,
    domains: &'static [FieldDomain],
) -> Result<(), DomainViolation> {
    value.check_domains(owner, field, domains)
}

/// Check a Python float bound for `f32` storage before it narrows, so a finite
/// value too large for `f32` is rejected instead of becoming an infinity.
pub fn check_f64_as_f32(
    owner: &'static str,
    field: &'static str,
    value: f64,
    domains: &'static [FieldDomain],
) -> Result<f32, DomainViolation> {
    check(owner, field, value, domains)?;
    let narrowed = value as f32;
    if value.is_finite() && !narrowed.is_finite() {
        return Err(DomainViolation::new(
            owner,
            field,
            None,
            ViolationKind::F32Range { value },
        ));
    }
    Ok(narrowed)
}

/// Check every component of a vector or range value, naming the first one
/// that fails.
pub fn check_components<T: Copy + Into<DomainValue>, const N: usize>(
    owner: &'static str,
    field: &'static str,
    components: [&'static str; N],
    values: [T; N],
    domains: &'static [FieldDomain],
) -> Result<[T; N], DomainViolation> {
    for (component, value) in components.into_iter().zip(values) {
        check_component(owner, field, Some(component), value.into(), domains)?;
    }
    Ok(values)
}

/// Check a flattened column of rows. `components` names the values of one
/// row for vector fields and is empty for scalar fields.
pub fn check_column<T: Copy + Into<DomainValue>>(
    owner: &'static str,
    field: &'static str,
    components: &'static [&'static str],
    values: &[T],
    domains: &'static [FieldDomain],
) -> Result<(), DomainViolation> {
    let width = components.len().max(1);
    for (index, &value) in values.iter().enumerate() {
        let component = components.get(index % width).copied();
        check_component(owner, field, component, value.into(), domains)
            .map_err(|violation| violation.at_row(index / width))?;
    }
    Ok(())
}

/// Check that `field` does not exceed `greater`, or is less than it when
/// `strict`. NaN on either side fails.
pub fn check_ordered<T: Copy + Into<DomainValue>>(
    owner: &'static str,
    field: &'static str,
    value: T,
    greater: &'static str,
    greater_value: T,
    strict: bool,
) -> Result<(), DomainViolation> {
    ordering_violation(
        owner,
        field,
        None,
        value.into(),
        greater,
        greater_value.into(),
        strict,
    )
}

/// [`check_ordered`] with the owner named by `O`'s Python class name.
#[cfg(feature = "pyo3")]
pub fn check_ordered_for<O: PyClass, T: Copy + Into<DomainValue>>(
    field: &'static str,
    value: T,
    greater: &'static str,
    greater_value: T,
    strict: bool,
) -> Result<(), DomainViolation> {
    check_ordered(
        <O as PyClass>::NAME,
        field,
        value,
        greater,
        greater_value,
        strict,
    )
}

fn ordering_violation(
    owner: &'static str,
    field: &'static str,
    component: Option<&'static str>,
    value: DomainValue,
    greater: &'static str,
    greater_value: DomainValue,
    strict: bool,
) -> Result<(), DomainViolation> {
    if value.orders_before(greater_value, strict) {
        return Ok(());
    }
    Err(DomainViolation::new(
        owner,
        field,
        component,
        ViolationKind::Ordering {
            greater,
            strict,
            value,
            greater_value,
        },
    ))
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use std::time::Duration;

    use bevy::math::{Quat, Vec2, Vec3, Vec3A, Vec4};
    #[cfg(feature = "text")]
    use bevy::text::FontSize;
    #[cfg(feature = "ui")]
    use bevy::ui::{UiRect, Val};

    use super::{
        DomainValue, DomainViolation, DurationInput, F32Input, FINITE, FINITE_NON_NEGATIVE,
        FieldDomain, IntegerInput, NON_NEGATIVE, NON_ZERO, NOT_NAN, POSITIVE, ViolationDetails,
        ViolationKind, check, check_column, check_components, check_f64_as_f32, check_ordered,
    };
    use crate::StorageError;

    const UNBOUNDED: &[FieldDomain] = &[FieldDomain::FiniteOrPosInf];
    const FINITE_POSITIVE: &[FieldDomain] = &[FieldDomain::Finite, FieldDomain::Positive];
    const HALF_OPEN: &[FieldDomain] = &[FieldDomain::Range { lo: 0.0, hi: 1.0 }];
    const CLOSED: &[FieldDomain] = &[FieldDomain::RangeInclusive { lo: -1.0, hi: 1.0 }];
    const AT_LEAST_ONE: &[FieldDomain] = &[FieldDomain::AtLeast(1)];
    const SLOTS: &[FieldDomain] = &[FieldDomain::RangeInclusive {
        lo: 1.0,
        hi: 4096.0,
    }];

    fn accepted(domains: &'static [FieldDomain], values: &[f64]) -> Vec<bool> {
        values
            .iter()
            .map(|&value| check("T", "f", value, domains).is_ok())
            .collect()
    }

    fn message<T>(result: Result<T, DomainViolation>) -> String {
        match result {
            Ok(_) => panic!("expected a domain violation"),
            Err(violation) => violation.to_string(),
        }
    }

    #[test]
    fn float_rules_treat_nan_infinities_and_negative_zero_as_documented() {
        let probes = [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -0.0, 0.0, 1.0];
        assert_eq!(
            accepted(FINITE, &probes),
            [false, false, false, true, true, true]
        );
        assert_eq!(
            accepted(NOT_NAN, &probes),
            [false, true, true, true, true, true]
        );
        assert_eq!(
            accepted(UNBOUNDED, &probes),
            [false, true, false, true, true, true]
        );
        assert_eq!(
            accepted(NON_NEGATIVE, &probes),
            [false, true, false, true, true, true]
        );
        assert_eq!(
            accepted(POSITIVE, &probes),
            [false, false, false, false, false, true]
        );
        assert_eq!(
            accepted(NON_ZERO, &probes),
            [false, true, true, false, false, true]
        );
        assert_eq!(
            message(check("G", "start", 0_i16, NON_ZERO)),
            "G.start must be non-zero (got 0)"
        );
        assert_eq!(
            accepted(HALF_OPEN, &probes),
            [false, false, false, true, true, false]
        );
        assert_eq!(
            accepted(CLOSED, &probes),
            [false, false, false, true, true, true]
        );
        assert_eq!(
            accepted(AT_LEAST_ONE, &probes),
            [false, true, false, false, false, true]
        );
        assert_eq!(
            accepted(CLOSED, &[-1.0, -1.0001, 1.0001]),
            [true, false, false]
        );
    }

    #[test]
    fn integer_rules_ignore_float_only_requirements() {
        assert!(check("T", "n", 0_u32, FINITE).is_ok());
        assert!(check("T", "n", -1_i32, NON_NEGATIVE).is_err());
        assert!(check("T", "n", 0_u32, POSITIVE).is_err());
        assert!(check("T", "n", 1_u32, POSITIVE).is_ok());
        assert!(check("T", "n", 0_usize, AT_LEAST_ONE).is_err());
        assert_eq!(check("T", "n", 4096_u32, SLOTS), Ok(4096));
        assert!(check("T", "n", 4097_u32, SLOTS).is_err());
        assert!(check("T", "n", u64::MAX, NON_NEGATIVE).is_ok());
    }

    #[test]
    fn f32_values_are_checked_and_returned_unchanged() {
        assert_eq!(check("T", "f", 0.5_f32, HALF_OPEN), Ok(0.5));
        assert!(check("T", "f", f32::NAN, NOT_NAN).is_err());
        assert!(check("T", "f", f32::INFINITY, UNBOUNDED).is_ok());
    }

    #[test]
    fn messages_name_owner_field_requirement_and_python_value() {
        assert_eq!(
            message(check("Exposure", "ev100", f32::NAN, FINITE)),
            "Exposure.ev100 must be finite (got nan)"
        );
        assert_eq!(
            message(check("UiScale", "scale", 0.0_f32, FINITE_POSITIVE)),
            "UiScale.scale must be finite and greater than zero (got 0.0)"
        );
        assert_eq!(
            message(check("Sphere", "radius", -1.5_f32, FINITE_NON_NEGATIVE)),
            "Sphere.radius must be finite and non-negative (got -1.5)"
        );
        assert_eq!(
            message(check("FogVolume", "scattering_asymmetry", 2.0_f32, CLOSED)),
            "FogVolume.scattering_asymmetry must be in [-1.0, 1.0] (got 2.0)"
        );
        assert_eq!(
            message(check(
                "CascadeShadowConfig",
                "overlap_proportion",
                1.0_f32,
                HALF_OPEN
            )),
            "CascadeShadowConfig.overlap_proportion must be in [0.0, 1.0) (got 1.0)"
        );
        assert_eq!(
            message(check("TextBounds", "width", f32::NAN, NOT_NAN)),
            "TextBounds.width must be a number, not NaN (got nan)"
        );
        assert_eq!(
            message(check("W", "max_width", f64::NEG_INFINITY, UNBOUNDED)),
            "W.max_width must be finite or positive infinity (got -inf)"
        );
        assert_eq!(
            message(check("", "timeout", f64::INFINITY, FINITE_POSITIVE)),
            "timeout must be finite and greater than zero (got inf)"
        );
    }

    #[test]
    fn integer_messages_drop_float_wording_and_print_integer_bounds() {
        assert_eq!(
            message(check("Bloom", "max_mip_dimension", 0_u32, FINITE_POSITIVE)),
            "Bloom.max_mip_dimension must be greater than zero (got 0)"
        );
        assert_eq!(
            message(check("TensorPort", "slots", 5000_u32, SLOTS)),
            "TensorPort.slots must be in [1, 4096] (got 5000)"
        );
        assert_eq!(
            message(check(
                "RegularPolygon",
                "sides",
                2_u32,
                &[FieldDomain::AtLeast(3)]
            )),
            "RegularPolygon.sides must be at least 3 (got 2)"
        );
    }

    #[test]
    fn narrowing_rejects_finite_values_outside_f32_range() {
        assert_eq!(
            check_f64_as_f32("W", "max_width", 1.5, UNBOUNDED),
            Ok(1.5_f32)
        );
        assert_eq!(
            check_f64_as_f32("W", "max_width", f64::INFINITY, UNBOUNDED),
            Ok(f32::INFINITY)
        );
        let too_large = check_f64_as_f32("W", "max_width", 1e39, UNBOUNDED);
        assert!(matches!(
            too_large.as_ref().map_err(DomainViolation::details),
            Err(ViolationDetails {
                kind: ViolationKind::F32Range { .. },
                ..
            })
        ));
        assert_eq!(
            message(too_large),
            "W.max_width must be within the 32-bit float range (got 1e+39)"
        );
        assert_eq!(
            message(check_f64_as_f32("W", "min_width", f64::NAN, FINITE)),
            "W.min_width must be finite (got nan)"
        );
    }

    #[test]
    fn integer_input_checks_rules_then_the_stored_range() {
        assert_eq!(
            check("B", "mips", IntegerInput::<u32>::new(512), POSITIVE).map(IntegerInput::stored),
            Ok(512)
        );
        assert_eq!(
            message(check("B", "mips", IntegerInput::<u32>::new(-1), POSITIVE)),
            "B.mips must be greater than zero (got -1)"
        );
        assert_eq!(
            message(check(
                "B",
                "mips",
                IntegerInput::<u32>::new(1 << 40),
                POSITIVE
            )),
            "B.mips must be within the 32-bit unsigned integer range (got 1099511627776)"
        );
        assert_eq!(
            message(check(
                "W",
                "latency",
                Some(IntegerInput::<u32>::new(0)),
                AT_LEAST_ONE
            )),
            "W.latency must be at least 1 (got 0)"
        );
        assert_eq!(
            message(check(
                "W",
                "latency",
                IntegerInput::<u32>::beyond(),
                AT_LEAST_ONE
            )),
            "W.latency must be within the 32-bit unsigned integer range \
             (got an integer wider than 128 bits)"
        );
        assert_eq!(
            check("W", "latency", None::<IntegerInput<u32>>, AT_LEAST_ONE),
            Ok(None)
        );
    }

    #[test]
    fn duration_input_checks_rules_then_the_duration_range() {
        assert_eq!(
            check(
                "P",
                "duration",
                DurationInput::Seconds(0.25),
                FINITE_NON_NEGATIVE
            )
            .map(DurationInput::stored),
            Ok(Duration::from_millis(250))
        );
        assert_eq!(
            check(
                "P",
                "duration",
                DurationInput::Exact(Duration::MAX),
                FINITE_NON_NEGATIVE
            )
            .map(DurationInput::stored),
            Ok(Duration::MAX)
        );
        assert_eq!(
            message(check(
                "P",
                "duration",
                DurationInput::Seconds(-1.0),
                FINITE_NON_NEGATIVE
            )),
            "P.duration must be finite and non-negative (got -1.0)"
        );
        assert_eq!(
            message(check(
                "P",
                "duration",
                DurationInput::Seconds(1e20),
                FINITE_NON_NEGATIVE
            )),
            "P.duration must be within the supported duration range (got 1e+20)"
        );
        assert_eq!(
            message(check(
                "E",
                "period",
                DurationInput::Exact(Duration::ZERO),
                POSITIVE
            )),
            "E.period must be finite and greater than zero (got 0.0)"
        );
        assert_eq!(
            message(check(
                "E",
                "period",
                DurationInput::Seconds(1e-320),
                POSITIVE
            )),
            "E.period must be within the supported duration range (got 1e-320)"
        );
        assert_eq!(
            check(
                "T",
                "delay",
                DurationInput::WholeSeconds(3),
                FINITE_NON_NEGATIVE
            )
            .map(DurationInput::stored),
            Ok(Duration::from_secs(3))
        );
        assert_eq!(
            message(check(
                "T",
                "delay",
                DurationInput::WholeSeconds(-1),
                FINITE_NON_NEGATIVE
            )),
            "T.delay must be non-negative (got -1)"
        );
        assert_eq!(
            message(check(
                "T",
                "delay",
                DurationInput::WholeSeconds(1 << 70),
                FINITE_NON_NEGATIVE
            )),
            "T.delay must be within the supported duration range (got 1180591620717411303424)"
        );
        assert_eq!(
            message(check(
                "E",
                "period",
                DurationInput::WholeSeconds(0),
                POSITIVE
            )),
            "E.period must be greater than zero (got 0)"
        );
        assert_eq!(
            check(
                "T",
                "delta",
                DurationInput::Seconds(1e-320),
                FINITE_NON_NEGATIVE
            )
            .map(DurationInput::stored),
            Ok(Duration::ZERO)
        );
    }

    #[cfg(feature = "ui")]
    #[test]
    fn val_and_ui_rect_check_magnitudes_and_skip_auto() {
        assert_eq!(check("N", "width", Val::Auto, FINITE), Ok(Val::Auto));
        assert_eq!(
            message(check("N", "width", Val::Percent(f32::NAN), FINITE)),
            "N.width must be finite (got nan)"
        );
        let rect = UiRect {
            top: Val::Auto,
            bottom: Val::Vh(-2.0),
            ..UiRect::px(1.0, 2.0, 0.0, 0.0)
        };
        assert_eq!(
            message(check("N", "padding", rect, NON_NEGATIVE)),
            "N.padding.bottom must be non-negative (got -2.0)"
        );
        assert_eq!(
            check("N", "padding", UiRect::AUTO, NON_NEGATIVE),
            Ok(UiRect::AUTO)
        );
    }

    #[cfg(feature = "text")]
    #[test]
    fn font_size_checks_every_variant_magnitude() {
        assert!(check("T", "font_size", FontSize::Rem(1.5), FINITE_NON_NEGATIVE).is_ok());
        assert_eq!(
            message(check(
                "T",
                "font_size",
                FontSize::VMax(-1.0),
                FINITE_NON_NEGATIVE
            )),
            "T.font_size must be finite and non-negative (got -1.0)"
        );
    }

    #[test]
    fn f32_input_narrows_after_the_checks() {
        assert_eq!(
            check("W", "max_width", F32Input(2.5), UNBOUNDED).map(F32Input::narrowed),
            Ok(2.5_f32)
        );
        assert_eq!(
            message(check("W", "max_width", F32Input(-1e300), UNBOUNDED)),
            "W.max_width must be within the 32-bit float range (got -1e+300)"
        );
        assert_eq!(
            message(check(
                "W",
                "max_width",
                F32Input(f64::NEG_INFINITY),
                UNBOUNDED
            )),
            "W.max_width must be finite or positive infinity (got -inf)"
        );
    }

    #[test]
    fn optional_values_pass_when_absent() {
        assert_eq!(check("Node", "aspect_ratio", None::<f32>, FINITE), Ok(None));
        assert_eq!(
            check("Node", "aspect_ratio", Some(2.0_f32), FINITE),
            Ok(Some(2.0))
        );
        assert_eq!(
            message(check("Node", "aspect_ratio", Some(f32::INFINITY), FINITE)),
            "Node.aspect_ratio must be finite (got inf)"
        );
    }

    #[test]
    fn vectors_and_quaternions_name_the_failing_component() {
        assert_eq!(
            message(check(
                "Transform",
                "translation",
                Vec3::new(1.0, f32::NAN, 0.0),
                FINITE
            )),
            "Transform.translation.y must be finite (got nan)"
        );
        let rotation = Quat::from_xyzw(0.0, 0.0, 0.0, f32::INFINITY);
        assert_eq!(
            message(check("Transform", "rotation", rotation, FINITE)),
            "Transform.rotation.w must be finite (got inf)"
        );
        assert_eq!(
            message(check(
                "Vignette",
                "center",
                Some(Vec2::new(f32::NAN, 0.0)),
                FINITE
            )),
            "Vignette.center.x must be finite (got nan)"
        );
        assert!(check("T", "v", Vec4::ONE, FINITE).is_ok());
        assert!(check("T", "v", Vec3A::ZERO, NON_NEGATIVE).is_ok());
    }

    #[test]
    fn components_and_columns_report_the_first_failing_element() {
        assert_eq!(
            message(check_components(
                "Vignette",
                "center",
                ["x", "y"],
                [0.5_f32, f32::NAN],
                FINITE
            )),
            "Vignette.center.y must be finite (got nan)"
        );
        assert_eq!(
            check_components("Vignette", "center", ["x", "y"], [0.5_f32, 0.5], FINITE),
            Ok([0.5, 0.5])
        );
        assert_eq!(
            message(check_column(
                "FogVolume",
                "scattering_asymmetry",
                &[],
                &[0.0_f32, 0.5, 2.0, 3.0],
                CLOSED
            )),
            "FogVolume.scattering_asymmetry must be in [-1.0, 1.0] (got 2.0 at row 2)"
        );
        assert_eq!(
            message(check_column(
                "Transform",
                "translation",
                &["x", "y", "z"],
                &[1.0_f32, 2.0, 3.0, 4.0, f32::INFINITY, 6.0],
                FINITE
            )),
            "Transform.translation.y must be finite (got inf at row 1)"
        );
        assert!(check_column("T", "f", &[], &[] as &[f32], FINITE).is_ok());
    }

    #[test]
    fn orderings_compare_numbers_and_fail_on_nan() {
        assert!(check_ordered("P", "min_threads", 4_u32, "max_threads", 4, false).is_ok());
        assert_eq!(
            message(check_ordered(
                "TaskPoolOptions",
                "min_total_threads",
                8_u32,
                "max_total_threads",
                4,
                false
            )),
            "TaskPoolOptions.min_total_threads must not exceed max_total_threads (got 8 > 4)"
        );
        assert_eq!(
            message(check_ordered(
                "Interval", "start", 5.0_f32, "end", 5.0, true
            )),
            "Interval.start must be less than end (got 5.0 >= 5.0)"
        );
        assert!(
            check_ordered(
                "Interval",
                "start",
                f32::NEG_INFINITY,
                "end",
                f32::INFINITY,
                true
            )
            .is_ok()
        );
        assert_eq!(
            message(check_ordered(
                "Viewport",
                "depth.start",
                f32::NAN,
                "depth.end",
                1.0,
                false
            )),
            "Viewport.depth.start must not exceed depth.end (got nan and 1.0)"
        );
    }

    #[test]
    fn ranges_check_both_ends_then_the_declared_ordering() {
        const DEPTH: &[FieldDomain] = &[
            FieldDomain::RangeInclusive { lo: 0.0, hi: 1.0 },
            FieldDomain::Ordered { strict: false },
        ];
        assert!(check("Viewport", "depth", 0.25_f32..0.25, DEPTH).is_ok());
        assert_eq!(
            message(check("Viewport", "depth", 0.0_f32..2.0, DEPTH)),
            "Viewport.depth.end must be in [0.0, 1.0] (got 2.0)"
        );
        assert_eq!(
            message(check("Viewport", "depth", 0.8_f32..0.2, DEPTH)),
            "Viewport.depth.start must not exceed depth.end (got 0.8 > 0.2)"
        );
        const STRICT: &[FieldDomain] = &[FieldDomain::Ordered { strict: true }];
        assert_eq!(
            message(check("S", "span", 1.0_f32..1.0, STRICT)),
            "S.span.start must be less than span.end (got 1.0 >= 1.0)"
        );
        assert_eq!(
            message(check("S", "span", f32::NAN..1.0, STRICT)),
            "S.span.start must be less than span.end (got nan and 1.0)"
        );
        assert!(check("S", "span", 5.0_f32..1.0, FINITE).is_ok());
        assert!(check("S", "value", 3.0_f32, STRICT).is_ok());
    }

    #[test]
    fn violations_convert_to_invalid_value_storage_errors() {
        let violation = match check("Exposure", "ev100", f32::INFINITY, FINITE) {
            Ok(_) => panic!("expected a domain violation"),
            Err(violation) => violation,
        };
        match StorageError::from(violation) {
            StorageError::InvalidValue(text) => {
                assert_eq!(text, "Exposure.ev100 must be finite (got inf)");
            }
            other => panic!("expected InvalidValue, got {other:?}"),
        }
        assert_eq!(DomainValue::from(3_usize), DomainValue::Int(3));
    }
}
