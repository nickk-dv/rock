use std::marker::PhantomData;

pub type Rawptr = usize;

#[derive(Copy, Clone)]
pub struct P<T> {
    ptr: Rawptr,
    phantom: PhantomData<T>,
}

impl<T> P<T> {
    pub fn new(ptr: Rawptr) -> Self {
        P {
            ptr,
            phantom: PhantomData,
        }
    }

    pub fn null() -> Self {
        P {
            ptr: 0,
            phantom: PhantomData,
        }
    }

    pub fn is_null(&self) -> bool {
        self.ptr == 0
    }

    pub fn as_raw(&self) -> Rawptr {
        self.ptr
    }

    pub fn as_mut(&self) -> *mut T {
        self.ptr as *mut T
    }

    pub fn copy(&self) -> P<T> {
        P::new(self.as_raw())
    }

    pub fn add(&self, count: usize) -> P<T> {
        let offset = unsafe { self.as_mut().add(count) };
        P::new(offset as Rawptr)
    }
}

impl<T> std::ops::Deref for P<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*(self.ptr as *mut T) }
    }
}

impl<T> std::ops::DerefMut for P<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *(self.ptr as *mut T) }
    }
}
