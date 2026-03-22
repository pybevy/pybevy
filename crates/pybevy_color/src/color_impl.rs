use bevy::{
    color::{
        Alpha, Color, Gray, Hsla, Hsva, Hue, Laba, Lcha, LinearRgba, Luminance, Mix, Oklaba,
        Oklcha, Saturation, Srgba, Xyza, color_difference::EuclideanDistance,
    },
    math::StableInterpolate,
};
use pybevy_core::{PyMaterializable, ValueStorage};
use pyo3::{exceptions::PyValueError, prelude::*};

/// Format f32 like Bevy: trim trailing zeros but keep at least one decimal.
/// e.g. 1.0, 0.5, 0.15, 0.333
fn fmt_f32(v: f32) -> String {
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0');
    if s.ends_with('.') {
        format!("{s}0")
    } else {
        s.to_string()
    }
}

#[pyclass(name = "Color", extends = PyMaterializable, eq)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyColor(pub Color);

impl Default for PyColor {
    fn default() -> Self {
        PyColor(Color::WHITE)
    }
}

impl From<Color> for PyColor {
    fn from(color: Color) -> Self {
        PyColor(color)
    }
}

impl From<PyColor> for Color {
    fn from(py_color: PyColor) -> Self {
        py_color.0
    }
}

impl PyColor {
    pub fn from_color(color: Color, py: Python) -> PyResult<Py<Self>> {
        Py::new(py, (PyColor(color), PyMaterializable))
    }
}

#[pymethods]
impl PyColor {
    #[new]
    pub fn new() -> (Self, PyMaterializable) {
        (PyColor(Color::WHITE), PyMaterializable)
    }

    pub fn __repr__(&self) -> String {
        let srgba: Srgba = self.0.into();
        format!(
            "Color.srgba({}, {}, {}, {})",
            fmt_f32(srgba.red),
            fmt_f32(srgba.green),
            fmt_f32(srgba.blue),
            fmt_f32(srgba.alpha),
        )
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

    pub fn to_linear(&self) -> PyLinearRgba {
        let linear: LinearRgba = self.0.into();
        PyLinearRgba::from_linear_rgba(linear)
    }

    pub fn to_srgba(&self) -> PySrgba {
        let srgba: Srgba = self.0.into();
        PySrgba::from_srgba(srgba)
    }

    // Note: materialize() method is in the main pybevy crate (depends on StandardMaterial)

    pub fn with_alpha(&self, alpha: f32, py: Python<'_>) -> PyResult<Py<Self>> {
        Self::from_color(self.0.with_alpha(alpha), py)
    }

    pub fn alpha(&self) -> f32 {
        self.0.alpha()
    }

    pub fn is_fully_transparent(&self) -> bool {
        self.0.is_fully_transparent()
    }

    pub fn is_fully_opaque(&self) -> bool {
        self.0.is_fully_opaque()
    }

    pub fn luminance(&self) -> f32 {
        self.0.luminance()
    }

    pub fn with_luminance(&self, value: f32, py: Python<'_>) -> PyResult<Py<Self>> {
        Self::from_color(self.0.with_luminance(value), py)
    }

    pub fn darker(&self, amount: f32, py: Python<'_>) -> PyResult<Py<Self>> {
        Self::from_color(self.0.darker(amount), py)
    }

    pub fn lighter(&self, amount: f32, py: Python<'_>) -> PyResult<Py<Self>> {
        Self::from_color(self.0.lighter(amount), py)
    }

    pub fn mix(&self, other: &Self, factor: f32, py: Python<'_>) -> PyResult<Py<Self>> {
        Self::from_color(self.0.mix(&other.0, factor), py)
    }

    pub fn hue(&self) -> f32 {
        self.0.hue()
    }

    pub fn with_hue(&self, hue: f32, py: Python<'_>) -> PyResult<Py<Self>> {
        Self::from_color(self.0.with_hue(hue), py)
    }

    pub fn rotate_hue(&self, degrees: f32, py: Python<'_>) -> PyResult<Py<Self>> {
        Self::from_color(self.0.rotate_hue(degrees), py)
    }

    pub fn saturation(&self) -> f32 {
        self.0.saturation()
    }

    pub fn with_saturation(&self, saturation: f32, py: Python<'_>) -> PyResult<Py<Self>> {
        Self::from_color(self.0.with_saturation(saturation), py)
    }

    pub fn distance(&self, other: &Self) -> f32 {
        self.0.distance(&other.0)
    }

    pub fn distance_squared(&self, other: &Self) -> f32 {
        self.0.distance_squared(&other.0)
    }

    pub fn to_hsla(&self) -> PyHsla {
        let hsla: Hsla = self.0.into();
        PyHsla::from_hsla(hsla)
    }

    pub fn set_alpha(&mut self, alpha: f32) {
        self.0 = self.0.with_alpha(alpha);
    }

    pub fn set_hue(&mut self, hue: f32) {
        self.0 = self.0.with_hue(hue);
    }

    pub fn set_saturation(&mut self, saturation: f32) {
        self.0 = self.0.with_saturation(saturation);
    }

    pub fn mix_assign(&mut self, other: &Self, factor: f32) {
        self.0 = self.0.mix(&other.0, factor);
    }
}

#[pyclass(name = "LinearRgba", eq)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyLinearRgba {
    storage: ValueStorage<LinearRgba>,
}

impl From<PyLinearRgba> for LinearRgba {
    #[inline(always)]
    fn from(py_color: PyLinearRgba) -> Self {
        py_color.storage.get().unwrap()
    }
}

impl From<&PyLinearRgba> for LinearRgba {
    #[inline(always)]
    fn from(py_color: &PyLinearRgba) -> Self {
        py_color.storage.get().unwrap()
    }
}

impl From<LinearRgba> for PyLinearRgba {
    #[inline(always)]
    fn from(color: LinearRgba) -> Self {
        PyLinearRgba::from_linear_rgba(color)
    }
}

impl PyLinearRgba {
    #[inline(always)]
    pub fn from_linear_rgba(color: LinearRgba) -> Self {
        PyLinearRgba {
            storage: ValueStorage::owned(color),
        }
    }

    #[inline(always)]
    pub const fn linear_rgba(color: LinearRgba) -> Self {
        PyLinearRgba {
            storage: ValueStorage::owned(color),
        }
    }

    #[inline(always)]
    fn as_ref(&self) -> PyResult<&LinearRgba> {
        Ok(self.storage.as_ref()?)
    }

    #[inline(always)]
    fn as_mut(&mut self) -> PyResult<&mut LinearRgba> {
        Ok(self.storage.as_mut()?)
    }
}

#[pymethods]
impl PyLinearRgba {
    #[new]
    #[pyo3(signature = (red = 1.0, green = 1.0, blue = 1.0, alpha = 1.0))]
    pub fn new(red: f32, green: f32, blue: f32, alpha: f32) -> Self {
        PyLinearRgba::linear_rgba(LinearRgba::new(red, green, blue, alpha))
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let c = self.as_ref()?;
        Ok(format!(
            "LinearRgba({}, {}, {}, {})",
            fmt_f32(c.red),
            fmt_f32(c.green),
            fmt_f32(c.blue),
            fmt_f32(c.alpha),
        ))
    }

    #[staticmethod]
    #[pyo3(name = "BLACK")]
    pub fn black() -> Self {
        Self::linear_rgba(LinearRgba::BLACK)
    }
    #[staticmethod]
    #[pyo3(name = "WHITE")]
    pub fn white() -> Self {
        Self::linear_rgba(LinearRgba::WHITE)
    }
    #[staticmethod]
    #[pyo3(name = "NONE")]
    pub fn none_() -> Self {
        Self::linear_rgba(LinearRgba::NONE)
    }
    #[staticmethod]
    #[pyo3(name = "NAN")]
    pub fn nan() -> Self {
        Self::linear_rgba(LinearRgba::NAN)
    }

    #[staticmethod]
    pub fn rgb(red: f32, green: f32, blue: f32) -> Self {
        PyLinearRgba::linear_rgba(LinearRgba::rgb(red, green, blue))
    }

    #[staticmethod]
    pub fn gray(lightness: f32) -> Self {
        PyLinearRgba::linear_rgba(LinearRgba::gray(lightness))
    }

    pub fn with_red(&self, red: f32) -> PyResult<Self> {
        Ok(PyLinearRgba::linear_rgba(self.as_ref()?.with_red(red)))
    }

    pub fn with_green(&self, green: f32) -> PyResult<Self> {
        Ok(PyLinearRgba::linear_rgba(self.as_ref()?.with_green(green)))
    }

    pub fn with_blue(&self, blue: f32) -> PyResult<Self> {
        Ok(PyLinearRgba::linear_rgba(self.as_ref()?.with_blue(blue)))
    }

