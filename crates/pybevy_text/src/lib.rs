pub mod editable_text;
pub mod font;
pub mod font_atlas;
pub mod font_atlas_sets;
pub mod font_feature_tag;
pub mod font_features;
pub mod font_hinting;
pub mod font_size;
pub mod font_smoothing;
pub mod font_source;
pub mod font_style;
pub mod font_weight;
pub mod font_width;
pub mod justify;
pub mod letter_spacing;
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

use pyo3::prelude::*;

pub mod prelude {
    pub use crate::{
        editable_text::PyEditableText, font::PyFont, font_hinting::PyFontHinting,
        font_size::PyFontSize, font_source::PyFontSource, font_style::PyFontStyle,
        font_weight::PyFontWeight, font_width::PyFontWidth, justify::PyJustify,
        letter_spacing::PyLetterSpacing, line_break::PyLineBreak, plugin::PyTextPlugin,
        text_background_color::PyTextBackgroundColor, text_color::PyTextColor,
        text_font::PyTextFont, text_layout::PyTextLayout, text_span::PyTextSpan, text2d::PyText2d,
        text2d_shadow::PyText2dShadow,
    };
}

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "text")?;
    m.add_class::<plugin::PyTextPlugin>()?;

    m.add_class::<text2d::PyText2d>()?;
    m.add_class::<text2d_shadow::PyText2dShadow>()?;
    m.add_class::<text_color::PyTextColor>()?;
    m.add_class::<text_background_color::PyTextBackgroundColor>()?;
    m.add_class::<text_font::PyTextFont>()?;
    m.add_class::<text_layout::PyTextLayout>()?;
    m.add_class::<text_span::PyTextSpan>()?;
    m.add_class::<text_bounds::PyTextBounds>()?;
    m.add_class::<editable_text::PyEditableText>()?;
    m.add_class::<letter_spacing::PyLetterSpacing>()?;

    m.add_class::<font::PyFont>()?;

    m.add_class::<font_atlas::PyFontAtlas>()?;
    m.add_class::<font_atlas::PyFontAtlasKey>()?;
    m.add_class::<font_atlas_sets::PyFontAtlasSet>()?;

    m.add_class::<font_features::PyFontFeatures>()?;
    m.add_class::<font_size::PyFontSize>()?;
    m.add_class::<font_source::PyFontSource>()?;
    m.add_class::<font_smoothing::PyFontSmoothing>()?;
    m.add_class::<font_style::PyFontStyle>()?;
    m.add_class::<font_weight::PyFontWeight>()?;
    m.add_class::<font_width::PyFontWidth>()?;
    m.add_class::<font_hinting::PyFontHinting>()?;
    m.add_class::<justify::PyJustify>()?;
    m.add_class::<line_break::PyLineBreak>()?;
    m.add_class::<line_height::PyLineHeight>()?;
    m.add_class::<font_feature_tag::PyFontFeatureTag>()?;

    m.add_class::<text_decoration::PyStrikethrough>()?;
    m.add_class::<text_decoration::PyStrikethroughColor>()?;
    m.add_class::<text_decoration::PyUnderline>()?;
    m.add_class::<text_decoration::PyUnderlineColor>()?;
    parent.add_submodule(&m)
}
