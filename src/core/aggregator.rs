use crate::types::{AllocationEvent, FlameNode, FrameId, SymbolInfo};
use std::collections::HashMap;
use std::sync::mpsc::Receiver;

pub struct LiveAllocation {
    pub address: usize,
    pub size: usize,
    pub stack: std::sync::Arc<[FrameId]>,
    pub thread_id: u32,
    pub sequence: u64,
}

pub enum SnapshotPolicy {
    Every(std::time::Duration),
}

pub struct Aggregator {
    pub root: FlameNode,
    pub symbol_table: HashMap<FrameId, SymbolInfo>,
    pub interner: HashMap<crate::types::FrameKey, FrameId>,
    pub next_frame_id: FrameId,
    pub live_allocations: HashMap<usize, LiveAllocation>,
}

const KNOWN_ALLOCATOR_MODULES: &[&str] = &["ntdll", "ucrtbase", "msvcrt", "libc"];
const KNOWN_ALLOCATOR_SYMBOLS: &[&str] = &[
    "malloc",
    "free",
    "realloc",
    "calloc",
    "RtlAllocateHeap",
    "RtlFreeHeap",
    "HeapAlloc",
    "HeapFree",
];

impl Default for Aggregator {
    fn default() -> Self {
        Self::new()
    }
}

impl Aggregator {
    pub fn new() -> Self {
        Self {
            root: FlameNode {
                frame: 0,
                live_bytes: 0,
                total_bytes: 0,
                peak_live_bytes: 0,
                live_count: 0,
                total_count: 0,
                children: HashMap::new(),
            },
            symbol_table: HashMap::new(),
            interner: HashMap::new(),
            next_frame_id: 1, // 0 is reserved for root
            live_allocations: HashMap::new(),
        }
    }

    pub fn snapshot(&self) -> crate::ui::flame_chart::snapshot::FlameSnapshot {
        let mut frames = vec![SymbolInfo::default(); self.next_frame_id as usize];
        for (&id, info) in &self.symbol_table {
            frames[id as usize] = info.clone();
        }

        crate::ui::flame_chart::snapshot::FlameSnapshot {
            root: self.root.clone(),
            symbols: std::sync::Arc::new(crate::ui::flame_chart::snapshot::SymbolSnapshot {
                frames: frames.into(),
            }),
            generated_sequence: 0, // Placeholder
        }
    }

    fn should_strip(&self, id: FrameId) -> bool {
        if let Some(info) = self.symbol_table.get(&id) {
            if KNOWN_ALLOCATOR_MODULES
                .iter()
                .any(|&m| info.module.contains(m))
            {
                return true;
            }
            if KNOWN_ALLOCATOR_SYMBOLS
                .iter()
                .any(|&s| info.name.contains(s))
            {
                return true;
            }
        }
        false
    }

    pub fn intern_frame(&mut self, key: crate::types::FrameKey) -> FrameId {
        if let Some(&id) = self.interner.get(&key) {
            return id;
        }

        let id = self.next_frame_id;
        self.next_frame_id += 1;

        // TODO: Resolve symbol asynchronously or lazily.
        // For now, insert a placeholder symbol info.
        self.symbol_table.insert(
            id,
            SymbolInfo {
                name: format!("0x{:x}", key.instruction_pointer),
                module: "unknown".to_string(),
                address: key.instruction_pointer,
            },
        );

        self.interner.insert(key, id);
        id
    }

    pub fn run(
        &mut self,
        rx: Receiver<AllocationEvent>,
        publisher: Option<
            std::sync::Arc<
                std::sync::RwLock<
                    Option<std::sync::Arc<crate::ui::flame_chart::snapshot::FlameSnapshot>>,
                >,
            >,
        >,
        policy: SnapshotPolicy,
    ) {
        use std::time::Instant;
        let mut last_publish = Instant::now();
        let SnapshotPolicy::Every(publish_interval) = policy;

        while let Ok(event) = rx.recv() {
            self.process_event(event);

            if let Some(pub_lock) = &publisher
                && last_publish.elapsed() >= publish_interval
            {
                let snapshot = std::sync::Arc::new(self.snapshot());
                if let Ok(mut guard) = pub_lock.write() {
                    *guard = Some(snapshot);
                }
                last_publish = Instant::now();
            }
        }

        // Final publish when channel closes
        if let Some(pub_lock) = &publisher {
            let snapshot = std::sync::Arc::new(self.snapshot());
            if let Ok(mut guard) = pub_lock.write() {
                *guard = Some(snapshot);
            }
        }
    }