    pub fn luminance(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.luminance())
    }

    pub fn with_luminance(&self, luminance: f32) -> PyResult<Self> {
        Ok(PyLinearRgba::linear_rgba(
            self.as_ref()?.with_luminance(luminance),
        ))
    }

    pub fn darker(&self, amount: f32) -> PyResult<Self> {
        Ok(PyLinearRgba::linear_rgba(self.as_ref()?.darker(amount)))
    }

    pub fn lighter(&self, amount: f32) -> PyResult<Self> {
        Ok(PyLinearRgba::linear_rgba(self.as_ref()?.lighter(amount)))
    }

    pub fn mix(&self, other: &Self, factor: f32) -> PyResult<Self> {
        Ok(PyLinearRgba::linear_rgba(
            self.as_ref()?.mix(other.as_ref()?, factor),
        ))
    }

    pub fn mix_assign(&mut self, other: &Self, factor: f32) -> PyResult<()> {
        let result = self.as_ref()?.mix(other.as_ref()?, factor);
        *self.as_mut()? = result;
        Ok(())
    }

    pub fn distance_squared(&self, other: &Self) -> PyResult<f32> {
        Ok(self.as_ref()?.distance_squared(other.as_ref()?))
    }

    pub fn to_tuple(&self) -> PyResult<(f32, f32, f32, f32)> {
        let color = self.as_ref()?;
        Ok((color.red, color.green, color.blue, color.alpha))
    }

    #[staticmethod]
    pub fn from_tuple(t: (f32, f32, f32, f32)) -> Self {
        PyLinearRgba::linear_rgba(LinearRgba::new(t.0, t.1, t.2, t.3))
    }

    #[getter]
    pub fn red(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.red)
    }

    #[setter]
    pub fn set_red(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.red = value;
        Ok(())
    }

    #[getter]
    pub fn green(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.green)
    }

    #[setter]
    pub fn set_green(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.green = value;
        Ok(())
    }

    #[getter]
    pub fn blue(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.blue)
    }

    #[setter]
    pub fn set_blue(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.blue = value;
        Ok(())
    }

    #[getter]
    pub fn alpha(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.alpha)
    }

    #[setter]
    pub fn set_alpha(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.alpha = value;
        Ok(())
    }

    pub fn with_alpha(&self, alpha: f32) -> PyResult<Self> {
        Ok(PyLinearRgba::linear_rgba(self.as_ref()?.with_alpha(alpha)))
    }

    pub fn is_fully_transparent(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_fully_transparent())
    }

    pub fn is_fully_opaque(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_fully_opaque())
    }

    pub fn __add__(&self, other: &Self) -> PyResult<Self> {
        Ok(PyLinearRgba::linear_rgba(
            *self.as_ref()? + *other.as_ref()?,
        ))
    }

    pub fn __sub__(&self, other: &Self) -> PyResult<Self> {
        Ok(PyLinearRgba::linear_rgba(
            *self.as_ref()? - *other.as_ref()?,
        ))
    }

    pub fn __mul__(&self, scalar: f32) -> PyResult<Self> {
        Ok(PyLinearRgba::linear_rgba(*self.as_ref()? * scalar))
    }

    pub fn __rmul__(&self, scalar: f32) -> PyResult<Self> {
        Ok(PyLinearRgba::linear_rgba(scalar * *self.as_ref()?))
    }

    pub fn __truediv__(&self, scalar: f32) -> PyResult<Self> {
        Ok(PyLinearRgba::linear_rgba(*self.as_ref()? / scalar))
    }

    pub fn __neg__(&self) -> PyResult<Self> {
        Ok(PyLinearRgba::linear_rgba(-*self.as_ref()?))
    }

    pub fn to_f32_array(&self) -> PyResult<[f32; 4]> {
        let color = self.as_ref()?;
        Ok([color.red, color.green, color.blue, color.alpha])
    }

    pub fn to_f32_array_no_alpha(&self) -> PyResult<[f32; 3]> {
        let color = self.as_ref()?;
        Ok([color.red, color.green, color.blue])
    }

    pub fn to_vec4(&self) -> PyResult<pybevy_math::PyVec4> {
        use bevy::math::Vec4;
        let color = self.as_ref()?;
        let vec4 = Vec4::new(color.red, color.green, color.blue, color.alpha);
        Ok(pybevy_math::PyVec4::from(vec4))
    }

    pub fn to_vec3(&self) -> PyResult<pybevy_math::PyVec3> {
        use bevy::math::Vec3;
        let color = self.as_ref()?;
        let vec3 = Vec3::new(color.red, color.green, color.blue);
        Ok(pybevy_math::PyVec3::from(vec3))
    }

    #[staticmethod]
    pub fn from_f32_array(color: [f32; 4]) -> Self {
        PyLinearRgba::linear_rgba(LinearRgba::new(color[0], color[1], color[2], color[3]))
    }

    #[staticmethod]
    pub fn from_f32_array_no_alpha(color: [f32; 3]) -> Self {
        PyLinearRgba::linear_rgba(LinearRgba::rgb(color[0], color[1], color[2]))
    }

    #[staticmethod]
    pub fn from_vec4(color: &pybevy_math::PyVec4) -> Self {
        let v: bevy::math::Vec4 = color.into();
        PyLinearRgba::linear_rgba(LinearRgba::new(v.x, v.y, v.z, v.w))
    }

    #[staticmethod]
    pub fn from_vec3(color: &pybevy_math::PyVec3) -> Self {
        let v: bevy::math::Vec3 = color.into();
        PyLinearRgba::linear_rgba(LinearRgba::rgb(v.x, v.y, v.z))
    }

    pub fn to_u8_array(&self) -> PyResult<[u8; 4]> {
        let color = self.as_ref()?;
        Ok([
            (color.red * 255.0) as u8,
            (color.green * 255.0) as u8,
            (color.blue * 255.0) as u8,
            (color.alpha * 255.0) as u8,
        ])
    }

    pub fn to_u8_array_no_alpha(&self) -> PyResult<[u8; 3]> {
        let color = self.as_ref()?;
        Ok([
            (color.red * 255.0) as u8,
            (color.green * 255.0) as u8,
            (color.blue * 255.0) as u8,
        ])
    }

    #[staticmethod]
    pub fn from_u8_array(color: [u8; 4]) -> Self {
        PyLinearRgba::linear_rgba(LinearRgba::new(
            color[0] as f32 / 255.0,
            color[1] as f32 / 255.0,
            color[2] as f32 / 255.0,
            color[3] as f32 / 255.0,
        ))
    }

    #[staticmethod]
    pub fn from_u8_array_no_alpha(color: [u8; 3]) -> Self {
        PyLinearRgba::linear_rgba(LinearRgba::rgb(
            color[0] as f32 / 255.0,
            color[1] as f32 / 255.0,
            color[2] as f32 / 255.0,
        ))
    }
}

#[pyclass(name = "Srgba", eq)]
#[derive(Debug, Clone, PartialEq)]
pub struct PySrgba {
    storage: ValueStorage<Srgba>,
}

impl From<PySrgba> for Srgba {
    #[inline(always)]
    fn from(py_color: PySrgba) -> Self {
        py_color.storage.get().unwrap()
    }
}

impl From<&PySrgba> for Srgba {
    #[inline(always)]
    fn from(py_color: &PySrgba) -> Self {
        py_color.storage.get().unwrap()
    }
}

impl From<Srgba> for PySrgba {
    #[inline(always)]
    fn from(color: Srgba) -> Self {
        PySrgba::from_srgba(color)
    }
}

impl PySrgba {
    #[inline(always)]
    pub fn from_srgba(color: Srgba) -> Self {
        PySrgba {
            storage: ValueStorage::owned(color),
        }
    }

    #[inline(always)]
    pub const fn srgba(color: Srgba) -> Self {
        PySrgba {
            storage: ValueStorage::owned(color),
        }
    }

    #[inline(always)]
    fn as_ref(&self) -> PyResult<&Srgba> {
        Ok(self.storage.as_ref()?)
    }

    #[inline(always)]
    fn as_mut(&mut self) -> PyResult<&mut Srgba> {
        Ok(self.storage.as_mut()?)
    }
}

#[pymethods]
impl PySrgba {
    #[new]
    #[pyo3(signature = (red = 1.0, green = 1.0, blue = 1.0, alpha = 1.0))]
    pub fn new(red: f32, green: f32, blue: f32, alpha: f32) -> Self {
        PySrgba::srgba(Srgba::new(red, green, blue, alpha))
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let c = self.as_ref()?;
        Ok(format!(
            "Srgba({}, {}, {}, {})",
            fmt_f32(c.red),
            fmt_f32(c.green),
            fmt_f32(c.blue),
            fmt_f32(c.alpha),
        ))
    }

    #[staticmethod]
    #[pyo3(name = "BLACK")]
    pub fn black() -> Self {
        Self::srgba(Srgba::BLACK)
    }
    #[staticmethod]
    #[pyo3(name = "WHITE")]
    pub fn white() -> Self {
        Self::srgba(Srgba::WHITE)
    }
    #[staticmethod]
    #[pyo3(name = "NONE")]
    pub fn none_() -> Self {
        Self::srgba(Srgba::NONE)
    }

    #[staticmethod]
    pub fn rgb(red: f32, green: f32, blue: f32) -> Self {
        PySrgba::srgba(Srgba::rgb(red, green, blue))
    }

    #[staticmethod]
    pub fn gray(lightness: f32) -> Self {
        PySrgba::srgba(Srgba::gray(lightness))
    }

    #[staticmethod]
    pub fn rgb_u8(r: u8, g: u8, b: u8) -> Self {
        PySrgba::srgba(Srgba::rgb_u8(r, g, b))
    }

    #[staticmethod]
    pub fn rgba_u8(r: u8, g: u8, b: u8, a: u8) -> Self {
        PySrgba::srgba(Srgba::rgba_u8(r, g, b, a))
    }

    #[staticmethod]
    pub fn hex(hex: &str) -> PyResult<Self> {
        Srgba::hex(hex)
            .map(PySrgba::srgba)
            .map_err(|e| PyValueError::new_err(format!("Invalid hex color: {:?}", e)))
    }

