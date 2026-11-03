//! Small, fixed-capacity vectors for transient on-chain data.
//!
//! These buffers keep transient storage inline and bounded. Callers that need
//! normal error handling should use the `try_*` methods rather than the
//! panic-on-overflow convenience methods.

use {
    core::{
        fmt,
        mem::MaybeUninit,
        ops::{
            Deref, DerefMut, Index, IndexMut, Range, RangeFrom, RangeFull, RangeInclusive, RangeTo,
            RangeToInclusive,
        },
        slice, str,
    },
    wincode::len::SeqLen as _,
};

pub struct FixedVec<T, const N: usize> {
    data: [MaybeUninit<T>; N],
    len: usize,
}

/// UTF-8 string storage with a capacity selected by the caller.
pub struct FixedString<const N: usize>(FixedVec<u8, N>);

impl<const N: usize> FixedString<N> {
    #[inline]
    pub const fn new() -> Self {
        Self(FixedVec::new())
    }

    #[inline]
    pub const fn capacity(&self) -> usize {
        N
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        // SAFETY: bytes enter `FixedString` only through `try_from`,
        // `try_push_str`, or the validating wincode implementation below.
        unsafe { str::from_utf8_unchecked(self.0.as_slice()) }
    }

    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_slice()
    }

    #[inline]
    pub fn try_push_str(&mut self, value: &str) -> Result<(), ()> {
        self.0.try_extend_from_slice(value.as_bytes())
    }
}

impl<const N: usize> Default for FixedString<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> Clone for FixedString<N> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<const N: usize> fmt::Debug for FixedString<N> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_str().fmt(formatter)
    }
}

impl<const N: usize> Deref for FixedString<N> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl<const N: usize> AsRef<str> for FixedString<N> {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl<const N: usize> TryFrom<&str> for FixedString<N> {
    type Error = ();

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let mut string = Self::new();
        string.try_push_str(value)?;
        Ok(string)
    }
}

impl<const N: usize> TryFrom<&[u8]> for FixedString<N> {
    type Error = ();

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        let value = str::from_utf8(value).map_err(|_| ())?;
        Self::try_from(value)
    }
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
    pub(crate) fn resize(&mut self, len: usize, value: T)
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
        assert!(
            values.len() <= N - self.len,
            "fixed vector capacity exceeded"
        );
        for value in values {
            self.push(*value);
        }
    }

    #[inline]
    pub fn try_extend_from_slice(&mut self, values: &[T]) -> Result<(), ()>
    where
        T: Clone,
    {
        if values.len() > N - self.len {
            return Err(());
        }
        for value in values {
            self.data[self.len].write(value.clone());
            self.len += 1;
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

impl_slice_index!(
    Range<usize>,
    RangeFrom<usize>,
    RangeFull,
    RangeInclusive<usize>,
    RangeTo<usize>,
    RangeToInclusive<usize>
);

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

// Wincode's variable-length sequence implementation is allocation-backed.
// Generated on-chain bindings use this implementation with a capacity chosen
// by the concrete call site, while preserving Borsh's u32 length prefix.
unsafe impl<T, const N: usize, C> wincode::SchemaWrite<C> for FixedVec<T, N>
where
    C: wincode::config::Config,
    T: wincode::SchemaWrite<C, Src = T>,
{
    type Src = Self;

    fn size_of(value: &Self::Src) -> wincode::WriteResult<usize> {
        let length = C::LengthEncoding::write_bytes_needed(value.len())?;
        value
            .iter()
            .map(T::size_of)
            .try_fold(length, |total, size| {
                size.and_then(|size| {
                    total.checked_add(size).ok_or(wincode::WriteError::Custom(
                        "fixed vector serialized size overflow",
                    ))
                })
            })
    }

    fn write(mut writer: impl wincode::io::Writer, value: &Self::Src) -> wincode::WriteResult<()> {
        C::LengthEncoding::write(writer.by_ref(), value.len())?;
        for item in value {
            T::write(writer.by_ref(), item)?;
        }
        Ok(())
    }
}

unsafe impl<'de, T, const N: usize, C> wincode::SchemaRead<'de, C> for FixedVec<T, N>
where
    C: wincode::config::Config,
    T: wincode::SchemaRead<'de, C, Dst = T>,
{
    type Dst = Self;

    fn read(
        mut reader: impl wincode::io::Reader<'de>,
        dst: &mut MaybeUninit<Self::Dst>,
    ) -> wincode::ReadResult<()> {
        let length = C::LengthEncoding::read(reader.by_ref())?;
        if length > N {
            return Err(wincode::ReadError::Custom("fixed vector capacity exceeded"));
        }

        let mut value = Self::new();
        for _ in 0..length {
            value
                .try_push(T::get(reader.by_ref())?)
                .map_err(|_| wincode::ReadError::Custom("fixed vector capacity exceeded"))?;
        }
        dst.write(value);
        Ok(())
    }
}

unsafe impl<const N: usize, C> wincode::SchemaWrite<C> for FixedString<N>
where
    C: wincode::config::Config,
{
    type Src = Self;

    fn size_of(value: &Self::Src) -> wincode::WriteResult<usize> {
        <str as wincode::SchemaWrite<C>>::size_of(value.as_str())
    }

    fn write(writer: impl wincode::io::Writer, value: &Self::Src) -> wincode::WriteResult<()> {
        <str as wincode::SchemaWrite<C>>::write(writer, value.as_str())
    }
}

unsafe impl<'de, const N: usize, C> wincode::SchemaRead<'de, C> for FixedString<N>
where
    C: wincode::config::Config,
{
    type Dst = Self;

    fn read(
        mut reader: impl wincode::io::Reader<'de>,
        dst: &mut MaybeUninit<Self::Dst>,
    ) -> wincode::ReadResult<()> {
        let length = C::LengthEncoding::read(reader.by_ref())?;
        if length > N {
            return Err(wincode::ReadError::Custom("fixed string capacity exceeded"));
        }

        let mut bytes = FixedVec::new();
        for _ in 0..length {
            bytes
                .try_push(reader.take_byte()?)
                .map_err(|_| wincode::ReadError::Custom("fixed string capacity exceeded"))?;
        }
        str::from_utf8(bytes.as_slice()).map_err(wincode::ReadError::InvalidUtf8Encoding)?;
        dst.write(Self(bytes));
        Ok(())
    }
}
