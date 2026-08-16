use bevy::{
    color::{
        Alpha, Color, Hsla, Hsva, Hue, Hwba, Laba, Lcha, LinearRgba, Luminance, Mix, Oklaba,
        Oklcha, Saturation, Srgba, Xyza, color_difference::EuclideanDistance,
    },
    ecs::{component::Component, resource::Resource},
    math::TryStableInterpolate,
};
use pybevy_core::{
    BorrowableStorage, ComponentStorage, FromBorrowedStorage, PyMaterializable, ResourceStorage,
    StorageMut, StorageRef, ValueStorage,
    public_error::{COLOR_INTERPOLATION_MISMATCH, enum_variant_changed},
};
use pybevy_macros::pyenum;
use pyo3::{
    Borrowed,
    exceptions::{PyRuntimeError, PyValueError},
    prelude::*,
};

use super::{
    common::fmt_f32, hsla::PyHsla, hsva::PyHsva, hwba::PyHwba, laba::PyLaba, lcha::PyLcha,
    linear_rgba::PyLinearRgba, oklaba::PyOklaba, oklcha::PyOklcha, srgba::PySrgba, xyza::PyXyza,
};

#[pyenum(Color, manual)]
#[pyclass(
    name = "Color",
    module = "pybevy.color",
    extends = PyMaterializable,
    subclass,
    from_py_object
)]
#[derive(Debug, Clone)]
pub struct PyColor {
    storage: ValueStorage<Color>,
    expected: ColorVariant,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OwnedColorValue(pub Color);

impl From<Color> for OwnedColorValue {
    fn from(color: Color) -> Self {
        Self(color)
    }
}

impl From<OwnedColorValue> for Color {
    fn from(color: OwnedColorValue) -> Self {
        color.0
    }
}

impl FromPyObject<'_, '_> for OwnedColorValue {
    type Error = PyErr;

    fn extract(obj: Borrowed<'_, '_, PyAny>) -> PyResult<Self> {
        let color = obj.extract::<PyRef<'_, PyColor>>()?;
        Ok(Self(color.resolved_copy()?))
    }
}

impl<'py> IntoPyObject<'py> for OwnedColorValue {
    type Target = PyColor;
    type Output = Bound<'py, PyColor>;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> PyResult<Self::Output> {
        Ok(PyColor::from_color(self.0, py)?.into_bound(py))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ColorVariant {
    Srgba,
    LinearRgba,
    Hsla,
    Hsva,
    Hwba,
    Laba,
    Lcha,
    Oklaba,
    Oklcha,
    Xyza,
}

impl ColorVariant {
    fn of(color: &Color) -> Self {
        match color {
            Color::Srgba(_) => Self::Srgba,
            Color::LinearRgba(_) => Self::LinearRgba,
            Color::Hsla(_) => Self::Hsla,
            Color::Hsva(_) => Self::Hsva,
            Color::Hwba(_) => Self::Hwba,
            Color::Laba(_) => Self::Laba,
            Color::Lcha(_) => Self::Lcha,
            Color::Oklaba(_) => Self::Oklaba,
            Color::Oklcha(_) => Self::Oklcha,
            Color::Xyza(_) => Self::Xyza,
        }
    }

    fn qualname(self) -> &'static str {
        match self {
            Self::Srgba => "Color.Srgba",
            Self::LinearRgba => "Color.LinearRgba",
            Self::Hsla => "Color.Hsla",
            Self::Hsva => "Color.Hsva",
            Self::Hwba => "Color.Hwba",
            Self::Laba => "Color.Laba",
            Self::Lcha => "Color.Lcha",
            Self::Oklaba => "Color.Oklaba",
            Self::Oklcha => "Color.Oklcha",
            Self::Xyza => "Color.Xyza",
        }
    }
}

impl Default for PyColor {
    fn default() -> Self {
        Color::WHITE.into()
    }
}

impl From<Color> for PyColor {
    fn from(color: Color) -> Self {
        Self {
            expected: ColorVariant::of(&color),
            storage: ValueStorage::owned(color),
        }
    }
}

impl TryFrom<PyColor> for Color {
    type Error = PyErr;