    pub fn to_hex(&self) -> PyResult<String> {
        Ok(self.as_ref()?.to_hex())
    }

    pub fn with_red(&self, red: f32) -> PyResult<Self> {
        Ok(PySrgba::srgba(self.as_ref()?.with_red(red)))
    }

    pub fn with_green(&self, green: f32) -> PyResult<Self> {
        Ok(PySrgba::srgba(self.as_ref()?.with_green(green)))
    }

    pub fn with_blue(&self, blue: f32) -> PyResult<Self> {
        Ok(PySrgba::srgba(self.as_ref()?.with_blue(blue)))
    }

    #[getter]
    pub fn red(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.red)
    }

    #[setter]
    pub fn set_red(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.red = value;
        Ok(())
    }

    #[getter]
    pub fn green(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.green)
    }

    #[setter]
    pub fn set_green(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.green = value;
        Ok(())
    }

    #[getter]
    pub fn blue(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.blue)
    }

    #[setter]
    pub fn set_blue(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.blue = value;
        Ok(())
    }

    #[getter]
    pub fn alpha(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.alpha)
    }

    #[setter]
    pub fn set_alpha(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.alpha = value;
        Ok(())
    }

    pub fn with_alpha(&self, alpha: f32) -> PyResult<Self> {
        Ok(PySrgba::srgba(self.as_ref()?.with_alpha(alpha)))
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

    pub fn with_luminance(&self, luminance: f32) -> PyResult<Self> {
        Ok(PySrgba::srgba(self.as_ref()?.with_luminance(luminance)))
    }

    pub fn darker(&self, amount: f32) -> PyResult<Self> {
        Ok(PySrgba::srgba(self.as_ref()?.darker(amount)))
    }

    pub fn lighter(&self, amount: f32) -> PyResult<Self> {
        Ok(PySrgba::srgba(self.as_ref()?.lighter(amount)))
    }

    pub fn mix(&self, other: &Self, factor: f32) -> PyResult<Self> {
        Ok(PySrgba::srgba(self.as_ref()?.mix(other.as_ref()?, factor)))
    }

    pub fn mix_assign(&mut self, other: &Self, factor: f32) -> PyResult<()> {
        let result = self.as_ref()?.mix(other.as_ref()?, factor);
        *self.as_mut()? = result;
        Ok(())
    }

    pub fn distance(&self, other: &Self) -> PyResult<f32> {
        Ok(self.as_ref()?.distance(other.as_ref()?))
    }

    pub fn distance_squared(&self, other: &Self) -> PyResult<f32> {
        Ok(self.as_ref()?.distance_squared(other.as_ref()?))
    }

    pub fn to_f32_array(&self) -> PyResult<[f32; 4]> {
        let color = self.as_ref()?;
        Ok([color.red, color.green, color.blue, color.alpha])
    }

    pub fn to_f32_array_no_alpha(&self) -> PyResult<[f32; 3]> {
        let color = self.as_ref()?;
        Ok([color.red, color.green, color.blue])
    }

    pub fn to_vec4(&self) -> PyResult<pybevy_math::PyVec4> {
        use bevy::math::Vec4;
        let color = self.as_ref()?;
        let vec4 = Vec4::new(color.red, color.green, color.blue, color.alpha);
        Ok(pybevy_math::PyVec4::from(vec4))
    }

    pub fn to_vec3(&self) -> PyResult<pybevy_math::PyVec3> {
        use bevy::math::Vec3;
        let color = self.as_ref()?;
        let vec3 = Vec3::new(color.red, color.green, color.blue);
        Ok(pybevy_math::PyVec3::from(vec3))
    }

    #[staticmethod]
    pub fn from_f32_array(color: [f32; 4]) -> Self {
        PySrgba::from_srgba(Srgba::new(color[0], color[1], color[2], color[3]))
    }

    #[staticmethod]
    pub fn from_f32_array_no_alpha(color: [f32; 3]) -> Self {
        PySrgba::from_srgba(Srgba::rgb(color[0], color[1], color[2]))
    }

    #[staticmethod]
    pub fn from_vec4(color: &pybevy_math::PyVec4) -> Self {
        let v: bevy::math::Vec4 = color.into();
        PySrgba::from_srgba(Srgba::new(v.x, v.y, v.z, v.w))
    }

    #[staticmethod]
    pub fn from_vec3(color: &pybevy_math::PyVec3) -> Self {
        let v: bevy::math::Vec3 = color.into();
        PySrgba::from_srgba(Srgba::rgb(v.x, v.y, v.z))
    }

    pub fn to_u8_array(&self) -> PyResult<[u8; 4]> {
        let color = self.as_ref()?;
        Ok([
            (color.red * 255.0) as u8,
            (color.green * 255.0) as u8,
            (color.blue * 255.0) as u8,
            (color.alpha * 255.0) as u8,
        ])
    }

    pub fn to_u8_array_no_alpha(&self) -> PyResult<[u8; 3]> {
        let color = self.as_ref()?;
        Ok([
            (color.red * 255.0) as u8,
            (color.green * 255.0) as u8,
            (color.blue * 255.0) as u8,
        ])
    }

    #[staticmethod]
    pub fn from_u8_array(color: [u8; 4]) -> Self {
        PySrgba::from_srgba(Srgba::new(
            color[0] as f32 / 255.0,
            color[1] as f32 / 255.0,
            color[2] as f32 / 255.0,
            color[3] as f32 / 255.0,
        ))
    }

    #[staticmethod]
    pub fn from_u8_array_no_alpha(color: [u8; 3]) -> Self {
        PySrgba::from_srgba(Srgba::rgb(
            color[0] as f32 / 255.0,
            color[1] as f32 / 255.0,
            color[2] as f32 / 255.0,
        ))
    }

    pub fn interpolate_stable(&self, other: &Self, t: f32) -> PyResult<Self> {
        Ok(PySrgba::from_srgba(
            self.as_ref()?.interpolate_stable(other.as_ref()?, t),
        ))
    }

    #[pyo3(name = "set_alpha")]
    pub fn method_set_alpha(&mut self, alpha: f32) -> PyResult<()> {
        self.as_mut()?.set_alpha(alpha);
        Ok(())
    }

    #[staticmethod]
    pub fn gamma_function(value: f32) -> f32 {
        Srgba::gamma_function(value)
    }

    #[staticmethod]
    pub fn gamma_function_inverse(value: f32) -> f32 {
        Srgba::gamma_function_inverse(value)
    }
}

#[pyclass(name = "Hsla", eq)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyHsla {
    storage: ValueStorage<Hsla>,
}

impl From<PyHsla> for Hsla {
    #[inline(always)]
    fn from(py_color: PyHsla) -> Self {
        py_color.storage.get().unwrap()
    }
}

impl From<&PyHsla> for Hsla {
    #[inline(always)]
    fn from(py_color: &PyHsla) -> Self {
        py_color.storage.get().unwrap()
    }
}

impl From<Hsla> for PyHsla {
    #[inline(always)]
    fn from(color: Hsla) -> Self {
        PyHsla::from_hsla(color)
    }
}

impl PyHsla {
    #[inline(always)]
    pub fn from_hsla(color: Hsla) -> Self {
        PyHsla {
            storage: ValueStorage::owned(color),
        }
    }

    #[inline(always)]
    pub const fn hsla(color: Hsla) -> Self {
        PyHsla {
            storage: ValueStorage::owned(color),
        }
    }

    #[inline(always)]
    fn as_ref(&self) -> PyResult<&Hsla> {
        Ok(self.storage.as_ref()?)
    }

    #[inline(always)]
    fn as_mut(&mut self) -> PyResult<&mut Hsla> {
        Ok(self.storage.as_mut()?)
    }
}

#[pymethods]
impl PyHsla {
    #[new]
    #[pyo3(signature = (hue = 0.0, saturation = 0.0, lightness = 1.0, alpha = 1.0))]
    pub fn new(hue: f32, saturation: f32, lightness: f32, alpha: f32) -> Self {
        PyHsla::hsla(Hsla::new(hue, saturation, lightness, alpha))
    }

    #[staticmethod]
    pub fn hsl(hue: f32, saturation: f32, lightness: f32) -> Self {
        PyHsla::hsla(Hsla::hsl(hue, saturation, lightness))
    }

    #[staticmethod]
    pub fn gray(lightness: f32) -> Self {
        PyHsla::hsla(Hsla::gray(lightness))
    }

