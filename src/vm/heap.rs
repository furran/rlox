use crate::object::{ObjRef, ObjString, ObjStringPtr, Object};
use std::collections::HashSet;

#[derive(Debug)]
pub struct Heap {
    objects: Vec<Box<Object>>,
    strings: HashSet<ObjStringPtr>,
}

impl Heap {
    pub fn new() -> Self {
        Self {
            objects: Vec::new(),
            strings: HashSet::new(),
        }
    }

    pub fn alloc_string(&mut self, s: &str) -> ObjRef {
        if let Some(existing) = self.get_interned(s) {
            return existing;
        }
        let obj_ref = self.alloc(Object::String(ObjString { str: s.to_string() }));

        self.intern(ObjStringPtr::from(obj_ref));
        obj_ref
    }

    pub fn alloc(&mut self, obj: Object) -> ObjRef {
        let mut boxed = Box::new(obj);
        let ptr = boxed.as_mut() as *mut Object;
        self.objects.push(boxed);
        ObjRef::from(ptr)
    }

    fn get_interned(&self, s: &str) -> Option<ObjRef> {
        self.strings.get(s).map(|ptr| ObjRef::from(ptr.as_ptr()))
    }

    fn intern(&mut self, ptr: ObjStringPtr) {
        self.strings.insert(ptr);
    }
}
