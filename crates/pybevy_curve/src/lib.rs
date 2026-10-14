pub mod cubic_splines;
pub mod easing;
pub mod interval;

use pyo3::prelude::*;

pub mod prelude {
    #[doc(hidden)]
    pub use crate::{
        cubic_splines::{
            PyCubicBSpline2d, PyCubicBSpline3d, PyCubicBezier2d, PyCubicBezier3d,
            PyCubicCardinalSpline2d, PyCubicCardinalSpline3d, PyCubicCurve2d, PyCubicCurve3d,
            PyCubicHermite2d, PyCubicHermite3d, PyCubicNurbs2d, PyCubicNurbs3d, PyRationalCurve2d,
            PyRationalCurve3d,
        },
        interval::PyInterval,
    };
}

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "curve")?;
    m.add_class::<interval::PyInterval>()?;
    m.add_class::<easing::PyJumpAt>()?;
    m.add_class::<easing::PyEaseFunction>()?;
    m.add_class::<cubic_splines::cubic_curve::PyCubicCurve2d>()?;
    m.add_class::<cubic_splines::cubic_curve3d::PyCubicCurve3d>()?;
    m.add_class::<cubic_splines::cubic_bezier::PyCubicBezier2d>()?;
    m.add_class::<cubic_splines::cubic_bezier3d::PyCubicBezier3d>()?;
    m.add_class::<cubic_splines::cubic_cardinal_spline::PyCubicCardinalSpline2d>()?;
    m.add_class::<cubic_splines::cubic_cardinal_spline3d::PyCubicCardinalSpline3d>()?;
    m.add_class::<cubic_splines::cubic_hermite::PyCubicHermite2d>()?;
    m.add_class::<cubic_splines::cubic_hermite3d::PyCubicHermite3d>()?;
    m.add_class::<cubic_splines::cubic_bspline2d::PyCubicBSpline2d>()?;
    m.add_class::<cubic_splines::cubic_bspline3d::PyCubicBSpline3d>()?;
    m.add_class::<cubic_splines::cubic_nurbs2d::PyCubicNurbs2d>()?;
    m.add_class::<cubic_splines::cubic_nurbs3d::PyCubicNurbs3d>()?;
    m.add_class::<cubic_splines::linear_spline2d::PyLinearSpline2d>()?;
    m.add_class::<cubic_splines::linear_spline3d::PyLinearSpline3d>()?;
    m.add_class::<cubic_splines::rational_curve2d::PyRationalCurve2d>()?;
    m.add_class::<cubic_splines::rational_curve3d::PyRationalCurve3d>()?;
    m.add_class::<cubic_splines::PyCubicSegment2d>()?;
    m.add_class::<cubic_splines::PyCubicSegment3d>()?;
    m.add_class::<cubic_splines::PyRationalSegment2d>()?;
    m.add_class::<cubic_splines::PyRationalSegment3d>()?;
    parent.add_submodule(&m)
}