    #[getter]
    pub fn hue(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.hue)
    }

    #[setter]
    pub fn set_hue(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.hue = value;
        Ok(())
    }

    #[getter]
    pub fn saturation(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.saturation)
    }

    #[setter]
    pub fn set_saturation(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.saturation = value;
        Ok(())
    }

    #[getter]
    pub fn lightness(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.lightness)
    }

    #[setter]
    pub fn set_lightness(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.lightness = value;
        Ok(())
    }

    #[getter]
    pub fn alpha(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.alpha)
    }

    #[setter]
    pub fn set_alpha(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.alpha = value;
        Ok(())
    }

    pub fn with_hue(&self, hue: f32) -> PyResult<Self> {
        Ok(PyHsla::hsla(self.as_ref()?.with_hue(hue)))
    }

    pub fn rotate_hue(&self, degrees: f32) -> PyResult<Self> {
        Ok(PyHsla::hsla(self.as_ref()?.rotate_hue(degrees)))
    }

    pub fn with_saturation(&self, saturation: f32) -> PyResult<Self> {
        Ok(PyHsla::hsla(self.as_ref()?.with_saturation(saturation)))
    }

    pub fn with_lightness(&self, lightness: f32) -> PyResult<Self> {
        Ok(PyHsla::hsla(self.as_ref()?.with_lightness(lightness)))
    }

    pub fn luminance(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.luminance())
    }

    pub fn with_luminance(&self, lightness: f32) -> PyResult<Self> {
        Ok(PyHsla::hsla(self.as_ref()?.with_luminance(lightness)))
    }

    pub fn darker(&self, amount: f32) -> PyResult<Self> {
        Ok(PyHsla::hsla(self.as_ref()?.darker(amount)))
    }

    pub fn lighter(&self, amount: f32) -> PyResult<Self> {
        Ok(PyHsla::hsla(self.as_ref()?.lighter(amount)))
    }

    pub fn with_alpha(&self, alpha: f32) -> PyResult<Self> {
        Ok(PyHsla::hsla(self.as_ref()?.with_alpha(alpha)))
    }

    pub fn is_fully_transparent(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_fully_transparent())
    }

    pub fn is_fully_opaque(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_fully_opaque())
    }

    pub fn mix(&self, other: &Self, factor: f32) -> PyResult<Self> {
        Ok(PyHsla::hsla(self.as_ref()?.mix(other.as_ref()?, factor)))
    }

    pub fn mix_assign(&mut self, other: &Self, factor: f32) -> PyResult<()> {
        let result = self.as_ref()?.mix(other.as_ref()?, factor);
        *self.as_mut()? = result;
        Ok(())
    }

    pub fn to_linear(&self) -> PyResult<PyLinearRgba> {
        let linear: LinearRgba = (*self.as_ref()?).into();
        Ok(PyLinearRgba::from_linear_rgba(linear))
    }

    pub fn to_srgba(&self) -> PyResult<PySrgba> {
        let srgba: Srgba = (*self.as_ref()?).into();
        Ok(PySrgba::from_srgba(srgba))
    }

    pub fn to_f32_array(&self) -> PyResult<[f32; 4]> {
        let c = self.as_ref()?;
        Ok([c.hue, c.saturation, c.lightness, c.alpha])
    }

    pub fn to_f32_array_no_alpha(&self) -> PyResult<[f32; 3]> {
        let c = self.as_ref()?;
        Ok([c.hue, c.saturation, c.lightness])
    }

    #[staticmethod]
    pub fn from_f32_array(color: [f32; 4]) -> Self {
        PyHsla::hsla(Hsla::new(color[0], color[1], color[2], color[3]))
    }

    pub fn to_vec4(&self) -> PyResult<pybevy_math::PyVec4> {
        use bevy::math::Vec4;
        let c = self.as_ref()?;
        let vec4 = Vec4::new(c.hue, c.saturation, c.lightness, c.alpha);
        Ok(pybevy_math::PyVec4::from(vec4))
    }

    #[staticmethod]
    pub fn from_vec4(color: &pybevy_math::PyVec4) -> Self {
        let v: bevy::math::Vec4 = color.into();
        PyHsla::hsla(Hsla::new(v.x, v.y, v.z, v.w))
    }

    pub fn to_vec3(&self) -> PyResult<pybevy_math::PyVec3> {
        use bevy::math::Vec3;
        let c = self.as_ref()?;
        let vec3 = Vec3::new(c.hue, c.saturation, c.lightness);
        Ok(pybevy_math::PyVec3::from(vec3))
    }

    #[staticmethod]
    pub fn from_vec3(color: &pybevy_math::PyVec3) -> Self {
        let v: bevy::math::Vec3 = color.into();
        PyHsla::hsla(Hsla::new(v.x, v.y, v.z, 1.0))
    }

    #[staticmethod]
    pub fn from_f32_array_no_alpha(color: [f32; 3]) -> Self {
        PyHsla::hsla(Hsla::new(color[0], color[1], color[2], 1.0))
    }

    #[staticmethod]
    pub fn sequential_dispersed(index: u32) -> Self {
        PyHsla::hsla(Hsla::sequential_dispersed(index))
    }

    #[pyo3(name = "set_alpha")]
    pub fn method_set_alpha(&mut self, alpha: f32) -> PyResult<()> {
        self.as_mut()?.set_alpha(alpha);
        Ok(())
    }

    #[pyo3(name = "set_hue")]
    pub fn method_set_hue(&mut self, hue: f32) -> PyResult<()> {
        self.as_mut()?.set_hue(hue);
        Ok(())
    }

    #[pyo3(name = "set_saturation")]
    pub fn method_set_saturation(&mut self, saturation: f32) -> PyResult<()> {
        self.as_mut()?.set_saturation(saturation);
        Ok(())
    }
}

#[pyclass(name = "Oklcha", eq)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyOklcha {
    storage: ValueStorage<Oklcha>,
}

impl From<PyOklcha> for Oklcha {
    #[inline(always)]
    fn from(py_color: PyOklcha) -> Self {
        py_color.storage.get().unwrap()
    }
}

impl From<&PyOklcha> for Oklcha {
    #[inline(always)]
    fn from(py_color: &PyOklcha) -> Self {
        py_color.storage.get().unwrap()
    }
}

impl From<Oklcha> for PyOklcha {
    #[inline(always)]
    fn from(color: Oklcha) -> Self {
        PyOklcha::from_oklcha(color)
    }
}

impl PyOklcha {
    #[inline(always)]
    pub fn from_oklcha(color: Oklcha) -> Self {
        PyOklcha {
            storage: ValueStorage::owned(color),
        }
    }

    #[inline(always)]
    pub const fn oklcha(color: Oklcha) -> Self {
        PyOklcha {
            storage: ValueStorage::owned(color),
        }
    }

    #[inline(always)]
    fn as_ref(&self) -> PyResult<&Oklcha> {
        Ok(self.storage.as_ref()?)
    }

    #[inline(always)]
    fn as_mut(&mut self) -> PyResult<&mut Oklcha> {
        Ok(self.storage.as_mut()?)
    }
}

#[pymethods]
impl PyOklcha {
    #[new]
    #[pyo3(signature = (lightness = 1.0, chroma = 0.0, hue = 0.0, alpha = 1.0))]
    pub fn new(lightness: f32, chroma: f32, hue: f32, alpha: f32) -> Self {
        PyOklcha::oklcha(Oklcha::new(lightness, chroma, hue, alpha))
    }

    #[staticmethod]
    pub fn lch(lightness: f32, chroma: f32, hue: f32) -> Self {
        PyOklcha::oklcha(Oklcha::lch(lightness, chroma, hue))
    }

    #[staticmethod]
    pub fn gray(lightness: f32) -> Self {
        PyOklcha::oklcha(Oklcha::gray(lightness))
    }

