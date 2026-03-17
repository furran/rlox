use std::{
    borrow::Borrow,
    collections::HashSet,
    hash::{Hash, Hasher},
};

use rlox_gc::{Gc, Heap, Trace};

use crate::object::ObjString;

#[derive(Debug)]
struct InternedString(Gc<ObjString>);

impl Hash for InternedString {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.str.hash(state);
    }
}

impl PartialEq for InternedString {
    fn eq(&self, other: &Self) -> bool {
        self.0.str == other.0.str
    }
}

impl Eq for InternedString {}

impl Borrow<str> for InternedString {
    fn borrow(&self) -> &str {
        &self.0.str
    }
}

#[derive(Debug)]
pub struct LoxHeap {
    heap: Heap,
    interned_strings: HashSet<InternedString>,
}

impl LoxHeap {
    pub fn new() -> Self {
        Self {
            heap: Heap::new(),
            interned_strings: HashSet::new(),
        }
    }

    pub fn allocate<T: Trace>(&mut self, value: T) -> Gc<T> {
        self.heap.allocate(value)
    }

    pub fn intern(&mut self, s: &str) -> Gc<ObjString> {
        if let Some(existing) = self.interned_strings.get(s) {
            return existing.0;
        }
        let gc = self.heap.allocate(ObjString { str: s.to_owned() });
        self.interned_strings.insert(InternedString(gc));
        gc
    }
}
