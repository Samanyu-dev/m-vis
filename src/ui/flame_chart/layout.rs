use crate::types::FrameId;
use crate::ui::flame_chart::snapshot::FlameSnapshot;

pub type NodeId = u64;

#[derive(Debug, Clone)]
pub struct FlameLayoutNode {
    pub id: NodeId,
    pub frame: FrameId,
    pub x: u16,
    pub width: u16,
    pub depth: u16,
    pub bytes: u64,
}

pub struct FlameLayout {
    pub nodes: Vec<FlameLayoutNode>,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Metric {
    LiveBytes,
    TotalBytes,
    PeakLiveBytes,
}

impl FlameLayout {
    pub fn build(
        snapshot: &FlameSnapshot,
        total_width: u16,
        metric: Metric,
        focused_node_frame: Option<FrameId>,
    ) -> Self {
        let mut nodes = Vec::new();
        let mut next_id = 0;

        let mut root_node = &snapshot.root;
        if let Some(frame) = focused_node_frame {
            // Find the node with this frame id to act as root
            if let Some(focused) = Self::find_node(&snapshot.root, frame) {
                root_node = focused;
            }
        }

        let root_bytes = match metric {
            Metric::LiveBytes => root_node.live_bytes,
            Metric::TotalBytes => root_node.total_bytes,
            Metric::PeakLiveBytes => root_node.peak_live_bytes,
        };

        if root_bytes > 0 {
            Self::layout_node(
                root_node,
                0,
                total_width,
                0,
                metric,
                &mut nodes,
                &mut next_id,
            );
        }

        Self { nodes }
    }

    #[allow(clippy::too_many_arguments)]
    fn layout_node(
        node: &crate::types::FlameNode,
        x: u16,
        width: u16,
        depth: u16,
        metric: Metric,
        nodes: &mut Vec<FlameLayoutNode>,
        next_id: &mut NodeId,
    ) {
        if width == 0 {
            return;
        }

        let id = *next_id;
        *next_id += 1;

        let bytes = match metric {
            Metric::LiveBytes => node.live_bytes,
            Metric::TotalBytes => node.total_bytes,
            Metric::PeakLiveBytes => node.peak_live_bytes,
        };

        nodes.push(FlameLayoutNode {
            id,
            frame: node.frame,
            x,
            width,
            depth,
            bytes,
        });

        // Sort children descending by bytes
        let mut children: Vec<_> = node.children.values().collect();
        children.sort_by(|a, b| {
            let b_bytes = match metric {
                Metric::LiveBytes => b.live_bytes,
                Metric::TotalBytes => b.total_bytes,
                Metric::PeakLiveBytes => b.peak_live_bytes,
            };
            let a_bytes = match metric {
                Metric::LiveBytes => a.live_bytes,
                Metric::TotalBytes => a.total_bytes,
                Metric::PeakLiveBytes => a.peak_live_bytes,
            };
            b_bytes.cmp(&a_bytes)
        });

        let mut current_x = x;
        let mut remaining_width = width;
        let mut remaining_bytes = bytes;

        for child in children {
            let child_bytes = match metric {
                Metric::LiveBytes => child.live_bytes,
                Metric::TotalBytes => child.total_bytes,
                Metric::PeakLiveBytes => child.peak_live_bytes,
            };

            if child_bytes == 0 || remaining_bytes == 0 || remaining_width == 0 {
                continue;
            }

            // Calculate exact width as f64 to avoid integer truncation issues early
            let fraction = child_bytes as f64 / remaining_bytes as f64;
            let child_width = (remaining_width as f64 * fraction).round() as u16;

            // Ensure we don't exceed remaining width due to rounding
            let child_width = child_width.min(remaining_width);

            if child_width > 0 {
                Self::layout_node(
                    child,
                    current_x,
                    child_width,
                    depth + 1,
                    metric,
                    nodes,
                    next_id,
                );
            }

            current_x += child_width;
            remaining_width -= child_width;
            remaining_bytes -= child_bytes;
        }
    }

