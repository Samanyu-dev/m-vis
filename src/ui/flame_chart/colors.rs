use crate::ui::flame_chart::layout::FlameLayoutNode;
use crate::ui::flame_chart::snapshot::SymbolSnapshot;
use ratatui::style::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    Depth,
    Module,
    Bytes,
    Mono,
}

pub fn get_color(node: &FlameLayoutNode, mode: ColorMode, symbols: &SymbolSnapshot) -> Color {
    match mode {
        ColorMode::Depth => match node.depth % 3 {
            0 => Color::Rgb(255, 100, 100),
            1 => Color::Rgb(100, 255, 100),
            _ => Color::Rgb(100, 100, 255),
        },
        ColorMode::Module => {
            if let Some(info) = symbols.get(node.frame) {
                // A simple hash of the module name
                let hash = info
                    .module
                    .bytes()
                    .fold(0u16, |acc, b| acc.wrapping_add(b as u16));
                Color::Indexed((hash % 215 + 16) as u8) // 16-231 is the 256 color cube
            } else {
                Color::DarkGray
            }
        }
        ColorMode::Bytes => {
            // A simple heat map based on bytes. In a real implementation we'd scale relative to root.
            Color::Rgb(255, (255_u64.saturating_sub(node.bytes % 255)) as u8, 0)
        }
        ColorMode::Mono => Color::DarkGray,
    }
}