    #[getter]
    pub fn lightness(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.lightness)
    }

    #[setter]
    pub fn set_lightness(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.lightness = value;
        Ok(())
    }

    #[getter]
    pub fn chroma(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.chroma)
    }

    #[setter]
    pub fn set_chroma(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.chroma = value;
        Ok(())
    }

    #[getter]
    pub fn hue(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.hue)
    }

    #[setter]
    pub fn set_hue(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.hue = value;
        Ok(())
    }

    #[getter]
    pub fn alpha(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.alpha)
    }

    #[setter]
    pub fn set_alpha(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.alpha = value;
        Ok(())
    }

    pub fn with_hue(&self, hue: f32) -> PyResult<Self> {
        Ok(PyOklcha::oklcha(self.as_ref()?.with_hue(hue)))
    }

    pub fn rotate_hue(&self, degrees: f32) -> PyResult<Self> {
        Ok(PyOklcha::oklcha(self.as_ref()?.rotate_hue(degrees)))
    }

    pub fn with_lightness(&self, lightness: f32) -> PyResult<Self> {
        Ok(PyOklcha::oklcha(self.as_ref()?.with_lightness(lightness)))
    }

    pub fn with_chroma(&self, chroma: f32) -> PyResult<Self> {
        Ok(PyOklcha::oklcha(self.as_ref()?.with_chroma(chroma)))
    }

    pub fn luminance(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.luminance())
    }

    pub fn with_luminance(&self, lightness: f32) -> PyResult<Self> {
        Ok(PyOklcha::oklcha(self.as_ref()?.with_luminance(lightness)))
    }

    pub fn darker(&self, amount: f32) -> PyResult<Self> {
        Ok(PyOklcha::oklcha(self.as_ref()?.darker(amount)))
    }

    pub fn lighter(&self, amount: f32) -> PyResult<Self> {
        Ok(PyOklcha::oklcha(self.as_ref()?.lighter(amount)))
    }

    pub fn with_alpha(&self, alpha: f32) -> PyResult<Self> {
        Ok(PyOklcha::oklcha(self.as_ref()?.with_alpha(alpha)))
    }

    pub fn is_fully_transparent(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_fully_transparent())
    }

    pub fn is_fully_opaque(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_fully_opaque())
    }

    pub fn mix(&self, other: &PyOklcha, factor: f32) -> PyResult<Self> {
        Ok(PyOklcha::oklcha(
            self.as_ref()?.mix(other.as_ref()?, factor),
        ))
    }

    pub fn mix_assign(&mut self, other: &Self, factor: f32) -> PyResult<()> {
        let result = self.as_ref()?.mix(other.as_ref()?, factor);
        *self.as_mut()? = result;
        Ok(())
    }

    pub fn distance(&self, other: &PyOklcha) -> PyResult<f32> {
        Ok(self.as_ref()?.distance(other.as_ref()?))
    }

    pub fn distance_squared(&self, other: &PyOklcha) -> PyResult<f32> {
        Ok(self.as_ref()?.distance_squared(other.as_ref()?))
    }

    pub fn to_linear(&self) -> PyResult<PyLinearRgba> {
        let linear: LinearRgba = (*self.as_ref()?).into();
        Ok(PyLinearRgba::linear_rgba(linear))
    }

    pub fn to_srgba(&self) -> PyResult<PySrgba> {
        let srgba: Srgba = (*self.as_ref()?).into();
        Ok(PySrgba::from_srgba(srgba))
    }

    pub fn to_f32_array(&self) -> PyResult<[f32; 4]> {
        let c = self.as_ref()?;
        Ok([c.lightness, c.chroma, c.hue, c.alpha])
    }

    pub fn to_f32_array_no_alpha(&self) -> PyResult<[f32; 3]> {
        let c = self.as_ref()?;
        Ok([c.lightness, c.chroma, c.hue])
    }

    #[staticmethod]
    pub fn from_f32_array(color: [f32; 4]) -> Self {
        PyOklcha::oklcha(Oklcha::new(color[0], color[1], color[2], color[3]))
    }

    pub fn to_vec4(&self) -> PyResult<pybevy_math::PyVec4> {
        use bevy::math::Vec4;
        let c = self.as_ref()?;
        let vec4 = Vec4::new(c.lightness, c.chroma, c.hue, c.alpha);
        Ok(pybevy_math::PyVec4::from(vec4))
    }

    #[staticmethod]
    pub fn from_vec4(color: &pybevy_math::PyVec4) -> Self {
        let v: bevy::math::Vec4 = color.into();
        PyOklcha::oklcha(Oklcha::new(v.x, v.y, v.z, v.w))
    }

    pub fn to_vec3(&self) -> PyResult<pybevy_math::PyVec3> {
        use bevy::math::Vec3;
        let c = self.as_ref()?;
        let vec3 = Vec3::new(c.lightness, c.chroma, c.hue);
        Ok(pybevy_math::PyVec3::from(vec3))
    }

    #[staticmethod]
    pub fn from_vec3(color: &pybevy_math::PyVec3) -> Self {
        let v: bevy::math::Vec3 = color.into();
        PyOklcha::oklcha(Oklcha::new(v.x, v.y, v.z, 1.0))
    }

    #[staticmethod]
    pub fn from_f32_array_no_alpha(color: [f32; 3]) -> Self {
        PyOklcha::oklcha(Oklcha::new(color[0], color[1], color[2], 1.0))
    }

    #[staticmethod]
    pub fn sequential_dispersed(index: u32) -> Self {
        PyOklcha::oklcha(Oklcha::sequential_dispersed(index))
    }

    #[pyo3(name = "set_alpha")]
    pub fn method_set_alpha(&mut self, alpha: f32) -> PyResult<()> {
        self.as_mut()?.set_alpha(alpha);
        Ok(())
    }

    #[pyo3(name = "set_hue")]
    pub fn method_set_hue(&mut self, hue: f32) -> PyResult<()> {
        self.as_mut()?.set_hue(hue);
        Ok(())
    }
}

#[pyclass(name = "Lcha", eq)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyLcha {
    storage: ValueStorage<Lcha>,
}

impl From<PyLcha> for Lcha {
    #[inline(always)]
    fn from(py_color: PyLcha) -> Self {
        py_color.storage.get().unwrap()
    }
}

impl From<&PyLcha> for Lcha {
    #[inline(always)]
    fn from(py_color: &PyLcha) -> Self {
        py_color.storage.get().unwrap()
    }
}

impl From<Lcha> for PyLcha {
    #[inline(always)]
    fn from(color: Lcha) -> Self {
        PyLcha::from_lcha(color)
    }
}

impl PyLcha {
    #[inline(always)]
    pub fn from_lcha(color: Lcha) -> Self {
        PyLcha {
            storage: ValueStorage::owned(color),
        }
    }

    #[inline(always)]
    pub const fn lcha(color: Lcha) -> Self {
        PyLcha {
            storage: ValueStorage::owned(color),
        }
    }

    #[inline(always)]
    fn as_ref(&self) -> PyResult<&Lcha> {
        Ok(self.storage.as_ref()?)
    }

    #[inline(always)]
    fn as_mut(&mut self) -> PyResult<&mut Lcha> {
        Ok(self.storage.as_mut()?)
    }
}

#[pymethods]
impl PyLcha {
    #[new]
    #[pyo3(signature = (lightness = 1.0, chroma = 0.0, hue = 0.0, alpha = 1.0))]
    pub fn new(lightness: f32, chroma: f32, hue: f32, alpha: f32) -> Self {
        PyLcha::lcha(Lcha::new(lightness, chroma, hue, alpha))
    }

    #[staticmethod]
    pub fn lch(lightness: f32, chroma: f32, hue: f32) -> Self {
        PyLcha::lcha(Lcha::lch(lightness, chroma, hue))
    }

    #[staticmethod]
    pub fn gray(lightness: f32) -> Self {
        PyLcha::lcha(Lcha::gray(lightness))
    }

    #[getter]
    pub fn lightness(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.lightness)
    }

    #[setter]
    pub fn set_lightness(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.lightness = value;
        Ok(())
    }

    #[getter]
    pub fn chroma(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.chroma)
    }

    #[setter]
    pub fn set_chroma(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.chroma = value;
        Ok(())
    }

    #[getter]
    pub fn hue(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.hue)
    }

    #[setter]
    pub fn set_hue(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.hue = value;
        Ok(())
    }

    #[getter]
    pub fn alpha(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.alpha)
    }

    #[setter]
    pub fn set_alpha(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.alpha = value;
        Ok(())
    }

    pub fn with_hue(&self, hue: f32) -> PyResult<Self> {
        Ok(PyLcha::lcha(self.as_ref()?.with_hue(hue)))
    }

    pub fn rotate_hue(&self, degrees: f32) -> PyResult<Self> {
        Ok(PyLcha::lcha(self.as_ref()?.rotate_hue(degrees)))
    }

    pub fn with_lightness(&self, lightness: f32) -> PyResult<Self> {
        Ok(PyLcha::lcha(self.as_ref()?.with_lightness(lightness)))
    }

    pub fn with_chroma(&self, chroma: f32) -> PyResult<Self> {
        Ok(PyLcha::lcha(self.as_ref()?.with_chroma(chroma)))
    }

    pub fn luminance(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.luminance())
    }

    pub fn with_luminance(&self, lightness: f32) -> PyResult<Self> {
        Ok(PyLcha::lcha(self.as_ref()?.with_luminance(lightness)))
    }

    pub fn darker(&self, amount: f32) -> PyResult<Self> {
        Ok(PyLcha::lcha(self.as_ref()?.darker(amount)))
    }

    pub fn lighter(&self, amount: f32) -> PyResult<Self> {
        Ok(PyLcha::lcha(self.as_ref()?.lighter(amount)))
    }

    pub fn with_alpha(&self, alpha: f32) -> PyResult<Self> {
        Ok(PyLcha::lcha(self.as_ref()?.with_alpha(alpha)))
    }

    pub fn is_fully_transparent(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_fully_transparent())
    }

    pub fn is_fully_opaque(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_fully_opaque())
    }

    pub fn mix(&self, other: &PyLcha, factor: f32) -> PyResult<Self> {
        Ok(PyLcha::lcha(self.as_ref()?.mix(other.as_ref()?, factor)))
    }

    pub fn mix_assign(&mut self, other: &Self, factor: f32) -> PyResult<()> {
        let result = self.as_ref()?.mix(other.as_ref()?, factor);
        *self.as_mut()? = result;
        Ok(())
    }

    pub fn to_linear(&self) -> PyResult<PyLinearRgba> {
        let linear: LinearRgba = (*self.as_ref()?).into();
        Ok(PyLinearRgba::linear_rgba(linear))
    }

    pub fn to_srgba(&self) -> PyResult<PySrgba> {
        let srgba: Srgba = (*self.as_ref()?).into();
        Ok(PySrgba::from_srgba(srgba))
    }

    pub fn to_f32_array(&self) -> PyResult<[f32; 4]> {
        let c = self.as_ref()?;
        Ok([c.lightness, c.chroma, c.hue, c.alpha])
    }

    pub fn to_f32_array_no_alpha(&self) -> PyResult<[f32; 3]> {
        let c = self.as_ref()?;
        Ok([c.lightness, c.chroma, c.hue])
    }

    #[staticmethod]
    pub fn from_f32_array(color: [f32; 4]) -> Self {
        PyLcha::lcha(Lcha::new(color[0], color[1], color[2], color[3]))
    }

    pub fn to_vec4(&self) -> PyResult<pybevy_math::PyVec4> {
        use bevy::math::Vec4;
        let c = self.as_ref()?;
        let vec4 = Vec4::new(c.lightness, c.chroma, c.hue, c.alpha);
        Ok(pybevy_math::PyVec4::from(vec4))
    }

    #[staticmethod]
    pub fn from_vec4(color: &pybevy_math::PyVec4) -> Self {
        let v: bevy::math::Vec4 = color.into();
        PyLcha::lcha(Lcha::new(v.x, v.y, v.z, v.w))
    }

    pub fn to_vec3(&self) -> PyResult<pybevy_math::PyVec3> {
        use bevy::math::Vec3;
        let c = self.as_ref()?;
        let vec3 = Vec3::new(c.lightness, c.chroma, c.hue);
        Ok(pybevy_math::PyVec3::from(vec3))
    }

    #[staticmethod]
    pub fn from_vec3(color: &pybevy_math::PyVec3) -> Self {
        let v: bevy::math::Vec3 = color.into();
        PyLcha::lcha(Lcha::new(v.x, v.y, v.z, 1.0))
    }

    #[staticmethod]
    pub fn from_f32_array_no_alpha(color: [f32; 3]) -> Self {
        PyLcha::lcha(Lcha::new(color[0], color[1], color[2], 1.0))
    }

    #[staticmethod]
    pub fn sequential_dispersed(index: u32) -> Self {
        PyLcha::lcha(Lcha::sequential_dispersed(index))
    }

    #[pyo3(name = "set_alpha")]
    pub fn method_set_alpha(&mut self, alpha: f32) -> PyResult<()> {
        self.as_mut()?.set_alpha(alpha);
        Ok(())
    }

    #[pyo3(name = "set_hue")]
    pub fn method_set_hue(&mut self, hue: f32) -> PyResult<()> {
        self.as_mut()?.set_hue(hue);
        Ok(())
    }
}

