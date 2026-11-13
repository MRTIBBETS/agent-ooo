use serde_json::Value;
use std::collections::{HashSet, VecDeque};

const MAX_USER_ANCHORS: usize = 32;

/// A generational arena that keeps system prompts (Gen 0) and durable user
/// directives pinned across the entire session, and uses a ring buffer for
/// transient tool calls and conversation turns.
pub struct MemoryArena {
    /// Pinned Generation 0 arena (system prompts, root directives)
    gen0_pinned: Vec<Value>,

    /// Bounded ledger of explicit user turns across the entire session so
    /// user rules and negative constraints from turn 1 survive ring eviction
    user_anchors: VecDeque<Value>,

    /// Ring buffer for transient interactions (user turns, model replies, tool calls)
    transient_ring: VecDeque<Value>,

    /// Maximum capacity of the transient ring buffer before eviction
    max_transient_capacity: usize,
}

impl MemoryArena {
    pub fn new(max_transient_capacity: usize) -> Self {
        Self {
            gen0_pinned: Vec::new(),
            user_anchors: VecDeque::with_capacity(MAX_USER_ANCHORS),
            transient_ring: VecDeque::with_capacity(max_transient_capacity),
            max_transient_capacity,
        }
    }

    /// Pin a system prompt or root directive.
    pub fn pin_gen0(&mut self, item: Value) {
        self.gen0_pinned.push(item);
    }

    /// Record a user turn in the durable user anchor buffer (bounded to 32 turns).
    pub fn pin_user_anchor(&mut self, item: Value) {
        if self.user_anchors.len() >= MAX_USER_ANCHORS {
            // Preserve the very first user turn (index 0, often project rules/spec)
            // and rotate intermediate anchors.
            if self.user_anchors.len() > 1 {
                self.user_anchors.remove(1);
            } else {
                self.user_anchors.pop_front();
            }
        }
        self.user_anchors.push_back(item);
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

    pub fn user_anchors(&self) -> &VecDeque<Value> {
        &self.user_anchors
    }

    pub fn transient(&self) -> &VecDeque<Value> {
        &self.transient_ring
    }
}
