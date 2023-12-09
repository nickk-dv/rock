#[derive(Copy, Clone)]
pub struct P<T: Copy> {
    ptr: *mut T,
}

impl<T: Copy> P<T> {
    pub fn new(ptr: *mut T) -> Self {
        P { ptr }
    }

    pub fn null() -> Self {
        P {
            ptr: std::ptr::null_mut() as *mut T,
        }
    }

    pub fn is_null(&self) -> bool {
        self.ptr.is_null()
    }
}

impl<T: Copy> std::ops::Deref for P<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.ptr }
    }
}

impl<T: Copy> std::ops::DerefMut for P<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.ptr }
    }
}