    pub fn process_event(&mut self, event: AllocationEvent) {
        match event {
            AllocationEvent::Alloc {
                address,
                size,
                thread_id,
                sequence,
                stack,
            } => {
                let mut frame_ids = Vec::with_capacity(stack.len());
                for key in stack {
                    frame_ids.push(self.intern_frame(key));
                }

                // Strip allocator frames
                let mut start_idx = 0;
                for (i, &id) in frame_ids.iter().enumerate() {
                    if !self.should_strip(id) {
                        start_idx = i;
                        break;
                    }
                }
                let stripped_stack: std::sync::Arc<[FrameId]> = frame_ids[start_idx..].into();

                // Walk trie
                let mut current = &mut self.root;
                current.live_bytes += size as u64;
                current.total_bytes += size as u64;
                current.live_count += 1;
                current.total_count += 1;
                if current.live_bytes > current.peak_live_bytes {
                    current.peak_live_bytes = current.live_bytes;
                }

                for &id in stripped_stack.iter().rev() {
                    current = current.children.entry(id).or_insert_with(|| FlameNode {
                        frame: id,
                        live_bytes: 0,
                        total_bytes: 0,
                        peak_live_bytes: 0,
                        live_count: 0,
                        total_count: 0,
                        children: HashMap::new(),
                    });

                    current.live_bytes += size as u64;
                    current.total_bytes += size as u64;
                    current.live_count += 1;
                    current.total_count += 1;
                    if current.live_bytes > current.peak_live_bytes {
                        current.peak_live_bytes = current.live_bytes;
                    }
                }

                self.live_allocations.insert(
                    address,
                    LiveAllocation {
                        address,
                        size,
                        stack: stripped_stack,
                        thread_id,
                        sequence,
                    },
                );
            }
            AllocationEvent::Free { address, .. } => {
                if let Some(alloc) = self.live_allocations.remove(&address) {
                    let mut current = &mut self.root;
                    current.live_bytes -= alloc.size as u64;
                    current.live_count -= 1;

                    for &id in alloc.stack.iter().rev() {
                        if let Some(child) = current.children.get_mut(&id) {
                            child.live_bytes -= alloc.size as u64;
                            child.live_count -= 1;
                            current = child;
                        } else {
                            break;
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::thread;

    #[test]
    fn test_event_pipeline() {
        let (tx, rx) = mpsc::channel();

        let aggregator_handle = thread::spawn(move || {
            let mut aggregator = Aggregator::new();
            aggregator.run(
                rx,
                None,
                SnapshotPolicy::Every(std::time::Duration::from_millis(500)),
            );
            aggregator
        });

        // Send dummy event
        tx.send(AllocationEvent::Alloc {
            address: 0x1000,
            size: 64,
            thread_id: 1,
            sequence: 1,
            stack: Vec::new(),
        })
        .unwrap();

        drop(tx);

        let _final_state = aggregator_handle.join().unwrap();
    }

    #[test]
    fn test_interner_deduplication() {
        let mut aggregator = Aggregator::new();
        let key1 = crate::types::FrameKey {
            module_base: 0x1000,
            instruction_pointer: 0x1050,
        };
        let key2 = crate::types::FrameKey {
            module_base: 0x1000,
            instruction_pointer: 0x1050,
        };
        let key3 = crate::types::FrameKey {
            module_base: 0x1000,
            instruction_pointer: 0x2000,
        };

        let id1 = aggregator.intern_frame(key1);
        let id2 = aggregator.intern_frame(key2);
        let id3 = aggregator.intern_frame(key3);

        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
        assert_eq!(aggregator.interner.len(), 2);
    }

    #[test]
    fn test_alloc_free_lifecycle() {
        let mut aggregator = Aggregator::new();

        let frame1 = crate::types::FrameKey {
            module_base: 0,
            instruction_pointer: 100,
        };
        let frame2 = crate::types::FrameKey {
            module_base: 0,
            instruction_pointer: 200,
        };

        aggregator.process_event(AllocationEvent::Alloc {
            address: 0x1000,
            size: 64,
            thread_id: 1,
            sequence: 1,
            stack: vec![frame1.clone(), frame2.clone()],
        });

        assert_eq!(aggregator.root.live_bytes, 64);
        assert_eq!(aggregator.root.total_bytes, 64);
        assert_eq!(aggregator.root.peak_live_bytes, 64);
        assert_eq!(aggregator.live_allocations.len(), 1);

        aggregator.process_event(AllocationEvent::Alloc {
            address: 0x2000,
            size: 128,
            thread_id: 1,
            sequence: 2,
            stack: vec![frame1.clone()],
        });

        assert_eq!(aggregator.root.live_bytes, 192);
        assert_eq!(aggregator.root.total_bytes, 192);
        assert_eq!(aggregator.root.peak_live_bytes, 192);
        assert_eq!(aggregator.live_allocations.len(), 2);

        aggregator.process_event(AllocationEvent::Free {
            address: 0x1000,
            thread_id: 1,
            sequence: 3,
        });

        assert_eq!(aggregator.root.live_bytes, 128);
        assert_eq!(aggregator.root.total_bytes, 192); // Total does not decrease
        assert_eq!(aggregator.root.peak_live_bytes, 192); // Peak does not decrease
        assert_eq!(aggregator.live_allocations.len(), 1);
    }

    #[test]
    fn test_allocator_frame_stripping() {
        let mut aggregator = Aggregator::new();
        let key_ntdll = crate::types::FrameKey {
            module_base: 0x1,
            instruction_pointer: 0x100,
        };
        let key_malloc = crate::types::FrameKey {
            module_base: 0x2,
            instruction_pointer: 0x200,
        };
        let key_main = crate::types::FrameKey {
            module_base: 0x3,
            instruction_pointer: 0x300,
        };

        let id_ntdll = aggregator.intern_frame(key_ntdll.clone());
        let id_malloc = aggregator.intern_frame(key_malloc.clone());
        let id_main = aggregator.intern_frame(key_main.clone());

        // Override symbols manually for the test
        aggregator.symbol_table.get_mut(&id_ntdll).unwrap().module = "ntdll".to_string();
        aggregator.symbol_table.get_mut(&id_malloc).unwrap().name = "malloc".to_string();
        aggregator.symbol_table.get_mut(&id_main).unwrap().name = "main".to_string();

        assert!(aggregator.should_strip(id_ntdll));
        assert!(aggregator.should_strip(id_malloc));
        assert!(!aggregator.should_strip(id_main));

        aggregator.process_event(AllocationEvent::Alloc {
            address: 0x5000,
            size: 64,
            thread_id: 1,
            sequence: 1,
            stack: vec![key_ntdll, key_malloc, key_main],
        });

        // The stack should be stripped so that only `main` remains, meaning live_allocations[0x5000].stack has length 1
        assert_eq!(
            aggregator
                .live_allocations
                .get(&0x5000)
                .unwrap()
                .stack
                .len(),
            1
        );
        assert_eq!(
            aggregator.live_allocations.get(&0x5000).unwrap().stack[0],
            id_main
        );
    }

    #[test]
    fn test_determinism() {
        let mut aggregator1 = Aggregator::new();
        let mut aggregator2 = Aggregator::new();

        let events = vec![
            AllocationEvent::Alloc {
                address: 0x1,
                size: 64,
                thread_id: 1,
                sequence: 1,
                stack: vec![crate::types::FrameKey {
                    module_base: 0,
                    instruction_pointer: 10,
                }],
            },
            AllocationEvent::Alloc {
                address: 0x2,
                size: 128,
                thread_id: 1,
                sequence: 2,
                stack: vec![crate::types::FrameKey {
                    module_base: 0,
                    instruction_pointer: 10,
                }],
            },
            AllocationEvent::Free {
                address: 0x1,
                thread_id: 1,
                sequence: 3,
            },
        ];

        for event in &events {
            aggregator1.process_event(event.clone());
            aggregator2.process_event(event.clone());
        }

        assert_eq!(aggregator1.root.live_bytes, aggregator2.root.live_bytes);
        assert_eq!(aggregator1.root.total_bytes, aggregator2.root.total_bytes);
        assert_eq!(
            aggregator1.live_allocations.len(),
            aggregator2.live_allocations.len()
        );
        assert_eq!(aggregator1.interner.len(), aggregator2.interner.len());
    }
}
