/// A math value type and the coordinate names of its array form.
pub(crate) struct MathShape {
    pub(crate) name: &'static str,
    pub(crate) coordinates: &'static [&'static str],
    pub(crate) integral: bool,
}

impl MathShape {
    /// The array form written in messages, such as `[x, y, z]`.
    pub(crate) fn label(&self) -> String {
        format!("[{}]", self.coordinates.join(", "))
    }
}

const XY: &[&str] = &["x", "y"];
const XYZ: &[&str] = &["x", "y", "z"];
const XYZW: &[&str] = &["x", "y", "z", "w"];

const SHAPES: &[MathShape] = &[
    MathShape {
        name: "Vec2",
        coordinates: XY,
        integral: false,
    },
    MathShape {
        name: "Vec3",
        coordinates: XYZ,
        integral: false,
    },
    MathShape {
        name: "Vec4",
        coordinates: XYZW,
        integral: false,
    },
    MathShape {
        name: "Quat",
        coordinates: XYZW,
        integral: false,
    },
    MathShape {
        name: "UVec2",
        coordinates: XY,
        integral: true,
    },
    MathShape {
        name: "IVec2",
        coordinates: XY,
        integral: true,
    },
    MathShape {
        name: "UVec3",
        coordinates: XYZ,
        integral: true,
    },
];

/// Float vectors rendered as arrays by get_component and reflection readback.
pub(crate) const FLOAT_VECTORS: &[&str] = &["Vec2", "Vec3", "Vec4", "Quat"];

/// Types whose `{x, y, ...}` readback object set_component accepts as an array.
pub(crate) const READBACK_ARRAYS: &[&str] = &["Vec2", "UVec2", "IVec2", "UVec3", "Vec4", "Quat"];

/// Types built from a JSON array by constructing the pybevy.math class.
pub(crate) const CONSTRUCTIBLE: &[&str] =
    &["Vec2", "UVec2", "IVec2", "Vec3", "UVec3", "Vec4", "Quat"];

/// Looks `name` up in the shape table, limited to the caller's `subset`.
pub(crate) fn math_shape(subset: &[&str], name: &str) -> Option<&'static MathShape> {
    if !subset.contains(&name) {
        return None;
    }
    SHAPES.iter().find(|shape| shape.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_subset_name_has_a_shape() {
        for name in FLOAT_VECTORS
            .iter()
            .chain(READBACK_ARRAYS)
            .chain(CONSTRUCTIBLE)
        {
            assert!(math_shape(&[name], name).is_some(), "{name}");
        }
    }

    #[test]
    fn subsets_limit_lookups() {
        assert!(math_shape(READBACK_ARRAYS, "Vec3").is_none());
        assert!(math_shape(FLOAT_VECTORS, "UVec2").is_none());
        assert_eq!(
            math_shape(CONSTRUCTIBLE, "UVec3").unwrap().label(),
            "[x, y, z]"
        );
        assert!(math_shape(CONSTRUCTIBLE, "IVec2").unwrap().integral);
        assert!(math_shape(CONSTRUCTIBLE, "IVec3").is_none());
    }
}
