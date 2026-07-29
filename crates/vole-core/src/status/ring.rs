//! 固定大小环缓冲，对齐 mole `RingBuffer`。

#[derive(Debug, Clone)]
pub struct RingBuffer<T: Copy> {
    data: Vec<T>,
    index: usize,
    size: usize,
    cap: usize,
}

impl<T: Copy + Default> RingBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        Self {
            data: vec![T::default(); cap],
            index: 0,
            size: 0,
            cap,
        }
    }

    pub fn add(&mut self, val: T) {
        self.data[self.index] = val;
        self.index = (self.index + 1) % self.cap;
        if self.size < self.cap {
            self.size += 1;
        }
    }

    /// 按时间顺序返回有效元素（最旧 → 最新）。
    pub fn slice(&self) -> Vec<T> {
        if self.size == 0 {
            return Vec::new();
        }
        let mut res = Vec::with_capacity(self.size);
        if self.size < self.cap {
            res.extend_from_slice(&self.data[..self.size]);
        } else {
            res.extend_from_slice(&self.data[self.index..]);
            res.extend_from_slice(&self.data[..self.index]);
        }
        res
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chronological_after_wrap() {
        let mut rb = RingBuffer::new(5);
        for v in [1.0, 2.0, 3.0, 4.0, 5.0] {
            rb.add(v);
        }
        assert_eq!(rb.slice(), vec![1.0, 2.0, 3.0, 4.0, 5.0]);
        rb.add(6.0);
        assert_eq!(rb.slice(), vec![2.0, 3.0, 4.0, 5.0, 6.0]);
    }
}
