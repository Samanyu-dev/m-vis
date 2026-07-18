use crate::ui::flame_chart::layout::{FlameLayout, NodeId};

pub struct FlameNavigator {
    pub selected_node: Option<NodeId>,
    pub focused_node: Option<NodeId>, // For Milestone 4
}

impl Default for FlameNavigator {
    fn default() -> Self {
        Self::new()
    }
}

impl FlameNavigator {
    pub fn new() -> Self {
        Self {
            selected_node: Some(0), // 0 is root
            focused_node: None,
        }
    }

    pub fn handle_key(&mut self, key: crossterm::event::KeyCode, layout: &FlameLayout) {
        if let Some(selected) = self.selected_node
            && let Some(idx) = layout.nodes.iter().position(|n| n.id == selected)
        {
            let node = &layout.nodes[idx];
            match key {
                crossterm::event::KeyCode::Left => {
                    // Previous sibling
                    if node.depth > 0 {
                        for sibling in layout.nodes[..idx].iter().rev() {
                            if sibling.depth < node.depth {
                                break; // Hit parent
                            }
                            if sibling.depth == node.depth {
                                self.selected_node = Some(sibling.id);
                                break;
                            }
                        }
                    }
                }
                crossterm::event::KeyCode::Right => {
                    // Next sibling
                    for sibling in layout.nodes[idx + 1..].iter() {
                        if sibling.depth < node.depth {
                            break; // Hit end of parent scope
                        }
                        if sibling.depth == node.depth {
                            self.selected_node = Some(sibling.id);
                            break;
                        }
                    }
                }
                crossterm::event::KeyCode::Up => {
                    // Parent
                    if node.depth > 0 {
                        for parent in layout.nodes[..idx].iter().rev() {
                            if parent.depth == node.depth - 1 {
                                self.selected_node = Some(parent.id);
                                break;
                            }
                        }
                    }
                }
                crossterm::event::KeyCode::Down => {
                    // First child
                    for child in layout.nodes[idx + 1..].iter() {
                        if child.depth <= node.depth {
                            break; // Reached sibling or higher up node
                        }
                        if child.depth == node.depth + 1 {
                            self.selected_node = Some(child.id);
                            break;
                        }
                    }
                }
                crossterm::event::KeyCode::Enter => {
                    // Focus (Milestone 4)
                    self.focused_node = Some(node.id);
                }
                crossterm::event::KeyCode::Esc => {
                    // Pop focus (Milestone 4)
                    self.focused_node = None;
                }
                _ => {}
            }
        }
    }
}