    fn find_node(
        root: &crate::types::FlameNode,
        frame: FrameId,
    ) -> Option<&crate::types::FlameNode> {
        if root.frame == frame {
            return Some(root);
        }
        for child in root.children.values() {
            if let Some(found) = Self::find_node(child, frame) {
                return Some(found);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::FlameNode;
    use crate::ui::flame_chart::snapshot::{FlameSnapshot, SymbolSnapshot};
    use std::collections::HashMap;
    use std::sync::Arc;

    #[test]
    fn test_rounding_absorbs_remainders() {
        // Construct a root with 3 equal children (each 26.666 units)
        let child1 = FlameNode {
            frame: 1,
            live_bytes: 10,
            total_bytes: 10,
            peak_live_bytes: 10,
            live_count: 1,
            total_count: 1,
            children: HashMap::new(),
        };
        let child2 = FlameNode {
            frame: 2,
            live_bytes: 10,
            total_bytes: 10,
            peak_live_bytes: 10,
            live_count: 1,
            total_count: 1,
            children: HashMap::new(),
        };
        let child3 = FlameNode {
            frame: 3,
            live_bytes: 10,
            total_bytes: 10,
            peak_live_bytes: 10,
            live_count: 1,
            total_count: 1,
            children: HashMap::new(),
        };

        let mut children = HashMap::new();
        children.insert(1, child1);
        children.insert(2, child2);
        children.insert(3, child3);

        let root = FlameNode {
            frame: 0,
            live_bytes: 30,
            total_bytes: 30,
            peak_live_bytes: 30,
            live_count: 3,
            total_count: 3,
            children,
        };

        let snapshot = FlameSnapshot {
            root,
            symbols: Arc::new(SymbolSnapshot {
                frames: Arc::new([]),
            }),
            generated_sequence: 1,
        };

        let layout = FlameLayout::build(&snapshot, 80, Metric::LiveBytes, None);

        // Root + 3 children = 4 nodes
        assert_eq!(layout.nodes.len(), 4);

        // Find the 3 child nodes (depth == 1)
        let child_nodes: Vec<&FlameLayoutNode> =
            layout.nodes.iter().filter(|n| n.depth == 1).collect();
        assert_eq!(child_nodes.len(), 3);

        let sum_width: u16 = child_nodes.iter().map(|n| n.width).sum();

        // The children MUST exactly sum up to 80 (since total live_bytes exactly match the root's live_bytes).
        assert_eq!(sum_width, 80);

        // Individual widths should be 27, 27, 26 (or some permutation that sums to 80)
        let mut widths: Vec<u16> = child_nodes.iter().map(|n| n.width).collect();
        widths.sort();
        assert_eq!(widths, vec![26, 27, 27]);
    }
    #[test]
    fn test_golden_deterministic_tree() {
        let child_a_1 = FlameNode {
            frame: 3,
            live_bytes: 10,
            total_bytes: 10,
            peak_live_bytes: 10,
            live_count: 1,
            total_count: 1,
            children: HashMap::new(),
        };
        let child_a_2 = FlameNode {
            frame: 4,
            live_bytes: 10,
            total_bytes: 10,
            peak_live_bytes: 10,
            live_count: 1,
            total_count: 1,
            children: HashMap::new(),
        };

        let mut child_a_children = HashMap::new();
        child_a_children.insert(3, child_a_1);
        child_a_children.insert(4, child_a_2);

        let child_a = FlameNode {
            frame: 1,
            live_bytes: 20,
            total_bytes: 20,
            peak_live_bytes: 20,
            live_count: 2,
            total_count: 2,
            children: child_a_children,
        };
        let child_b = FlameNode {
            frame: 2,
            live_bytes: 30,
            total_bytes: 30,
            peak_live_bytes: 30,
            live_count: 1,
            total_count: 1,
            children: HashMap::new(),
        };

        let mut root_children = HashMap::new();
        root_children.insert(1, child_a);
        root_children.insert(2, child_b);

        let root = FlameNode {
            frame: 0,
            live_bytes: 50,
            total_bytes: 50,
            peak_live_bytes: 50,
            live_count: 3,
            total_count: 3,
            children: root_children,
        };

        let snapshot = FlameSnapshot {
            root,
            symbols: Arc::new(SymbolSnapshot {
                frames: Arc::new([]),
            }),
            generated_sequence: 1,
        };

        // Layout over 100 width
        // child_a = 40 width (20/50 * 100)
        // child_b = 60 width (30/50 * 100)
        // child_a_1 = 20 width (10/20 * 40)
        // child_a_2 = 20 width (10/20 * 40)
        let layout = FlameLayout::build(&snapshot, 100, Metric::LiveBytes, None);

        assert_eq!(layout.nodes.len(), 5);

        // Verify root
        assert_eq!(layout.nodes[0].frame, 0);
        assert_eq!(layout.nodes[0].depth, 0);
        assert_eq!(layout.nodes[0].x, 0);
        assert_eq!(layout.nodes[0].width, 100);

        // Sort nodes by depth and then x position to stably check order
        let mut nodes = layout.nodes.clone();
        nodes.sort_by(|a, b| a.depth.cmp(&b.depth).then(a.x.cmp(&b.x)));

        // Node 1 (child_b, since 30 > 20 it sorts first)
        assert_eq!(nodes[1].frame, 2);
        assert_eq!(nodes[1].depth, 1);
        assert_eq!(nodes[1].x, 0);
        assert_eq!(nodes[1].width, 60);

        // Node 2 (child_a)
        assert_eq!(nodes[2].frame, 1);
        assert_eq!(nodes[2].depth, 1);
        assert_eq!(nodes[2].x, 60);
        assert_eq!(nodes[2].width, 40);

        // child_a_1 and child_a_2 both have 10 bytes, so their sort order is unstable.
        // But we know their total width is 40 and they start at x=60.
        // Node 3 (child_a_1 or 2)
        let n3 = &nodes[3];
        let n4 = &nodes[4];
        assert!(n3.frame == 3 || n3.frame == 4);
        assert_eq!(n3.depth, 2);
        assert_eq!(n3.x, 60);
        assert_eq!(n3.width, 20);

        // Node 4 (the other one)
        assert!(n4.frame == 3 || n4.frame == 4);
        assert_eq!(n4.depth, 2);
        assert_eq!(n4.x, 80);
        assert_eq!(n4.width, 20);

        // Navigation checks
        let mut nav = crate::ui::flame_chart::navigation::FlameNavigator::new();
        assert_eq!(nav.selected_node, Some(0)); // Starts at root

        // Down -> First child (child_b)
        nav.handle_key(crossterm::event::KeyCode::Down, &layout);
        let child_b_id = nodes[1].id;
        assert_eq!(nav.selected_node, Some(child_b_id));

        // Right -> Next sibling (child_a)
        nav.handle_key(crossterm::event::KeyCode::Right, &layout);
        let child_a_id = nodes[2].id;
        assert_eq!(nav.selected_node, Some(child_a_id));

        // Left -> Previous sibling (child_b)
        nav.handle_key(crossterm::event::KeyCode::Left, &layout);
        assert_eq!(nav.selected_node, Some(child_b_id));

        // Right -> child_a again
        nav.handle_key(crossterm::event::KeyCode::Right, &layout);

        // Down -> First child of child_a (child_a_1 or child_a_2 depending on layout stable sort)
        nav.handle_key(crossterm::event::KeyCode::Down, &layout);
        assert_eq!(nav.selected_node, Some(nodes[3].id));

        // Up -> Parent (back to child_a)
        nav.handle_key(crossterm::event::KeyCode::Up, &layout);
        assert_eq!(nav.selected_node, Some(child_a_id));
    }
}