#[pyclass(name = "Hsva", eq)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyHsva {
    storage: ValueStorage<Hsva>,
}

impl From<PyHsva> for Hsva {
    #[inline(always)]
    fn from(py_color: PyHsva) -> Self {
        py_color.storage.get().unwrap()
    }
}

impl From<&PyHsva> for Hsva {
    #[inline(always)]
    fn from(py_color: &PyHsva) -> Self {
        py_color.storage.get().unwrap()
    }
}

impl From<Hsva> for PyHsva {
    #[inline(always)]
    fn from(color: Hsva) -> Self {
        PyHsva::from_hsva(color)
    }
}

impl PyHsva {
    #[inline(always)]
    pub fn from_hsva(color: Hsva) -> Self {
        PyHsva {
            storage: ValueStorage::owned(color),
        }
    }

    #[inline(always)]
    pub const fn hsva(color: Hsva) -> Self {
        PyHsva {
            storage: ValueStorage::owned(color),
        }
    }

    #[inline(always)]
    fn as_ref(&self) -> PyResult<&Hsva> {
        Ok(self.storage.as_ref()?)
    }

    #[inline(always)]
    fn as_mut(&mut self) -> PyResult<&mut Hsva> {
        Ok(self.storage.as_mut()?)
    }
}

#[pymethods]
impl PyHsva {
    #[new]
    #[pyo3(signature = (hue = 0.0, saturation = 0.0, value = 1.0, alpha = 1.0))]
    pub fn new(hue: f32, saturation: f32, value: f32, alpha: f32) -> Self {
        PyHsva::hsva(Hsva::new(hue, saturation, value, alpha))
    }

    #[staticmethod]
    pub fn hsv(hue: f32, saturation: f32, value: f32) -> Self {
        PyHsva::hsva(Hsva::hsv(hue, saturation, value))
    }

    #[staticmethod]
    pub fn gray(lightness: f32) -> Self {
        PyHsva::hsva(Hsva::gray(lightness))
    }

    #[getter]
    pub fn hue(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.hue)
    }

    #[setter]
    pub fn set_hue(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.hue = value;
        Ok(())
    }

    #[getter]
    pub fn saturation(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.saturation)
    }

    #[setter]
    pub fn set_saturation(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.saturation = value;
        Ok(())
    }

    #[getter]
    pub fn value(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.value)
    }

    #[setter]
    pub fn set_value(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.value = value;
        Ok(())
    }

    #[getter]
    pub fn alpha(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.alpha)
    }

    #[setter]
    pub fn set_alpha(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.alpha = value;
        Ok(())
    }

    pub fn with_hue(&self, hue: f32) -> PyResult<Self> {
        Ok(PyHsva::hsva(self.as_ref()?.with_hue(hue)))
    }

    pub fn rotate_hue(&self, degrees: f32) -> PyResult<Self> {
        Ok(PyHsva::hsva(self.as_ref()?.rotate_hue(degrees)))
    }

    pub fn with_saturation(&self, saturation: f32) -> PyResult<Self> {
        Ok(PyHsva::hsva(self.as_ref()?.with_saturation(saturation)))
    }

    pub fn with_value(&self, value: f32) -> PyResult<Self> {
        Ok(PyHsva::hsva(self.as_ref()?.with_value(value)))
    }

    pub fn with_alpha(&self, alpha: f32) -> PyResult<Self> {
        Ok(PyHsva::hsva(self.as_ref()?.with_alpha(alpha)))
    }

    pub fn is_fully_transparent(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_fully_transparent())
    }

    pub fn is_fully_opaque(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_fully_opaque())
    }

    pub fn mix(&self, other: &PyHsva, factor: f32) -> PyResult<Self> {
        Ok(PyHsva::hsva(self.as_ref()?.mix(other.as_ref()?, factor)))
    }

    pub fn mix_assign(&mut self, other: &Self, factor: f32) -> PyResult<()> {
        let result = self.as_ref()?.mix(other.as_ref()?, factor);
        *self.as_mut()? = result;
        Ok(())
    }

    pub fn to_linear(&self) -> PyResult<PyLinearRgba> {
        let linear: LinearRgba = (*self.as_ref()?).into();
        Ok(PyLinearRgba::linear_rgba(linear))
    }

    pub fn to_srgba(&self) -> PyResult<PySrgba> {
        let srgba: Srgba = (*self.as_ref()?).into();
        Ok(PySrgba::from_srgba(srgba))
    }

    pub fn to_f32_array(&self) -> PyResult<[f32; 4]> {
        let c = self.as_ref()?;
        Ok([c.hue, c.saturation, c.value, c.alpha])
    }

    pub fn to_f32_array_no_alpha(&self) -> PyResult<[f32; 3]> {
        let c = self.as_ref()?;
        Ok([c.hue, c.saturation, c.value])
    }

    #[staticmethod]
    pub fn from_f32_array(color: [f32; 4]) -> Self {
        PyHsva::hsva(Hsva::new(color[0], color[1], color[2], color[3]))
    }

    pub fn to_vec4(&self) -> PyResult<pybevy_math::PyVec4> {
        use bevy::math::Vec4;
        let c = self.as_ref()?;
        let vec4 = Vec4::new(c.hue, c.saturation, c.value, c.alpha);
        Ok(pybevy_math::PyVec4::from(vec4))
    }

    #[staticmethod]
    pub fn from_vec4(color: &pybevy_math::PyVec4) -> Self {
        let v: bevy::math::Vec4 = color.into();
        PyHsva::hsva(Hsva::new(v.x, v.y, v.z, v.w))
    }

    pub fn to_vec3(&self) -> PyResult<pybevy_math::PyVec3> {
        use bevy::math::Vec3;
        let c = self.as_ref()?;
        let vec3 = Vec3::new(c.hue, c.saturation, c.value);
        Ok(pybevy_math::PyVec3::from(vec3))
    }

    #[staticmethod]
    pub fn from_vec3(color: &pybevy_math::PyVec3) -> Self {
        let v: bevy::math::Vec3 = color.into();
        PyHsva::hsva(Hsva::new(v.x, v.y, v.z, 1.0))
    }

    #[staticmethod]
    pub fn from_f32_array_no_alpha(color: [f32; 3]) -> Self {
        PyHsva::hsva(Hsva::new(color[0], color[1], color[2], 1.0))
    }

    #[pyo3(name = "set_alpha")]
    pub fn method_set_alpha(&mut self, alpha: f32) -> PyResult<()> {
        self.as_mut()?.set_alpha(alpha);
        Ok(())
    }

    #[pyo3(name = "set_hue")]
    pub fn method_set_hue(&mut self, hue: f32) -> PyResult<()> {
        self.as_mut()?.set_hue(hue);
        Ok(())
    }

    #[pyo3(name = "set_saturation")]
    pub fn method_set_saturation(&mut self, saturation: f32) -> PyResult<()> {
        self.as_mut()?.set_saturation(saturation);
        Ok(())
    }
}

// === Laba ===

#[pyclass(name = "Laba", eq)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyLaba {
    storage: ValueStorage<Laba>,
}

impl PyLaba {
    pub fn laba(laba: Laba) -> Self {
        Self {
            storage: ValueStorage::owned(laba),
        }
    }

    fn as_ref(&self) -> PyResult<&Laba> {
        Ok(self.storage.as_ref()?)
    }

    fn as_mut(&mut self) -> PyResult<&mut Laba> {
        Ok(self.storage.as_mut()?)
    }
}

#[pymethods]
impl PyLaba {
    #[new]
    #[pyo3(signature = (lightness = 1.0, a = 0.0, b = 0.0, alpha = 1.0))]
    pub fn new(lightness: f32, a: f32, b: f32, alpha: f32) -> Self {
        PyLaba::laba(Laba::new(lightness, a, b, alpha))
    }

    #[staticmethod]
    pub fn lab(lightness: f32, a: f32, b: f32) -> Self {
        PyLaba::laba(Laba::lab(lightness, a, b))
    }

    #[staticmethod]
    pub fn gray(lightness: f32) -> Self {
        PyLaba::laba(Laba::gray(lightness))
    }

