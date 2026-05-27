pub mod atlas;
pub mod font;
pub mod template;
pub mod text_layer;

pub use atlas::FontAtlas;
pub use font::FontRef;
pub use template::{
    format_time_mmss, format_time_mmss_frame, format_time_mmss_millis, FormatConfig, Separator,
    TemplateVars, render as render_template,
};
pub use text_layer::{Alignment as TextAlignment, TextLayer};
