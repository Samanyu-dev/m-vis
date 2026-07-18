use crate::types::{FlameNode, FrameId, SymbolInfo};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct SymbolSnapshot {
    pub frames: Arc<[SymbolInfo]>,
}

impl SymbolSnapshot {
    pub fn get(&self, frame: FrameId) -> Option<&SymbolInfo> {
        self.frames.get(frame as usize)
    }
}

#[derive(Debug, Clone)]
pub struct FlameSnapshot {
    pub root: FlameNode,
    pub symbols: Arc<SymbolSnapshot>,
    pub generated_sequence: u64,
}