    #[staticmethod]
    #[pyo3(name = "BLACK")]
    pub fn black() -> Self {
        PyLaba::laba(Laba::BLACK)
    }

    #[staticmethod]
    #[pyo3(name = "WHITE")]
    pub fn white() -> Self {
        PyLaba::laba(Laba::WHITE)
    }

    #[getter]
    pub fn lightness(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.lightness)
    }

    #[setter]
    pub fn set_lightness(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.lightness = value;
        Ok(())
    }

    #[getter]
    pub fn a(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.a)
    }

    #[setter]
    pub fn set_a(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.a = value;
        Ok(())
    }

    #[getter]
    pub fn b(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.b)
    }

    #[setter]
    pub fn set_b(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.b = value;
        Ok(())
    }

    #[getter]
    pub fn alpha(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.alpha)
    }

    #[setter]
    pub fn set_alpha(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.alpha = value;
        Ok(())
    }

    #[pyo3(name = "set_alpha")]
    pub fn method_set_alpha(&mut self, alpha: f32) -> PyResult<()> {
        self.as_mut()?.set_alpha(alpha);
        Ok(())
    }

    pub fn with_lightness(&self, lightness: f32) -> PyResult<Self> {
        Ok(PyLaba::laba(self.as_ref()?.with_lightness(lightness)))
    }

    pub fn with_alpha(&self, alpha: f32) -> PyResult<Self> {
        Ok(PyLaba::laba(self.as_ref()?.with_alpha(alpha)))
    }

    pub fn luminance(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.luminance())
    }

    pub fn with_luminance(&self, lightness: f32) -> PyResult<Self> {
        Ok(PyLaba::laba(self.as_ref()?.with_luminance(lightness)))
    }

    pub fn darker(&self, amount: f32) -> PyResult<Self> {
        Ok(PyLaba::laba(self.as_ref()?.darker(amount)))
    }

    pub fn lighter(&self, amount: f32) -> PyResult<Self> {
        Ok(PyLaba::laba(self.as_ref()?.lighter(amount)))
    }

    pub fn is_fully_transparent(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_fully_transparent())
    }

    pub fn is_fully_opaque(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_fully_opaque())
    }

    pub fn mix(&self, other: &PyLaba, factor: f32) -> PyResult<Self> {
        Ok(PyLaba::laba(self.as_ref()?.mix(other.as_ref()?, factor)))
    }

    pub fn mix_assign(&mut self, other: &Self, factor: f32) -> PyResult<()> {
        let result = self.as_ref()?.mix(other.as_ref()?, factor);
        *self.as_mut()? = result;
        Ok(())
    }

    pub fn to_linear(&self) -> PyResult<PyLinearRgba> {
        let linear: LinearRgba = (*self.as_ref()?).into();
        Ok(PyLinearRgba::linear_rgba(linear))
    }

    pub fn to_srgba(&self) -> PyResult<PySrgba> {
        let srgba: Srgba = (*self.as_ref()?).into();
        Ok(PySrgba::from_srgba(srgba))
    }

    pub fn to_f32_array(&self) -> PyResult<[f32; 4]> {
        let c = self.as_ref()?;
        Ok([c.lightness, c.a, c.b, c.alpha])
    }

    pub fn to_f32_array_no_alpha(&self) -> PyResult<[f32; 3]> {
        let c = self.as_ref()?;
        Ok([c.lightness, c.a, c.b])
    }

    #[staticmethod]
    pub fn from_f32_array(color: [f32; 4]) -> Self {
        PyLaba::laba(Laba::new(color[0], color[1], color[2], color[3]))
    }

    #[staticmethod]
    pub fn from_f32_array_no_alpha(color: [f32; 3]) -> Self {
        PyLaba::laba(Laba::lab(color[0], color[1], color[2]))
    }

    pub fn to_vec4(&self) -> PyResult<pybevy_math::PyVec4> {
        use bevy::math::Vec4;
        let c = self.as_ref()?;
        let vec4 = Vec4::new(c.lightness, c.a, c.b, c.alpha);
        Ok(pybevy_math::PyVec4::from(vec4))
    }

    pub fn to_vec3(&self) -> PyResult<pybevy_math::PyVec3> {
        use bevy::math::Vec3;
        let c = self.as_ref()?;
        let vec3 = Vec3::new(c.lightness, c.a, c.b);
        Ok(pybevy_math::PyVec3::from(vec3))
    }

    #[staticmethod]
    pub fn from_vec4(color: &pybevy_math::PyVec4) -> Self {
        let v: bevy::math::Vec4 = color.into();
        PyLaba::laba(Laba::new(v.x, v.y, v.z, v.w))
    }

    #[staticmethod]
    pub fn from_vec3(color: &pybevy_math::PyVec3) -> Self {
        let v: bevy::math::Vec3 = color.into();
        PyLaba::laba(Laba::lab(v.x, v.y, v.z))
    }

    pub fn interpolate_stable(&self, other: &PyLaba, t: f32) -> PyResult<Self> {
        Ok(PyLaba::laba(
            self.as_ref()?.interpolate_stable(other.as_ref()?, t),
        ))
    }
}

// === Oklaba ===

#[pyclass(name = "Oklaba", eq)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyOklaba {
    storage: ValueStorage<Oklaba>,
}

impl PyOklaba {
    pub fn oklaba(oklaba: Oklaba) -> Self {
        Self {
            storage: ValueStorage::owned(oklaba),
        }
    }

    fn as_ref(&self) -> PyResult<&Oklaba> {
        Ok(self.storage.as_ref()?)
    }

    fn as_mut(&mut self) -> PyResult<&mut Oklaba> {
        Ok(self.storage.as_mut()?)
    }
}

#[pymethods]
impl PyOklaba {
    #[new]
    #[pyo3(signature = (lightness = 1.0, a = 0.0, b = 0.0, alpha = 1.0))]
    pub fn new(lightness: f32, a: f32, b: f32, alpha: f32) -> Self {
        PyOklaba::oklaba(Oklaba::new(lightness, a, b, alpha))
    }

    #[staticmethod]
    pub fn lab(lightness: f32, a: f32, b: f32) -> Self {
        PyOklaba::oklaba(Oklaba::lab(lightness, a, b))
    }

    #[staticmethod]
    pub fn gray(lightness: f32) -> Self {
        PyOklaba::oklaba(Oklaba::gray(lightness))
    }

    #[staticmethod]
    #[pyo3(name = "BLACK")]
    pub fn black() -> Self {
        PyOklaba::oklaba(Oklaba::BLACK)
    }

    #[staticmethod]
    #[pyo3(name = "WHITE")]
    pub fn white() -> Self {
        PyOklaba::oklaba(Oklaba::WHITE)
    }

    #[getter]
    pub fn lightness(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.lightness)
    }

    #[setter]
    pub fn set_lightness(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.lightness = value;
        Ok(())
    }

    #[getter]
    pub fn a(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.a)
    }

    #[setter]
    pub fn set_a(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.a = value;
        Ok(())
    }

    #[getter]
    pub fn b(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.b)
    }

    #[setter]
    pub fn set_b(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.b = value;
        Ok(())
    }

    #[getter]
    pub fn alpha(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.alpha)
    }

    #[setter]
    pub fn set_alpha(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.alpha = value;
        Ok(())
    }

    #[pyo3(name = "set_alpha")]
    pub fn method_set_alpha(&mut self, alpha: f32) -> PyResult<()> {
        self.as_mut()?.set_alpha(alpha);
        Ok(())
    }

    pub fn with_lightness(&self, lightness: f32) -> PyResult<Self> {
        Ok(PyOklaba::oklaba(self.as_ref()?.with_lightness(lightness)))
    }

    pub fn with_a(&self, a: f32) -> PyResult<Self> {
        let mut copy = *self.as_ref()?;
        copy.a = a;
        Ok(PyOklaba::oklaba(copy))
    }

    pub fn with_b(&self, b: f32) -> PyResult<Self> {
        let mut copy = *self.as_ref()?;
        copy.b = b;
        Ok(PyOklaba::oklaba(copy))
    }

    pub fn with_alpha(&self, alpha: f32) -> PyResult<Self> {
        Ok(PyOklaba::oklaba(self.as_ref()?.with_alpha(alpha)))
    }

    pub fn luminance(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.luminance())
    }

    pub fn with_luminance(&self, lightness: f32) -> PyResult<Self> {
        Ok(PyOklaba::oklaba(self.as_ref()?.with_luminance(lightness)))
    }

    pub fn darker(&self, amount: f32) -> PyResult<Self> {
        Ok(PyOklaba::oklaba(self.as_ref()?.darker(amount)))
    }

    pub fn lighter(&self, amount: f32) -> PyResult<Self> {
        Ok(PyOklaba::oklaba(self.as_ref()?.lighter(amount)))
    }

    pub fn is_fully_transparent(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_fully_transparent())
    }

    pub fn is_fully_opaque(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_fully_opaque())
    }

    pub fn mix(&self, other: &PyOklaba, factor: f32) -> PyResult<Self> {
        Ok(PyOklaba::oklaba(
            self.as_ref()?.mix(other.as_ref()?, factor),
        ))
    }

    pub fn mix_assign(&mut self, other: &Self, factor: f32) -> PyResult<()> {
        let result = self.as_ref()?.mix(other.as_ref()?, factor);
        *self.as_mut()? = result;
        Ok(())
    }

    pub fn distance(&self, other: &PyOklaba) -> PyResult<f32> {
        Ok(self.as_ref()?.distance(other.as_ref()?))
    }

    pub fn distance_squared(&self, other: &PyOklaba) -> PyResult<f32> {
        Ok(self.as_ref()?.distance_squared(other.as_ref()?))
    }

    pub fn to_linear(&self) -> PyResult<PyLinearRgba> {
        let linear: LinearRgba = (*self.as_ref()?).into();
        Ok(PyLinearRgba::linear_rgba(linear))
    }

    pub fn to_srgba(&self) -> PyResult<PySrgba> {
        let srgba: Srgba = (*self.as_ref()?).into();
        Ok(PySrgba::from_srgba(srgba))
    }

    pub fn to_f32_array(&self) -> PyResult<[f32; 4]> {
        let c = self.as_ref()?;
        Ok([c.lightness, c.a, c.b, c.alpha])
    }

    pub fn to_f32_array_no_alpha(&self) -> PyResult<[f32; 3]> {
        let c = self.as_ref()?;
        Ok([c.lightness, c.a, c.b])
    }

    #[staticmethod]
    pub fn from_f32_array(color: [f32; 4]) -> Self {
        PyOklaba::oklaba(Oklaba::new(color[0], color[1], color[2], color[3]))
    }

    #[staticmethod]
    pub fn from_f32_array_no_alpha(color: [f32; 3]) -> Self {
        PyOklaba::oklaba(Oklaba::lab(color[0], color[1], color[2]))
    }

    pub fn to_vec4(&self) -> PyResult<pybevy_math::PyVec4> {
        use bevy::math::Vec4;
        let c = self.as_ref()?;
        let vec4 = Vec4::new(c.lightness, c.a, c.b, c.alpha);
        Ok(pybevy_math::PyVec4::from(vec4))
    }

    pub fn to_vec3(&self) -> PyResult<pybevy_math::PyVec3> {
        use bevy::math::Vec3;
        let c = self.as_ref()?;
        let vec3 = Vec3::new(c.lightness, c.a, c.b);
        Ok(pybevy_math::PyVec3::from(vec3))
    }

    #[staticmethod]
    pub fn from_vec4(color: &pybevy_math::PyVec4) -> Self {
        let v: bevy::math::Vec4 = color.into();
        PyOklaba::oklaba(Oklaba::new(v.x, v.y, v.z, v.w))
    }

    #[staticmethod]
    pub fn from_vec3(color: &pybevy_math::PyVec3) -> Self {
        let v: bevy::math::Vec3 = color.into();
        PyOklaba::oklaba(Oklaba::lab(v.x, v.y, v.z))
    }

    pub fn interpolate_stable(&self, other: &PyOklaba, t: f32) -> PyResult<Self> {
        Ok(PyOklaba::oklaba(
            self.as_ref()?.interpolate_stable(other.as_ref()?, t),
        ))
    }
}

