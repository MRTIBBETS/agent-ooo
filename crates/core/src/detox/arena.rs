use serde_json::Value;
use std::collections::VecDeque;

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

    pub fn gen0(&self) -> &[Value] {
        &self.gen0_pinned
    }

    pub fn transient(&self) -> &VecDeque<Value> {
        &self.transient_ring
    }
}