    fn try_from(py_color: PyColor) -> PyResult<Self> {
        py_color.resolved_copy()
    }
}

impl TryFrom<&PyColor> for Color {
    type Error = PyErr;

    fn try_from(py_color: &PyColor) -> PyResult<Self> {
        py_color.resolved_copy()
    }
}

impl<'py> IntoPyObject<'py> for PyColor {
    type Target = PyColor;
    type Output = Bound<'py, PyColor>;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> PyResult<Self::Output> {
        Ok(Self::from_color(self.resolved_copy()?, py)?.into_bound(py))
    }
}

impl PyColor {
    pub fn from_color(color: Color, py: Python) -> PyResult<Py<Self>> {
        Self::from_storage(ValueStorage::owned(color), py)
    }

    pub fn from_snapshot(color: Color, py: Python) -> PyResult<Py<Self>> {
        Self::from_storage(ValueStorage::snapshot(&color), py)
    }

    pub fn from_storage(storage: ValueStorage<Color>, py: Python) -> PyResult<Py<Self>> {
        let color = storage.get()?;
        let expected = ColorVariant::of(&color);
        macro_rules! materialize_variant {
            ($variant:expr) => {{
                let value = Py::new(
                    py,
                    PyClassInitializer::from((PyColor { storage, expected }, PyMaterializable))
                        .add_subclass($variant),
                )?;
                Ok(value.into_bound(py).into_super().unbind())
            }};
        }

        match color {
            Color::Srgba(_) => materialize_variant!(PyColorSrgba),
            Color::LinearRgba(_) => materialize_variant!(PyColorLinearRgba),
            Color::Hsla(_) => materialize_variant!(PyColorHsla),
            Color::Hsva(_) => materialize_variant!(PyColorHsva),
            Color::Hwba(_) => materialize_variant!(PyColorHwba),
            Color::Laba(_) => materialize_variant!(PyColorLaba),
            Color::Lcha(_) => materialize_variant!(PyColorLcha),
            Color::Oklaba(_) => materialize_variant!(PyColorOklaba),
            Color::Oklcha(_) => materialize_variant!(PyColorOklcha),
            Color::Xyza(_) => materialize_variant!(PyColorXyza),
        }
    }

    pub fn from_component_field<T: Component>(
        storage: &ComponentStorage<T>,
        read: impl Fn(&T) -> &Color,
        py: Python,
    ) -> PyResult<Py<Self>> {
        Self::from_storage(storage.borrow_field(read)?, py)
    }

    pub fn from_resource_field<T: Resource>(
        storage: &ResourceStorage<T>,
        read: impl Fn(&T) -> &Color,
        py: Python,
    ) -> PyResult<Py<Self>> {
        Self::from_storage(storage.borrow_field(read)?, py)
    }

    fn validate_variant(&self, color: &Color) -> PyResult<()> {
        if ColorVariant::of(color) == self.expected {
            Ok(())
        } else {
            Err(PyRuntimeError::new_err(enum_variant_changed(
                self.expected.qualname(),
            )))
        }
    }

    fn as_ref(&self) -> PyResult<StorageRef<'_, Color>> {
        let color = self.storage.as_ref()?;
        self.validate_variant(&color)?;
        Ok(color)
    }

    fn as_mut(&mut self) -> PyResult<StorageMut<'_, Color>> {
        {
            let color = self.storage.as_ref()?;
            self.validate_variant(&color)?;
        }
        Ok(self.storage.as_mut()?)
    }

    pub fn resolved_copy(&self) -> PyResult<Color> {
        Ok(*self.as_ref()?)
    }

    pub fn try_eq(&self, other: &Self) -> PyResult<bool> {
        Ok(self.resolved_copy()? == other.resolved_copy()?)
    }
}

#[pymethods]
impl PyColor {
    #[new]
    pub fn new(py: Python<'_>) -> PyResult<Py<Self>> {
        Self::from_color(Color::default(), py)
    }

