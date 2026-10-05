//! A map that keeps at most `cap` entries and lets the oldest insertion go first: the note cache
//! and the semantic cache. Insertion order is kept explicitly so eviction is deterministic;
//! updating a key already held is not a new entry and evicts nothing.
use std::borrow::Borrow;
use std::collections::{HashMap, VecDeque};
use std::hash::Hash;

#[derive(Debug)]
pub(crate) struct FifoMap<K, V> {
    entries: HashMap<K, V>,
    /// Insertion order, oldest first.
    order: VecDeque<K>,
    cap: usize,
}

impl<K: Eq + Hash + Clone, V> FifoMap<K, V> {
    pub(crate) fn new(cap: usize) -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            cap,
        }
    }

    pub(crate) fn get<Q: Eq + Hash + ?Sized>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
    {
        self.entries.get(key)
    }

    pub(crate) fn insert(&mut self, key: K, value: V) {
        if self.entries.insert(key.clone(), value).is_none() {
            self.order.push_back(key);
        }
        while self.entries.len() > self.cap {
            match self.order.pop_front() {
                Some(oldest) => {
                    self.entries.remove(&oldest);
                }
                None => break,
            }
        }
    }

    pub(crate) fn remove<Q: Eq + Hash + ?Sized>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
    {
        let gone = self.entries.remove(key);
        if gone.is_some() {
            self.order.retain(|k| k.borrow() != key);
        }
        gone
    }

    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.entries.iter()
    }
}
