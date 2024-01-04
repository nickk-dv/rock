use std::marker::PhantomData;
pub type Rawptr = usize;

#[derive(Copy, Clone)]
pub struct P<T: Copy> {
    ptr: Rawptr,
    phantom: PhantomData<T>,
}

impl<T: Copy> P<T> {
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

    pub fn raw(&self) -> Rawptr {
        self.ptr
    }
}

impl<T: Copy> std::ops::Deref for P<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*(self.ptr as *mut T) }
    }
}

impl<T: Copy> std::ops::DerefMut for P<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *(self.ptr as *mut T) }
    }
}