    pub fn __repr__(&self) -> PyResult<String> {
        macro_rules! variant_repr {
            ($name:literal, $value:expr, $first:ident, $second:ident, $third:ident) => {{
                format!(
                    concat!("Color.", $name, "(", $name, "({}, {}, {}, {}))"),
                    fmt_f32($value.$first),
                    fmt_f32($value.$second),
                    fmt_f32($value.$third),
                    fmt_f32($value.alpha),
                )
            }};
        }

        let color = self.as_ref()?;
        let result = match *color {
            Color::Srgba(value) => variant_repr!("Srgba", value, red, green, blue),
            Color::LinearRgba(value) => {
                variant_repr!("LinearRgba", value, red, green, blue)
            }
            Color::Hsla(value) => variant_repr!("Hsla", value, hue, saturation, lightness),
            Color::Hsva(value) => variant_repr!("Hsva", value, hue, saturation, value),
            Color::Hwba(value) => variant_repr!("Hwba", value, hue, whiteness, blackness),
            Color::Laba(value) => variant_repr!("Laba", value, lightness, a, b),
            Color::Lcha(value) => variant_repr!("Lcha", value, lightness, chroma, hue),
            Color::Oklaba(value) => variant_repr!("Oklaba", value, lightness, a, b),
            Color::Oklcha(value) => variant_repr!("Oklcha", value, lightness, chroma, hue),
            Color::Xyza(value) => variant_repr!("Xyza", value, x, y, z),
        };
        Ok(result)
    }

    pub fn __copy__(&self, py: Python<'_>) -> PyResult<Py<Self>> {
        Self::from_color(self.resolved_copy()?, py)
    }

    #[staticmethod]
    #[pyo3(name = "WHITE")]
    pub fn white(py: Python<'_>) -> PyResult<Py<Self>> {
        Self::from_color(Color::WHITE, py)
    }

    #[staticmethod]
    #[pyo3(name = "BLACK")]
    pub fn black(py: Python<'_>) -> PyResult<Py<Self>> {
        Self::from_color(Color::BLACK, py)
    }

    #[staticmethod]
    #[pyo3(name = "NONE")]
    pub fn none_(py: Python<'_>) -> PyResult<Py<Self>> {
        Self::from_color(Color::NONE, py)
    }

    #[staticmethod]
    pub fn srgb_u8(red: u8, green: u8, blue: u8, py: Python<'_>) -> PyResult<Py<Self>> {
        Self::from_color(Color::srgb_u8(red, green, blue), py)
    }

    #[staticmethod]
    pub fn srgba_u8(red: u8, green: u8, blue: u8, alpha: u8, py: Python<'_>) -> PyResult<Py<Self>> {
        Self::from_color(Color::srgba_u8(red, green, blue, alpha), py)
    }

    #[staticmethod]
    pub fn srgb_u32(color: u32, py: Python<'_>) -> PyResult<Py<Self>> {
        Self::from_color(Color::srgb_u32(color), py)
    }

    #[staticmethod]
    pub fn srgba_u32(color: u32, py: Python<'_>) -> PyResult<Py<Self>> {
        Self::from_color(Color::srgba_u32(color), py)
    }

    #[staticmethod]
    pub fn srgb(red: f32, green: f32, blue: f32, py: Python<'_>) -> PyResult<Py<Self>> {
        Self::from_color(Color::srgb(red, green, blue), py)
    }

    #[staticmethod]
    pub fn srgba(
        red: f32,
        green: f32,
        blue: f32,
        alpha: f32,
        py: Python<'_>,
    ) -> PyResult<Py<Self>> {
        Self::from_color(Color::srgba(red, green, blue, alpha), py)
    }

    #[staticmethod]
    pub fn srgb_from_array(array: [f32; 3], py: Python<'_>) -> PyResult<Py<Self>> {
        Self::from_color(Color::srgb_from_array(array), py)
    }

    #[staticmethod]
    pub fn linear_rgb(red: f32, green: f32, blue: f32, py: Python<'_>) -> PyResult<Py<Self>> {
        Self::from_color(Color::linear_rgb(red, green, blue), py)
    }

    #[staticmethod]
    pub fn linear_rgba(
        red: f32,
        green: f32,
        blue: f32,
        alpha: f32,
        py: Python<'_>,
    ) -> PyResult<Py<Self>> {
        Self::from_color(Color::linear_rgba(red, green, blue, alpha), py)
    }