// === Xyza ===

#[pyclass(name = "Xyza", eq)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyXyza {
    storage: ValueStorage<Xyza>,
}

impl PyXyza {
    pub fn xyza(xyza: Xyza) -> Self {
        Self {
            storage: ValueStorage::owned(xyza),
        }
    }

    fn as_ref(&self) -> PyResult<&Xyza> {
        Ok(self.storage.as_ref()?)
    }

    fn as_mut(&mut self) -> PyResult<&mut Xyza> {
        Ok(self.storage.as_mut()?)
    }
}

#[pymethods]
impl PyXyza {
    #[new]
    #[pyo3(signature = (x = 0.0, y = 0.0, z = 0.0, alpha = 1.0))]
    pub fn new(x: f32, y: f32, z: f32, alpha: f32) -> Self {
        PyXyza::xyza(Xyza::new(x, y, z, alpha))
    }

    #[staticmethod]
    pub fn xyz(x: f32, y: f32, z: f32) -> Self {
        PyXyza::xyza(Xyza::xyz(x, y, z))
    }

    #[staticmethod]
    pub fn gray(lightness: f32) -> Self {
        PyXyza::xyza(Xyza::gray(lightness))
    }

    #[staticmethod]
    #[pyo3(name = "BLACK")]
    pub fn black() -> Self {
        PyXyza::xyza(Xyza::BLACK)
    }

    #[staticmethod]
    #[pyo3(name = "WHITE")]
    pub fn white() -> Self {
        PyXyza::xyza(Xyza::WHITE)
    }

    #[getter]
    pub fn x(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.x)
    }

    #[setter]
    pub fn set_x(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.x = value;
        Ok(())
    }

    #[getter]
    pub fn y(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.y)
    }

    #[setter]
    pub fn set_y(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.y = value;
        Ok(())
    }

    #[getter]
    pub fn z(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.z)
    }

    #[setter]
    pub fn set_z(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.z = value;
        Ok(())
    }

    #[getter]
    pub fn alpha(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.alpha)
    }

    #[setter]
    pub fn set_alpha(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.alpha = value;
        Ok(())
    }

    #[pyo3(name = "set_alpha")]
    pub fn method_set_alpha(&mut self, alpha: f32) -> PyResult<()> {
        self.as_mut()?.set_alpha(alpha);
        Ok(())
    }

    pub fn with_x(&self, x: f32) -> PyResult<Self> {
        let mut copy = *self.as_ref()?;
        copy.x = x;
        Ok(PyXyza::xyza(copy))
    }

    pub fn with_y(&self, y: f32) -> PyResult<Self> {
        let mut copy = *self.as_ref()?;
        copy.y = y;
        Ok(PyXyza::xyza(copy))
    }

    pub fn with_z(&self, z: f32) -> PyResult<Self> {
        let mut copy = *self.as_ref()?;
        copy.z = z;
        Ok(PyXyza::xyza(copy))
    }

    pub fn with_alpha(&self, alpha: f32) -> PyResult<Self> {
        Ok(PyXyza::xyza(self.as_ref()?.with_alpha(alpha)))
    }

    pub fn luminance(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.luminance())
    }

    pub fn with_luminance(&self, lightness: f32) -> PyResult<Self> {
        Ok(PyXyza::xyza(self.as_ref()?.with_luminance(lightness)))
    }

    pub fn darker(&self, amount: f32) -> PyResult<Self> {
        Ok(PyXyza::xyza(self.as_ref()?.darker(amount)))
    }

    pub fn lighter(&self, amount: f32) -> PyResult<Self> {
        Ok(PyXyza::xyza(self.as_ref()?.lighter(amount)))
    }

    pub fn is_fully_transparent(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_fully_transparent())
    }

    pub fn is_fully_opaque(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_fully_opaque())
    }

    pub fn mix(&self, other: &PyXyza, factor: f32) -> PyResult<Self> {
        Ok(PyXyza::xyza(self.as_ref()?.mix(other.as_ref()?, factor)))
    }

    pub fn mix_assign(&mut self, other: &Self, factor: f32) -> PyResult<()> {
        let result = self.as_ref()?.mix(other.as_ref()?, factor);
        *self.as_mut()? = result;
        Ok(())
    }

    pub fn to_linear(&self) -> PyResult<PyLinearRgba> {
        let linear: LinearRgba = (*self.as_ref()?).into();
        Ok(PyLinearRgba::linear_rgba(linear))
    }

    pub fn to_srgba(&self) -> PyResult<PySrgba> {
        let srgba: Srgba = (*self.as_ref()?).into();
        Ok(PySrgba::from_srgba(srgba))
    }

    pub fn to_f32_array(&self) -> PyResult<[f32; 4]> {
        let c = self.as_ref()?;
        Ok([c.x, c.y, c.z, c.alpha])
    }

    pub fn to_f32_array_no_alpha(&self) -> PyResult<[f32; 3]> {
        let c = self.as_ref()?;
        Ok([c.x, c.y, c.z])
    }

    #[staticmethod]
    pub fn from_f32_array(color: [f32; 4]) -> Self {
        PyXyza::xyza(Xyza::new(color[0], color[1], color[2], color[3]))
    }

    #[staticmethod]
    pub fn from_f32_array_no_alpha(color: [f32; 3]) -> Self {
        PyXyza::xyza(Xyza::xyz(color[0], color[1], color[2]))
    }

    pub fn to_vec4(&self) -> PyResult<pybevy_math::PyVec4> {
        use bevy::math::Vec4;
        let c = self.as_ref()?;
        let vec4 = Vec4::new(c.x, c.y, c.z, c.alpha);
        Ok(pybevy_math::PyVec4::from(vec4))
    }

    pub fn to_vec3(&self) -> PyResult<pybevy_math::PyVec3> {
        use bevy::math::Vec3;
        let c = self.as_ref()?;
        let vec3 = Vec3::new(c.x, c.y, c.z);
        Ok(pybevy_math::PyVec3::from(vec3))
    }

    #[staticmethod]
    pub fn from_vec4(color: &pybevy_math::PyVec4) -> Self {
        let v: bevy::math::Vec4 = color.into();
        PyXyza::xyza(Xyza::new(v.x, v.y, v.z, v.w))
    }

    #[staticmethod]
    pub fn from_vec3(color: &pybevy_math::PyVec3) -> Self {
        let v: bevy::math::Vec3 = color.into();
        PyXyza::xyza(Xyza::xyz(v.x, v.y, v.z))
    }

    pub fn interpolate_stable(&self, other: &PyXyza, t: f32) -> PyResult<Self> {
        Ok(PyXyza::xyza(
            self.as_ref()?.interpolate_stable(other.as_ref()?, t),
        ))
    }
}
