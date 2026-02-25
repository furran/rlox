use std::{collections::HashSet, hash::Hash};

use crate::common::ObjString;

#[derive(Debug)]
pub struct Interner {
    set: HashSet<Box<ObjString>>,
}

impl Interner {
    pub fn new() -> Self {
        Self {
            set: HashSet::new(),
        }
    }

    pub fn intern(&mut self, e: &str) -> *const ObjString {
        if let Some(existing) = self.set.get(e) {
            return existing.as_ref() as *const ObjString;
        }
        let obj = Box::new(ObjString { str: e.to_string() });
        let ptr = obj.as_ref() as *const ObjString;
        self.set.insert(obj);
        ptr
    }
}
