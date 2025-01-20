#[derive(Debug, Copy, Clone)]
pub struct PhysPtr {
    ptr: u64,
}

impl PhysPtr {
    pub const fn null() -> Self {
        Self { ptr: 0 }
    }

    pub fn from_ref<T>(ref_: &T) -> Self {
        Self {
            ptr: ref_ as *const T as u64,
        }
    }

    pub fn from_ptr<T>(ptr: *const T) -> Self {
        Self { ptr: ptr as u64 }
    }

    pub fn is_null(&self) -> bool {
        self.ptr == 0
    }

    pub fn ptr<T>(&self) -> *const T {
        self.ptr as *const T
    }

    pub fn mut_ptr<T>(&self) -> *mut T {
        self.ptr as *mut T
    }

    pub unsafe fn ref_<T>(&self) -> &T {
        &*self.ptr()
    }

    pub fn get(&self) -> u64 {
        self.ptr
    }

    pub fn set(&mut self, ptr: u64) {
        self.ptr = ptr;
    }
}
