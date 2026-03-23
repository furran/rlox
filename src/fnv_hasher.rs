use std::{
    collections::{HashMap, HashSet},
    hash::{BuildHasherDefault, Hasher},
    ops::{Deref, DerefMut},
};

use rlox_gc::Trace;

pub struct FnvHasher(u64);

impl Default for FnvHasher {
    fn default() -> Self {
        Self(2166136261)
    }
}

impl Hasher for FnvHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.0 ^= byte as u64;
            self.0 *= 16777619;
        }
    }
}

#[derive(Debug, Trace)]
pub struct FnvHashMap<K: Trace, V: Trace>(HashMap<K, V, BuildHasherDefault<FnvHasher>>);
#[derive(Debug)]
pub struct FnvHashSet<K>(HashSet<K, BuildHasherDefault<FnvHasher>>);

impl<K> FnvHashSet<K> {
    pub fn new() -> Self {
        Self(HashSet::with_hasher(Default::default()))
    }
}

impl<K> Deref for FnvHashSet<K> {
    type Target = HashSet<K, BuildHasherDefault<FnvHasher>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<K> DerefMut for FnvHashSet<K> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<K: Trace, V: Trace> FnvHashMap<K, V> {
    pub fn new() -> Self {
        Self(HashMap::with_hasher(Default::default()))
    }
}

impl<K: Trace, V: Trace> Deref for FnvHashMap<K, V> {
    type Target = HashMap<K, V, BuildHasherDefault<FnvHasher>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<K: Trace, V: Trace> DerefMut for FnvHashMap<K, V> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
