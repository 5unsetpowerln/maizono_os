// use alloc::collections::vec_deque::VecDeque;

// pub struct Queue<T> {
//     inner: VecDeque<T>,
// }

// impl<T> Queue<T> {
//     pub const fn new() -> Self {
//         Self {
//             inner: VecDeque::new(),
//         }
//     }

//     pub fn enqueue(&mut self, item: T) {
//         self.inner.push_back(item)
//     }

//     pub fn dequeue(&mut self) -> Option<T> {
//         self.inner.pop_front()
//     }

//     pub fn front(&mut self) -> Option<T> {
//         self.inner.front()
//     }

//     pub fn is_empty(&self) -> bool {
//         self.inner.is_empty()
//     }

//     pub fn len(&self) -> usize {
//         self.inner.len()
//     }
// }
