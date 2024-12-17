pub struct CapacityError();
impl CapacityError {
    fn new() -> Self {
        Self()
    }
}

#[derive(Clone, Copy)]
pub struct ArrayVec<T: Copy, const CAP: usize> {
    array: [Option<T>; CAP],
    index: usize,
}

impl<T: Copy, const CAP: usize> ArrayVec<T, CAP> {
    pub const fn new() -> Self {
        Self {
            array: [None; CAP],
            index: 0,
        }
    }

    pub fn push(&mut self, new_element: T) -> Result<(), CapacityError> {
        *self
            .array
            .iter_mut()
            .find(|e| e.is_none())
            .ok_or(CapacityError::new())? = Some(new_element);
        Ok(())
    }
}

impl<T: Copy, const CAP: usize> Iterator for ArrayVec<T, CAP> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(current) = self.array[self.index] {
            self.index += 1;
            Some(current)
        } else {
            None
        }
    }
}
