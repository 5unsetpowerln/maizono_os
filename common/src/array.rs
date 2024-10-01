#[repr(align(64))]
pub struct AlignedArray64<T, const N: usize>([T; N]);

impl<T, const N: usize> AlignedArray64<T, N> {
    pub const fn from_array(array: [T; N]) -> Self {
        Self(array)
    }

    pub unsafe fn as_ptr(&self) -> *const T {
        self.0.as_ptr()
    }
}
