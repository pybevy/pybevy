use bevy::ui::{MaxTrackSizingFunction, MinTrackSizingFunction};
use pybevy_core::domain::{FINITE, check_for};
use pybevy_macros::pyenum;
use pyo3::prelude::*;

use crate::grid_track::PyGridTrack;

#[pyenum(MinTrackSizingFunction, empty_tuple)]
#[pyclass(
    name = "MinTrackSizingFunction",
    module = "pybevy.ui",
    eq,
    frozen,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PyMinTrackSizingFunction {
    #[py_bevy(tuple)]
    Px {
        value: f32,
    },
    #[py_bevy(tuple)]
    Percent {
        value: f32,
    },
    #[py_bevy(tuple)]
    Em {
        value: f32,
    },
    #[py_bevy(tuple)]
    Rem {
        value: f32,
    },
    MinContent(),
    MaxContent(),
    Auto(),
    #[py_bevy(tuple)]
    VMin {
        value: f32,
    },
    #[py_bevy(tuple)]
    VMax {
        value: f32,
    },
    #[py_bevy(tuple)]
    Vh {
        value: f32,
    },
    #[py_bevy(tuple)]
    Vw {
        value: f32,
    },
}

#[pyenum(MaxTrackSizingFunction, empty_tuple)]
#[pyclass(
    name = "MaxTrackSizingFunction",
    module = "pybevy.ui",
    eq,
    frozen,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PyMaxTrackSizingFunction {
    #[py_bevy(tuple)]
    Px {
        value: f32,
    },
    #[py_bevy(tuple)]
    Percent {
        value: f32,
    },
    #[py_bevy(tuple)]
    Em {
        value: f32,
    },
    #[py_bevy(tuple)]
    Rem {
        value: f32,
    },
    MinContent(),
    MaxContent(),
    #[py_bevy(tuple)]
    FitContentPx {
        value: f32,
    },
    #[py_bevy(tuple)]
    FitContentPercent {
        value: f32,
    },
    Auto(),
    #[py_bevy(tuple)]
    Fraction {
        value: f32,
    },
    #[py_bevy(tuple)]
    VMin {
        value: f32,
    },
    #[py_bevy(tuple)]
    VMax {
        value: f32,
    },
    #[py_bevy(tuple)]
    Vh {
        value: f32,
    },
    #[py_bevy(tuple)]
    Vw {
        value: f32,
    },
}

/// Payloads follow the finite rule of the other GridTrack factories.
pub(crate) fn checked_min(min: PyMinTrackSizingFunction) -> PyResult<MinTrackSizingFunction> {
    let min = MinTrackSizingFunction::from(min);
    match min {
        MinTrackSizingFunction::Px(value)
        | MinTrackSizingFunction::Percent(value)
        | MinTrackSizingFunction::Em(value)
        | MinTrackSizingFunction::Rem(value)
        | MinTrackSizingFunction::VMin(value)
        | MinTrackSizingFunction::VMax(value)
        | MinTrackSizingFunction::Vh(value)
        | MinTrackSizingFunction::Vw(value) => {
            check_for::<PyGridTrack, _>("minmax.min", value, FINITE)?;
        }
        MinTrackSizingFunction::MinContent
        | MinTrackSizingFunction::MaxContent
        | MinTrackSizingFunction::Auto => {}
    }
    Ok(min)
}

pub(crate) fn checked_max(max: PyMaxTrackSizingFunction) -> PyResult<MaxTrackSizingFunction> {
    let max = MaxTrackSizingFunction::from(max);
    match max {
        MaxTrackSizingFunction::Px(value)
        | MaxTrackSizingFunction::Percent(value)
        | MaxTrackSizingFunction::Em(value)
        | MaxTrackSizingFunction::Rem(value)
        | MaxTrackSizingFunction::FitContentPx(value)
        | MaxTrackSizingFunction::FitContentPercent(value)
        | MaxTrackSizingFunction::Fraction(value)
        | MaxTrackSizingFunction::VMin(value)
        | MaxTrackSizingFunction::VMax(value)
        | MaxTrackSizingFunction::Vh(value)
        | MaxTrackSizingFunction::Vw(value) => {
            check_for::<PyGridTrack, _>("minmax.max", value, FINITE)?;
        }
        MaxTrackSizingFunction::MinContent
        | MaxTrackSizingFunction::MaxContent
        | MaxTrackSizingFunction::Auto => {}
    }
    Ok(max)
}
