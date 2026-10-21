use serde_json::Value;
use std::collections::{HashSet, VecDeque};

/// A generational arena that keeps system prompts (Gen 0) pinned,
/// and uses a ring buffer for transient tool calls and conversation turns.
pub struct MemoryArena {
    /// Pinned Generation 0 arena (system prompts, root directives)
    gen0_pinned: Vec<Value>,

    /// Ring buffer for transient interactions (user turns, model replies, tool calls)
    transient_ring: VecDeque<Value>,

    /// Maximum capacity of the transient ring buffer before eviction
    max_transient_capacity: usize,
}

impl MemoryArena {
    pub fn new(max_transient_capacity: usize) -> Self {
        Self {
            gen0_pinned: Vec::new(),
            transient_ring: VecDeque::with_capacity(max_transient_capacity),
            max_transient_capacity,
        }
    }

    /// Pin a system prompt or root directive.
    pub fn pin_gen0(&mut self, item: Value) {
        self.gen0_pinned.push(item);
    }

    /// Add a transient item. If capacity is exceeded, the oldest item is evicted.
    pub fn push_transient(&mut self, item: Value) -> Option<Value> {
        let evicted = if self.transient_ring.len() >= self.max_transient_capacity {
            self.transient_ring.pop_front()
        } else {
            None
        };
        self.transient_ring.push_back(item);
        evicted
    }

    /// Prunes transient turns whose indices are in `prune_indices`.
    /// Returns the number of turns removed from the active ring buffer.
    pub fn sever_loops(&mut self, prune_indices: &HashSet<usize>) -> usize {
        if prune_indices.is_empty() {
            return 0;
        }
        let before = self.transient_ring.len();
        let mut retained = VecDeque::with_capacity(before);
        for (idx, item) in self.transient_ring.drain(..).enumerate() {
            if !prune_indices.contains(&idx) {
                retained.push_back(item);
            }
        }
        self.transient_ring = retained;
        before.saturating_sub(self.transient_ring.len())
    }

    pub fn gen0(&self) -> &[Value] {
        &self.gen0_pinned
    }

    pub fn transient(&self) -> &VecDeque<Value> {
        &self.transient_ring
    }
}
