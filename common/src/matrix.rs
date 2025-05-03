use core::ops::{Add, AddAssign, Sub};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec2<T: Add<Output = T> + Sub<Output = T> + AddAssign + PartialOrd + Copy> {
    pub x: T,
    pub y: T,
    // pub x_min: T,
    // pub x_max: T,
    // pub y_min: T,
    // pub y_max: T,
}

impl<T: Add<Output = T> + Sub<Output = T> + AddAssign + PartialOrd + Copy> Vec2<T> {
    pub const fn new(
        x: T,
        y: T,
        // x_min: T, x_max: T, y_min: T, y_max: T
    ) -> Self {
        Self {
            x,
            y,
            // x_min,
            // x_max,
            // y_min,
            // y_max,
        }
    }

    pub const fn newa(x: T, y: T) -> Self {
        Self {
            x,
            y,
            // x_min: T::,
            // x_max,
            // y_min,
            // y_max,
        }
    }

    // this function is called automatically when calculation or assignments orrur.
    // pub fn clip(&mut self) {
    //     if self.x > self.x_max {
    //         self.x = self.x_max;
    //     }
    //     if self.x < self.x_min {
    //         self.x = self.x_min;
    //     }

    //     if self.y > self.y_max {
    //         self.y = self.y_max;
    //     }
    //     if self.y < self.y_min {
    //         self.y = self.y_min;
    //     }
    // }
}

impl<T: Add<Output = T> + Sub<Output = T> + AddAssign + PartialOrd + Copy> Add for Vec2<T> {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        // let mut s =
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
            // x_min: self.x_min,
            // x_max: self.x_max,
            // y_min: self.y_min,
            // y_max: self.y_max,
        }
        // s.clip();
        // s
    }
}

impl<T: Add<Output = T> + Sub<Output = T> + AddAssign + PartialOrd + Copy> Sub for Vec2<T> {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        // let mut s =
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
            // x_min: self.x_min,
            // x_max: self.x_max,
            // y_min: self.y_min,
            // y_max: self.y_max,
        }
        // s.clip();
        // s
    }
}

impl<T: Add<Output = T> + Sub<Output = T> + AddAssign + PartialOrd + Copy> AddAssign for Vec2<T> {
    fn add_assign(&mut self, rhs: Self) {
        self.x += rhs.x;
        self.y += rhs.y;
        // self.clip();
    }
}
