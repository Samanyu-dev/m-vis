use serde::Serialize;

/// A single memory region in a process's address space.
///
/// Corresponds to one entry from `VirtualQueryEx` on Windows
/// or one line from `/proc/<pid>/maps` on Linux.
#[derive(Clone, Debug, Serialize)]
pub struct Region {
    pub base: usize,
    pub size: usize,
    pub state: RegionState,
    pub kind: RegionKind,
    pub protect: RegionProtect,
    pub name: String,
}

#[derive(Serialize)]
pub struct RegionEntry {
    pub base: usize,
    pub size: usize,
    pub state: RegionState,
    pub kind: RegionKind,
    pub protect: RegionProtect,
    pub name: String,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum RegionState {
    Committed,
    Reserved,
    Free,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum RegionKind {
    Image,
    Mapped,
    Private,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum RegionProtect {
    NoAccess,
    Readonly,
    ReadWrite,
    Execute,
    Guard,
    Other,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct HeapBlock {
    pub address: usize,
    pub size: usize,
    pub is_free: bool,
    pub vm_protect: RegionProtect,
}

#[derive(Clone, Debug, Serialize)]
pub struct HeapStats {
    pub address: usize,
    pub size: usize,
    pub rss: usize, // resident set size — actually in RAM
}

#[derive(Debug, Clone, PartialEq)]
pub enum ModuleStatus {
    Ok,
    Tampered,
    Injected,
    Unreadable,
    Modified,
}

#[derive(Debug, Clone)]
pub struct ModuleInfo {
    pub base: usize,
    pub size: usize,
    pub name: String,
    pub path: String,
    pub status: ModuleStatus,
}

pub type FrameId = u32;

#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize)]
pub struct FrameKey {
    pub module_base: usize,
    pub instruction_pointer: usize,
}

#[derive(Debug, Clone, Serialize)]
pub enum AllocationEvent {
    Alloc {
        address: usize,
        size: usize,
        thread_id: u32,
        sequence: u64,
        stack: Vec<FrameKey>,
    },
    Free {
        address: usize,
        thread_id: u32,
        sequence: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum TracingMode {
    EveryAllocation,
}

#[derive(Debug, Clone, Serialize)]
pub struct FlameNode {
    pub frame: FrameId,
    pub live_bytes: u64,
    pub total_bytes: u64,
    pub peak_live_bytes: u64,
    pub live_count: u64,
    pub total_count: u64,
    pub children: std::collections::HashMap<FrameId, FlameNode>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct SymbolInfo {
    pub name: String,
    pub module: String,
    pub address: usize,
}
