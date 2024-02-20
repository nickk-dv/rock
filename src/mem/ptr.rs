use std::marker::PhantomData;

#[derive(Copy, Clone, PartialEq)]
pub struct P<T: Copy> {
    ptr: usize,
    phantom: PhantomData<T>,
}

impl<T: Copy> P<T> {
    pub(super) fn new(ptr: *mut T) -> Self {
        P {
            ptr: ptr as usize,
            phantom: PhantomData,
        }
    }

    pub(super) fn null() -> Self {
        P {
            ptr: 0,
            phantom: PhantomData,
        }
    }

    pub(super) fn is_null(&self) -> bool {
        self.ptr == 0
    }

    pub(super) fn as_mut(&self) -> *mut T {
        self.ptr as *mut T
    }
}

impl<T: Copy> std::ops::Deref for P<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.as_mut() }
    }
}

impl<T: Copy> std::ops::DerefMut for P<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.as_mut() }
    }
}
