use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::Widget;

use crate::ui::flame_chart::colors::{ColorMode, get_color};
use crate::ui::flame_chart::layout::FlameLayout;
use crate::ui::flame_chart::navigation::FlameNavigator;
use crate::ui::flame_chart::snapshot::SymbolSnapshot;

pub struct FlameChartWidget<'a> {
    layout: &'a FlameLayout,
    symbols: &'a SymbolSnapshot,
    navigator: Option<&'a FlameNavigator>,
    color_mode: ColorMode,
}

impl<'a> FlameChartWidget<'a> {
    pub fn new(
        layout: &'a FlameLayout,
        symbols: &'a SymbolSnapshot,
        navigator: Option<&'a FlameNavigator>,
        color_mode: ColorMode,
    ) -> Self {
        Self {
            layout,
            symbols,
            navigator,
            color_mode,
        }
    }
}

impl<'a> Widget for FlameChartWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        for node in &self.layout.nodes {
            let y = area.y + node.depth;

            // Clip to terminal height
            if y >= area.y + area.height {
                continue;
            }

            let x = area.x + node.x;
            let width = node.width;

            if width == 0 {
                continue;
            }

            // Draw rectangle background
            let rect = Rect {
                x,
                y,
                width,
                height: 1,
            };

            // Colors based on ColorMode
            let color = get_color(node, self.color_mode, self.symbols);

            let is_selected = self
                .navigator
                .is_some_and(|nav| nav.selected_node == Some(node.id));

            let style = if is_selected {
                Style::default()
                    .bg(color)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
                    .add_modifier(Modifier::REVERSED)
            } else {
                Style::default().bg(color).fg(Color::White)
            };

            buf.set_style(rect, style);

            // Text Elision
            if width > 2
                && let Some(info) = self.symbols.get(node.frame)
            {
                let mut name = info.name.clone();
                if name.is_empty() {
                    name = "unknown".to_string();
                }

                if width <= 5 {
                    // Narrow width -> Truncated
                    buf.set_string(
                        x,
                        y,
                        name.chars().take(width as usize).collect::<String>(),
                        style,
                    );
                } else {
                    // Wide width -> Full if it fits, else ellipsis
                    if name.len() > width as usize {
                        let truncated: String = name.chars().take((width - 3) as usize).collect();
                        buf.set_string(x, y, format!("{}...", truncated), style);
                    } else {
                        buf.set_string(x, y, name, style);
                    }
                }
            }
        }
    }
}
