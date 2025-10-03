use std::borrow::Cow;

use crate::common::{ObjString, ObjectRef};

#[derive(Debug)]
pub struct Heap<'src> {
    strings: Vec<Box<ObjString<'src>>>,
}

impl<'src> Heap<'src> {
    pub fn new() -> Self {
        Heap {
            strings: Vec::new(),
        }
    }

    pub fn alloc_owned_string(&mut self, chars: String) -> ObjectRef<'src> {
        let boxed = Box::new(ObjString {
            chars: Cow::Owned(chars),
        });

        let ptr = &*boxed as *const ObjString;
        self.strings.push(boxed);

        ObjectRef::String(ptr)
    }

    pub fn alloc_borrowed_string(&mut self, chars: &'src str) -> ObjectRef<'src> {
        let boxed = Box::new(ObjString {
            chars: Cow::Borrowed(chars),
        });
        let ptr = &*boxed as *const ObjString;
        self.strings.push(boxed);

        ObjectRef::String(ptr)
    }
}