    #[staticmethod]
    pub fn hsl(hue: f32, saturation: f32, lightness: f32, py: Python<'_>) -> PyResult<Py<Self>> {
        Self::from_color(Color::hsl(hue, saturation, lightness), py)
    }

    #[staticmethod]
    pub fn hsla(
        hue: f32,
        saturation: f32,
        lightness: f32,
        alpha: f32,
        py: Python<'_>,
    ) -> PyResult<Py<Self>> {
        Self::from_color(Color::hsla(hue, saturation, lightness, alpha), py)
    }

    #[staticmethod]
    pub fn hsv(hue: f32, saturation: f32, value: f32, py: Python<'_>) -> PyResult<Py<Self>> {
        Self::from_color(Color::hsv(hue, saturation, value), py)
    }

    #[staticmethod]
    pub fn hsva(
        hue: f32,
        saturation: f32,
        value: f32,
        alpha: f32,
        py: Python<'_>,
    ) -> PyResult<Py<Self>> {
        Self::from_color(Color::hsva(hue, saturation, value, alpha), py)
    }

    #[staticmethod]
    pub fn hwb(hue: f32, whiteness: f32, blackness: f32, py: Python<'_>) -> PyResult<Py<Self>> {
        Self::from_color(Color::hwb(hue, whiteness, blackness), py)
    }

    #[staticmethod]
    pub fn hwba(
        hue: f32,
        whiteness: f32,
        blackness: f32,
        alpha: f32,
        py: Python<'_>,
    ) -> PyResult<Py<Self>> {
        Self::from_color(Color::hwba(hue, whiteness, blackness, alpha), py)
    }

    #[staticmethod]
    pub fn lab(lightness: f32, a: f32, b: f32, py: Python<'_>) -> PyResult<Py<Self>> {
        Self::from_color(Color::lab(lightness, a, b), py)
    }

    #[staticmethod]
    pub fn laba(lightness: f32, a: f32, b: f32, alpha: f32, py: Python<'_>) -> PyResult<Py<Self>> {
        Self::from_color(Color::laba(lightness, a, b, alpha), py)
    }

    #[staticmethod]
    pub fn lch(lightness: f32, chroma: f32, hue: f32, py: Python<'_>) -> PyResult<Py<Self>> {
        Self::from_color(Color::lch(lightness, chroma, hue), py)
    }

    #[staticmethod]
    pub fn lcha(
        lightness: f32,
        chroma: f32,
        hue: f32,
        alpha: f32,
        py: Python<'_>,
    ) -> PyResult<Py<Self>> {
        Self::from_color(Color::lcha(lightness, chroma, hue, alpha), py)
    }

    #[staticmethod]
    pub fn oklab(lightness: f32, a: f32, b: f32, py: Python<'_>) -> PyResult<Py<Self>> {
        Self::from_color(Color::oklab(lightness, a, b), py)
    }

    #[staticmethod]
    pub fn oklaba(
        lightness: f32,
        a: f32,
        b: f32,
        alpha: f32,
        py: Python<'_>,
    ) -> PyResult<Py<Self>> {
        Self::from_color(Color::oklaba(lightness, a, b, alpha), py)
    }

    #[staticmethod]
    pub fn oklch(lightness: f32, chroma: f32, hue: f32, py: Python<'_>) -> PyResult<Py<Self>> {
        Self::from_color(Color::oklch(lightness, chroma, hue), py)
    }

    #[staticmethod]
    pub fn oklcha(
        lightness: f32,
        chroma: f32,
        hue: f32,
        alpha: f32,
        py: Python<'_>,
    ) -> PyResult<Py<Self>> {
        Self::from_color(Color::oklcha(lightness, chroma, hue, alpha), py)
    }

    #[staticmethod]
    pub fn xyz(x: f32, y: f32, z: f32, py: Python<'_>) -> PyResult<Py<Self>> {
        Self::from_color(Color::xyz(x, y, z), py)
    }

    #[staticmethod]
    pub fn xyza(x: f32, y: f32, z: f32, alpha: f32, py: Python<'_>) -> PyResult<Py<Self>> {
        Self::from_color(Color::xyza(x, y, z, alpha), py)
    }

