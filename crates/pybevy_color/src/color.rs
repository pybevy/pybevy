use bevy::color::{
    Alpha, Color, Hsla, Hue, LinearRgba, Luminance, Mix, Saturation, Srgba,
    color_difference::EuclideanDistance,
};
use pybevy_core::PyMaterializable;
use pyo3::prelude::*;

use super::{common::fmt_f32, hsla::PyHsla, linear_rgba::PyLinearRgba, srgba::PySrgba};

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
