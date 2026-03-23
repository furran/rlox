use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    fmt::{self, Display},
    hash::{Hash, Hasher},
    ops::Deref,
    ptr::NonNull,
};

pub use rlox_gc_derive::Trace;
pub extern crate self as rlox_gc;

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

#[repr(C)]
struct GcHeader {
    next: Option<NonNull<GcHeader>>,
    marked: Cell<bool>,
    size: usize,
    trace: unsafe fn(NonNull<GcHeader>),
    drop: unsafe fn(NonNull<GcHeader>),
    #[cfg(feature = "gc_log")]
    pub type_name: &'static str,
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
        self.mark();
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
impl Trace for *const u8 {
    fn trace(&self) {}
}
impl Trace for () {
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
impl<K: Trace, V: Trace, S> Trace for HashMap<K, V, S> {
    fn trace(&self) {
        for (k, v) in self.iter() {
            k.trace();
            v.trace();
        }
    }
}
impl<T: Trace, S> Trace for HashSet<T, S> {
    fn trace(&self) {
        for v in self {
            v.trace();
        }
    }
}

impl<T: Trace> Trace for RefCell<T> {
    fn trace(&self) {
        self.borrow().trace();
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

    pub fn get_bytes_alloc(&self) -> usize {
        self.bytes_alloc
    }

    pub fn allocate<T: Trace>(&mut self, value: T) -> Gc<T> {
        self.bytes_alloc += std::mem::size_of::<GcObject<T>>();

        let obj = Box::new(GcObject {
            header: GcHeader {
                next: self.head,
                marked: Cell::new(false),
                size: std::mem::size_of::<GcObject<T>>(),
                trace: trace_fn::<T>,
                drop: drop_fn::<T>,
                #[cfg(feature = "gc_log")]
                type_name: std::any::type_name::<T>(),
            },
            value,
        });

        let obj_ptr: NonNull<GcObject<T>> = NonNull::from(Box::leak(obj));
        let header_ptr: NonNull<GcHeader> = obj_ptr.cast::<GcHeader>();

        self.head = Some(header_ptr);

        Gc { ptr: obj_ptr }
    }

    pub fn mark(&self, roots: &dyn Trace) {
        roots.trace();
    }

    pub fn sweep(&mut self) {
        let mut cursor: *mut Option<NonNull<GcHeader>> = &mut self.head;

        while let Some(ptr) = unsafe { *cursor } {
            let marked = unsafe { (*ptr.as_ptr()).marked.get() };
            if marked {
                #[cfg(feature = "gc_log")]
                println!("GC: skipping {}", unsafe { (*ptr.as_ptr()).type_name },);
                unsafe { (*ptr.as_ptr()).marked.set(false) };
                cursor = unsafe { &raw mut (*ptr.as_ptr()).next };
            } else {
                #[cfg(feature = "gc_log")]
                println!("GC: sweeping {}", unsafe { (*ptr.as_ptr()).type_name },);
                let next = unsafe { (*ptr.as_ptr()).next };
                let size = unsafe { (*ptr.as_ptr()).size };
                unsafe { *cursor = next };
                self.bytes_alloc -= size;
                unsafe { ((*ptr.as_ptr()).drop)(ptr) };
            }
        }
    }

    pub fn should_collect(&self) -> bool {
        #[cfg(feature = "gc_stress")]
        {
            return true;
        }
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

#[cfg(test)]
mod test {
    use super::*;

    struct NoRoots;
    impl Trace for NoRoots {
        fn trace(&self) {}
    }

    #[derive(Trace)]
    struct TestObj {
        value: f64,
    }

    #[test]
    fn test_gc_collects_unreachable() {
        let mut heap = Heap::new();
        let _gc = heap.allocate(TestObj { value: 42.0 });

        heap.mark(&NoRoots);
        heap.sweep();

        assert_eq!(heap.bytes_alloc, 0);
    }

    #[test]
    fn test_gc_keeps_reachable() {
        let mut heap = Heap::new();
        let gc = heap.allocate(TestObj { value: 42.0 });
        heap.mark(&[gc]);

        heap.sweep();
        assert!(heap.bytes_alloc > 0);
        assert_eq!(gc.value, 42.0);
    }

    #[derive(Trace)]
    struct Node {
        next: Option<Gc<Node>>,
        value: f64,
    }

    #[test]
    fn test_gc_traces_object_graph() {
        let mut heap = Heap::new();
        let leaf = heap.allocate(Node {
            next: None,
            value: 2.0,
        });
        let root = heap.allocate(Node {
            next: Some(leaf),
            value: 1.0,
        });

        heap.mark(&root);
        assert_eq!(heap.bytes_alloc, size_of::<GcObject<Node>>() * 2);
        assert_eq!(root.value, 1.0);
        assert_eq!(root.next.unwrap().value, 2.0);
    }

    #[test]
    fn test_gc_collects_cycles() {
        let mut heap = Heap::new();
        let a = heap.allocate(Node {
            next: None,
            value: 1.0,
        });
        let b = heap.allocate(Node {
            next: Some(a),
            value: 2.0,
        });

        // make a cyclical reference
        unsafe {
            (*a.ptr.as_ptr()).value.next = Some(b);
        }

        heap.mark(&NoRoots);
        heap.sweep();

        assert_eq!(heap.bytes_alloc, 0);
    }

    #[test]
    fn test_gc_collects_cycles_rooted() {
        let mut heap = Heap::new();
        let a = heap.allocate(Node {
            next: None,
            value: 1.0,
        });
        let b = heap.allocate(Node {
            next: Some(a),
            value: 2.0,
        });

        // make a cyclical reference
        unsafe {
            (*a.ptr.as_ptr()).value.next = Some(b);
        }

        // root one of them
        heap.mark(&a);
        heap.sweep();

        assert_eq!(heap.bytes_alloc, size_of::<GcObject<Node>>() * 2);
    }

    #[derive(Trace)]
    struct Inner {
        value: Gc<Node>,
    }

    #[derive(Trace)]
    struct Outer {
        inner: Inner,
        direct: Gc<Node>,
    }

    #[test]
    fn test_gc_traces_nested_struct() {
        let mut heap = Heap::new();
        let node1 = heap.allocate(Node {
            next: None,
            value: 1.0,
        });
        let node2 = heap.allocate(Node {
            next: None,
            value: 2.0,
        });

        let outer = heap.allocate(Outer {
            inner: Inner { value: node1 },
            direct: node2,
        });

        heap.mark(&outer);
        heap.sweep();

        assert_eq!(
            heap.bytes_alloc,
            size_of::<GcObject<Outer>>() + size_of::<GcObject<Node>>() * 2
        );
        assert_eq!(outer.inner.value.value, 1.0);
        assert_eq!(outer.direct.value, 2.0);
    }
}
