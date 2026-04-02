pub mod font;
pub mod font_atlas;
pub mod font_atlas_sets;
pub mod font_feature_tag;
pub mod font_features;
pub mod font_smoothing;
pub mod font_weight;
pub mod justify;
pub mod line_break;
pub mod line_height;
pub mod plugin;
pub mod text2d;
pub mod text2d_shadow;
pub mod text_background_color;
pub mod text_bounds;
pub mod text_color;
pub mod text_decoration;
pub mod text_font;
pub mod text_layout;
pub mod text_span;

pub use font::PyFont;
pub use font_atlas::{PyFontAtlas, PyFontAtlasKey};
pub use font_atlas_sets::PyFontAtlasSet;
pub use font_feature_tag::PyFontFeatureTag;
pub use font_features::PyFontFeatures;
pub use font_smoothing::PyFontSmoothing;
pub use font_weight::PyFontWeight;
pub use justify::PyJustify;
pub use line_break::PyLineBreak;
pub use line_height::PyLineHeight;
pub use plugin::PyTextPlugin;
use pyo3::prelude::*;
pub use text_background_color::PyTextBackgroundColor;
pub use text_bounds::PyTextBounds;
pub use text_color::PyTextColor;
pub use text_decoration::{PyStrikethrough, PyStrikethroughColor, PyUnderline, PyUnderlineColor};
pub use text_font::PyTextFont;
pub use text_layout::PyTextLayout;
pub use text_span::PyTextSpan;
pub use text2d::PyText2d;
pub use text2d_shadow::PyText2dShadow;

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "text")?;
    m.add_class::<PyTextPlugin>()?;

    m.add_class::<PyText2d>()?;
    m.add_class::<PyText2dShadow>()?;
    m.add_class::<PyTextColor>()?;
    m.add_class::<PyTextBackgroundColor>()?;
    m.add_class::<PyTextFont>()?;
    m.add_class::<PyTextLayout>()?;
    m.add_class::<PyTextSpan>()?;
    m.add_class::<PyTextBounds>()?;

    m.add_class::<PyFont>()?;

    m.add_class::<PyFontAtlas>()?;
    m.add_class::<PyFontAtlasKey>()?;
    m.add_class::<PyFontAtlasSet>()?;

    m.add_class::<PyFontFeatures>()?;
    m.add_class::<PyFontSmoothing>()?;
    m.add_class::<PyFontWeight>()?;
    m.add_class::<PyJustify>()?;
    m.add_class::<PyLineBreak>()?;
    m.add_class::<PyLineHeight>()?;
    m.add_class::<PyFontFeatureTag>()?;

    m.add_class::<PyStrikethrough>()?;
    m.add_class::<PyStrikethroughColor>()?;
    m.add_class::<PyUnderline>()?;
    m.add_class::<PyUnderlineColor>()?;
    parent.add_submodule(&m)
}