    pub fn to_linear(&self) -> PyResult<PyLinearRgba> {
        let linear: LinearRgba = self.resolved_copy()?.into();
        Ok(PyLinearRgba::from_linear_rgba(linear))
    }

    pub fn to_srgba(&self) -> PyResult<PySrgba> {
        let srgba: Srgba = self.resolved_copy()?.into();
        Ok(PySrgba::from_srgba(srgba))
    }

    // Note: materialize() method is in the main pybevy crate (depends on StandardMaterial)

    pub fn with_alpha(&self, alpha: f32, py: Python<'_>) -> PyResult<Py<Self>> {
        Self::from_color(self.as_ref()?.with_alpha(alpha), py)
    }

    pub fn alpha(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.alpha())
    }

    pub fn is_fully_transparent(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_fully_transparent())
    }

    pub fn is_fully_opaque(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_fully_opaque())
    }

    pub fn luminance(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.luminance())
    }

    pub fn with_luminance(&self, value: f32, py: Python<'_>) -> PyResult<Py<Self>> {
        Self::from_color(self.as_ref()?.with_luminance(value), py)
    }

    pub fn darker(&self, amount: f32, py: Python<'_>) -> PyResult<Py<Self>> {
        Self::from_color(self.as_ref()?.darker(amount), py)
    }

    pub fn lighter(&self, amount: f32, py: Python<'_>) -> PyResult<Py<Self>> {
        Self::from_color(self.as_ref()?.lighter(amount), py)
    }

    pub fn mix(&self, other: &Self, factor: f32, py: Python<'_>) -> PyResult<Py<Self>> {
        let other = other.resolved_copy()?;
        Self::from_color(self.as_ref()?.mix(&other, factor), py)
    }

    pub fn hue(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.hue())
    }

    pub fn with_hue(&self, hue: f32, py: Python<'_>) -> PyResult<Py<Self>> {
        Self::from_color(self.as_ref()?.with_hue(hue), py)
    }

    pub fn rotate_hue(&self, degrees: f32, py: Python<'_>) -> PyResult<Py<Self>> {
        Self::from_color(self.as_ref()?.rotate_hue(degrees), py)
    }

    pub fn saturation(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.saturation())
    }

    pub fn with_saturation(&self, saturation: f32, py: Python<'_>) -> PyResult<Py<Self>> {
        Self::from_color(self.as_ref()?.with_saturation(saturation), py)
    }

    pub fn distance(&self, other: &Self) -> PyResult<f32> {
        let other = other.resolved_copy()?;
        Ok(self.as_ref()?.distance(&other))
    }

    pub fn distance_squared(&self, other: &Self) -> PyResult<f32> {
        let other = other.resolved_copy()?;
        Ok(self.as_ref()?.distance_squared(&other))
    }

    pub fn try_interpolate_stable(
        &self,
        other: &Self,
        t: f32,
        py: Python<'_>,
    ) -> PyResult<Py<Self>> {
        let other = other.resolved_copy()?;
        let color = self
            .as_ref()?
            .try_interpolate_stable(&other, t)
            .map_err(|_| PyValueError::new_err(COLOR_INTERPOLATION_MISMATCH))?;
        Self::from_color(color, py)
    }

    pub fn set_alpha(&mut self, alpha: f32) -> PyResult<()> {
        let color = self.resolved_copy()?.with_alpha(alpha);
        *self.as_mut()? = color;
        Ok(())
    }

    pub fn set_hue(&mut self, hue: f32) -> PyResult<()> {
        let color = self.resolved_copy()?.with_hue(hue);
        *self.as_mut()? = color;
        Ok(())
    }

    pub fn mix_assign(&mut self, other: &Self, factor: f32) -> PyResult<()> {
        let other = other.resolved_copy()?;
        let color = self.resolved_copy()?.mix(&other, factor);
        *self.as_mut()? = color;
        Ok(())
    }

    pub fn __eq__(&self, other: &Self) -> PyResult<bool> {
        self.try_eq(other)
    }

    pub fn __ne__(&self, other: &Self) -> PyResult<bool> {
        Ok(!self.try_eq(other)?)
    }

    #[classattr]
    const __hash__: Option<Py<PyAny>> = None;
}

