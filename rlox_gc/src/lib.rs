use std::{
    cell::Cell,
    collections::{HashMap, HashSet},
    fmt::{self, Display},
    hash::{Hash, Hasher},
    ops::Deref,
    ptr::NonNull,
};

pub use rlox_gc_derive::Trace;

#[derive(Debug)]
pub struct Gc<T> {
    ptr: NonNull<GcObject<T>>,
}

impl<T> Copy for Gc<T> {}
impl<T> Clone for Gc<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Deref for Gc<T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &self.ptr.as_ref().value }
    }
}

impl<T> PartialEq for Gc<T> {
    fn eq(&self, other: &Self) -> bool {
        self.ptr.as_ptr() == other.ptr.as_ptr()
    }
}

impl<T> Eq for Gc<T> {}

impl<T> Hash for Gc<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.ptr.as_ptr().hash(state);
    }
}

impl<T: Display> Display for Gc<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        unsafe { self.ptr.as_ref().value.fmt(f) }
    }
}

impl<T> From<NonNull<GcObject<T>>> for Gc<T> {
    fn from(ptr: NonNull<GcObject<T>>) -> Self {
        Self { ptr }
    }
}

impl<T> Gc<T> {
    pub fn mark(&self) {
        let header = unsafe { self.ptr.cast::<GcHeader>().as_ref() };
        if header.is_marked() {
            return;
        }
        header.set_marked(true);
        unsafe { (header.trace)(self.ptr.cast::<GcHeader>()) };
    }

    pub fn is_marked(&self) -> bool {
        let header = unsafe { self.ptr.cast::<GcHeader>().as_ref() };
        header.is_marked()
    }
}

struct GcHeader {
    next: Option<NonNull<GcHeader>>,
    marked: Cell<bool>,
    trace: unsafe fn(NonNull<GcHeader>),
    drop: unsafe fn(NonNull<GcHeader>),
}

impl GcHeader {
    pub fn is_marked(&self) -> bool {
        self.marked.get()
    }

    pub fn set_marked(&self, val: bool) {
        self.marked.set(val);
    }
}

#[repr(C)]
struct GcObject<T> {
    pub header: GcHeader,
    pub value: T,
}

pub trait Trace {
    fn trace(&self);
}

impl<T: Trace> Trace for Gc<T> {
    fn trace(&self) {
        unsafe { self.ptr.as_ref().trace() };
    }
}

impl<T: Trace> Trace for GcObject<T> {
    fn trace(&self) {
        self.value.trace();
    }
}

impl Trace for String {
    fn trace(&self) {}
}
impl Trace for usize {
    fn trace(&self) {}
}
impl Trace for f64 {
    fn trace(&self) {}
}
impl Trace for u64 {
    fn trace(&self) {}
}
impl Trace for bool {
    fn trace(&self) {}
}
impl Trace for u8 {
    fn trace(&self) {}
}
impl<T: Trace + Copy> Trace for Cell<T> {
    fn trace(&self) {
        self.get().trace();
    }
}
impl<T: Trace> Trace for Vec<T> {
    fn trace(&self) {
        for item in self {
            item.trace();
        }
    }
}
impl<K, V: Trace> Trace for HashMap<K, V> {
    fn trace(&self) {
        for v in self.values() {
            v.trace();
        }
    }
}
impl<T: Trace> Trace for HashSet<T> {
    fn trace(&self) {
        for v in self {
            v.trace();
        }
    }
}
impl<T: Trace, const N: usize> Trace for [T; N] {
    fn trace(&self) {
        for v in self {
            v.trace();
        }
    }
}

impl<T: Trace> Trace for Option<T> {
    fn trace(&self) {
        if let Some(value) = self {
            value.trace();
        }
    }
}

#[derive(Debug)]
pub struct Heap {
    head: Option<NonNull<GcHeader>>,
    bytes_alloc: usize,
    pub threshold: usize,
}

impl Heap {
    pub fn new() -> Self {
        Self {
            head: None,
            bytes_alloc: 0,
            threshold: 1024 * 1024,
        }
    }

    pub fn allocate<T: Trace>(&mut self, value: T) -> Gc<T> {
        self.bytes_alloc += std::mem::size_of::<GcObject<T>>();

        let mut obj = Box::new(GcObject {
            header: GcHeader {
                next: self.head,
                marked: Cell::new(false),
                trace: trace_fn::<T>,
                drop: drop_fn::<T>,
            },
            value,
        });
        let header_ptr: NonNull<GcHeader> = NonNull::from(&mut obj.header);
        let obj_ptr: NonNull<GcObject<T>> = NonNull::from(Box::leak(obj));

        self.head = Some(header_ptr);

        Gc { ptr: obj_ptr }
    }

    pub fn mark(&self, roots: &dyn Trace) {
        roots.trace();
    }

    pub fn sweep(&mut self) {
        let mut cursor = &mut self.head;

        while let Some(ptr) = *cursor {
            let header = unsafe { &*ptr.as_ptr() };

            if header.marked.get() {
                header.marked.set(false);
                cursor = unsafe { &mut (*ptr.as_ptr()).next };
            } else {
                *cursor = header.next;
                self.bytes_alloc -= unsafe { size_of_val(&*ptr.as_ptr()) };
                unsafe { (header.drop)(ptr) };
            }
        }
    }

    pub fn should_collect(&self) -> bool {
        self.bytes_alloc > self.threshold
    }

    pub fn update_threshold(&mut self) {
        self.threshold = self.bytes_alloc * 2;
    }
}

unsafe fn trace_fn<T: Trace>(ptr: NonNull<GcHeader>) {
    let obj = ptr.cast::<GcObject<T>>();
    unsafe { obj.as_ref().value.trace() };
}

unsafe fn drop_fn<T>(ptr: NonNull<GcHeader>) {
    drop(unsafe { Box::from_raw(ptr.cast::<GcObject<T>>().as_ptr()) });
}
