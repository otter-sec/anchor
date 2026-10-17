//! Small, fixed-capacity vectors for transient on-chain data.
//!
//! These buffers deliberately panic on overflow, matching the failure mode of
//! the old heap-backed `Vec` while keeping the storage inline and bounded.

use core::{
    fmt,
    mem::MaybeUninit,
    ops::{
        Deref, DerefMut, Index, IndexMut, Range, RangeFrom, RangeFull, RangeInclusive, RangeTo,
        RangeToInclusive,
    },
    slice,
};

pub struct FixedVec<T, const N: usize> {
    data: [MaybeUninit<T>; N],
    len: usize,
}

impl<T: fmt::Debug, const N: usize> fmt::Debug for FixedVec<T, N> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_list().entries(self.as_slice()).finish()
    }
}

impl<T, const N: usize> FixedVec<T, N> {
    #[inline]
    pub const fn new() -> Self {
        Self {
            data: [const { MaybeUninit::uninit() }; N],
            len: 0,
        }
    }

    #[inline]
    pub const fn capacity(&self) -> usize {
        N
    }

    #[inline]
    pub const fn len(&self) -> usize {
        self.len
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
    pub const fn is_full(&self) -> bool {
        self.len == N
    }

    #[inline]
    pub fn push(&mut self, value: T) {
        assert!(self.len < N, "fixed vector capacity exceeded");
        self.data[self.len].write(value);
        self.len += 1;
    }

    #[inline]
    pub fn try_push(&mut self, value: T) -> Result<(), T> {
        if self.len == N {
            return Err(value);
        }
        self.data[self.len].write(value);
        self.len += 1;
        Ok(())
    }

    #[inline]
    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }
        self.len -= 1;
        Some(unsafe { self.data[self.len].assume_init_read() })
    }

    #[inline]
    pub fn clear(&mut self) {
        while self.pop().is_some() {}
    }

    #[inline]
    pub fn truncate(&mut self, len: usize) {
        while self.len > len {
            let _ = self.pop();
        }
    }

    #[inline]
    pub fn resize(&mut self, len: usize, value: T)
    where
        T: Clone,
    {
        assert!(len <= N, "fixed vector capacity exceeded");
        while self.len < len {
            self.push(value.clone());
        }
        self.truncate(len);
    }

    #[inline]
    pub fn as_slice(&self) -> &[T] {
        unsafe { slice::from_raw_parts(self.data.as_ptr().cast(), self.len) }
    }

    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        unsafe { slice::from_raw_parts_mut(self.data.as_mut_ptr().cast(), self.len) }
    }

    #[inline]
    pub fn iter(&self) -> slice::Iter<'_, T> {
        self.as_slice().iter()
    }

    #[inline]
    pub fn iter_mut(&mut self) -> slice::IterMut<'_, T> {
        self.as_mut_slice().iter_mut()
    }

    #[inline]
    pub fn extend_from_slice(&mut self, values: &[T])
    where
        T: Copy,
    {
        assert!(self.len + values.len() <= N, "fixed vector capacity exceeded");
        for value in values {
            self.push(*value);
        }
    }

    #[inline]
    pub fn try_extend_from_slice(&mut self, values: &[T]) -> Result<(), ()>
    where
        T: Copy,
    {
        if self.len + values.len() > N {
            return Err(());
        }
        for value in values {
            self.push(*value);
        }
        Ok(())
    }

}

impl<const N: usize> FixedVec<u8, N> {
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        self.as_slice()
    }
}

impl<T, const N: usize> Default for FixedVec<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const N: usize> Drop for FixedVec<T, N> {
    fn drop(&mut self) {
        self.clear();
    }
}

impl<T: Clone, const N: usize> Clone for FixedVec<T, N> {
    fn clone(&self) -> Self {
        let mut out = Self::new();
        for value in self {
            out.push(value.clone());
        }
        out
    }
}

impl<T, const N: usize> Deref for FixedVec<T, N> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<T, const N: usize> DerefMut for FixedVec<T, N> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut_slice()
    }
}

impl<T, const N: usize> AsRef<[T]> for FixedVec<T, N> {
    fn as_ref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T, const N: usize> Index<usize> for FixedVec<T, N> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        &self.as_slice()[index]
    }
}

impl<T, const N: usize> IndexMut<usize> for FixedVec<T, N> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.as_mut_slice()[index]
    }
}

macro_rules! impl_slice_index {
    ($($range:ty),* $(,)?) => {
        $(
            impl<T, const N: usize> Index<$range> for FixedVec<T, N> {
                type Output = [T];
                fn index(&self, index: $range) -> &Self::Output {
                    &self.as_slice()[index]
                }
            }
        )*
    };
}

impl_slice_index!(Range<usize>, RangeFrom<usize>, RangeFull, RangeInclusive<usize>, RangeTo<usize>, RangeToInclusive<usize>);

impl<'a, T, const N: usize> IntoIterator for &'a FixedVec<T, N> {
    type Item = &'a T;
    type IntoIter = slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, T, const N: usize> IntoIterator for &'a mut FixedVec<T, N> {
    type Item = &'a mut T;
    type IntoIter = slice::IterMut<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<T, const N: usize> Extend<T> for FixedVec<T, N> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for value in iter {
            self.push(value);
        }
    }
}

impl<const N: usize> wincode::io::Writer for FixedVec<u8, N> {
    fn write(&mut self, src: &[u8]) -> wincode::io::WriteResult<()> {
        self.try_extend_from_slice(src)
            .map_err(|_| wincode::io::WriteError::WriteSizeLimit(src.len()))
    }
}