macro_rules! define_color_variant {
    ($python:ident, $name:literal, $variant:ident, $native:ty, $wrapper:ty) => {
        #[pyclass(name = $name, module = "pybevy.color", extends = PyColor)]
        pub struct $python;

        #[pymethods]
        #[allow(non_upper_case_globals)]
        impl $python {
            #[classattr]
            const __qualname__: &'static str = concat!("Color.", $name);

            #[classattr]
            fn __match_args__() -> (&'static str,) {
                ("value",)
            }

            #[new]
            pub fn new(value: &$wrapper) -> PyResult<PyClassInitializer<Self>> {
                let value = <$native>::try_from(value)?;
                Ok(PyClassInitializer::from((
                    PyColor::from(Color::$variant(value)),
                    PyMaterializable,
                ))
                .add_subclass(Self))
            }

            #[getter]
            pub fn value(slf: PyRef<'_, Self>) -> PyResult<$wrapper> {
                let base = slf.into_super();
                Ok(base.storage.borrow_resolved_variant_as(
                    concat!("Color.", $name),
                    |color| match color {
                        Color::$variant(value) => Some(value),
                        _ => None,
                    },
                    |color| match color {
                        Color::$variant(value) => Some(value),
                        _ => None,
                    },
                )?)
            }
        }
    };
}

macro_rules! impl_color_value_storage {
    ($wrapper:ty, $native:ty) => {
        impl FromBorrowedStorage<ValueStorage<$native>> for $wrapper {
            fn from_borrowed(storage: ValueStorage<$native>) -> Self {
                Self { storage }
            }
        }
    };
}

impl_color_value_storage!(PySrgba, Srgba);
impl_color_value_storage!(PyLinearRgba, LinearRgba);
impl_color_value_storage!(PyHsla, Hsla);
impl_color_value_storage!(PyHsva, Hsva);
impl_color_value_storage!(PyLaba, Laba);
impl_color_value_storage!(PyLcha, Lcha);
impl_color_value_storage!(PyOklaba, Oklaba);
impl_color_value_storage!(PyOklcha, Oklcha);
impl_color_value_storage!(PyXyza, Xyza);

define_color_variant!(PyColorSrgba, "Srgba", Srgba, Srgba, PySrgba);
define_color_variant!(
    PyColorLinearRgba,
    "LinearRgba",
    LinearRgba,
    LinearRgba,
    PyLinearRgba
);
define_color_variant!(PyColorHsla, "Hsla", Hsla, Hsla, PyHsla);
define_color_variant!(PyColorHsva, "Hsva", Hsva, Hsva, PyHsva);
define_color_variant!(PyColorHwba, "Hwba", Hwba, Hwba, PyHwba);
define_color_variant!(PyColorLaba, "Laba", Laba, Laba, PyLaba);
define_color_variant!(PyColorLcha, "Lcha", Lcha, Lcha, PyLcha);
define_color_variant!(PyColorOklaba, "Oklaba", Oklaba, Oklaba, PyOklaba);
define_color_variant!(PyColorOklcha, "Oklcha", Oklcha, Oklcha, PyOklcha);
define_color_variant!(PyColorXyza, "Xyza", Xyza, Xyza, PyXyza);

pub fn register_color_variants(module: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = module.py();
    let base = module.getattr("Color")?;
    base.setattr("Srgba", py.get_type::<PyColorSrgba>())?;
    base.setattr("LinearRgba", py.get_type::<PyColorLinearRgba>())?;
    base.setattr("Hsla", py.get_type::<PyColorHsla>())?;
    base.setattr("Hsva", py.get_type::<PyColorHsva>())?;
    base.setattr("Hwba", py.get_type::<PyColorHwba>())?;
    base.setattr("Laba", py.get_type::<PyColorLaba>())?;
    base.setattr("Lcha", py.get_type::<PyColorLcha>())?;
    base.setattr("Oklaba", py.get_type::<PyColorOklaba>())?;
    base.setattr("Oklcha", py.get_type::<PyColorOklcha>())?;
    base.setattr("Xyza", py.get_type::<PyColorXyza>())?;
    Ok(())
}
