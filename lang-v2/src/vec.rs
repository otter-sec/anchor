//! A growable buffer backed by the program's global allocator. This reimplements `Vec`
//! using the Pinocchio allocator, allowing `no_alloc` builds.
//! When compiled off-chain, uses the host's global allocator.

use {
    anchor_lang_error::{Error, ErrorCode},
    core::{
        alloc::Layout,
        fmt,
        mem::{size_of, ManuallyDrop, MaybeUninit},
        ops::{Deref, DerefMut, Range},
        ptr::{self, NonNull},
        slice,
    },
    wincode::len::SeqLen as _,
};

#[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
extern crate std;

#[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
mod heap {
    pub use super::std::alloc::{alloc, dealloc, realloc};
}

#[cfg(any(target_os = "solana", target_arch = "bpf"))]
mod heap {
    use core::alloc::Layout;

    // These shims call the same allocator registered by the program entrypoint.
    // Stable core cannot look up the global allocator, so the entrypoint exports
    // the shims without relying on rustc's private allocator symbols.
    unsafe extern "Rust" {
        #[link_name = "__anchor_heap_alloc"]
        pub fn alloc(layout: Layout) -> *mut u8;
        #[link_name = "__anchor_heap_dealloc"]
        pub fn dealloc(pointer: *mut u8, layout: Layout);
        #[link_name = "__anchor_heap_realloc"]
        pub fn realloc(pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8;
    }
}

/// Register Anchor's default global allocator and its growable-buffer shims.
/// Programs using a custom entrypoint can install this in place of Pinocchio's
/// `default_allocator!`.
#[macro_export]
macro_rules! default_allocator {
    () => {
        #[cfg(any(target_os = "solana", target_arch = "bpf"))]
        mod __anchor_global_allocator {
            use {
                core::alloc::{GlobalAlloc, Layout},
                $crate::pinocchio::entrypoint::{
                    BumpAllocator, HEAP_START_ADDRESS, MAX_HEAP_LENGTH,
                },
            };

            #[global_allocator]
            static ALLOCATOR: BumpAllocator = unsafe {
                BumpAllocator::new_unchecked(HEAP_START_ADDRESS as usize, MAX_HEAP_LENGTH as usize)
            };

            #[unsafe(no_mangle)]
            unsafe extern "Rust" fn __anchor_heap_alloc(layout: Layout) -> *mut u8 {
                unsafe { ALLOCATOR.alloc(layout) }
            }

            #[unsafe(no_mangle)]
            unsafe extern "Rust" fn __anchor_heap_dealloc(pointer: *mut u8, layout: Layout) {
                unsafe { ALLOCATOR.dealloc(pointer, layout) }
            }

            #[unsafe(no_mangle)]
            unsafe extern "Rust" fn __anchor_heap_realloc(
                pointer: *mut u8,
                layout: Layout,
                new_size: usize,
            ) -> *mut u8 {
                unsafe { ALLOCATOR.realloc(pointer, layout, new_size) }
            }
        }
    };
}

/// The requested vector capacity overflowed or the allocator denied it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TryReserveError {
    kind: TryReserveErrorKind,
}

impl TryReserveError {
    pub const fn kind(&self) -> TryReserveErrorKind {
        self.kind
    }
}

/// The reason a vector could not reserve capacity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TryReserveErrorKind {
    /// The requested capacity exceeds the vector's maximum.
    CapacityOverflow,
    /// The allocator could not satisfy the request.
    AllocError {
        /// The layout of the requested allocation.
        layout: Layout,
    },
}

impl From<TryReserveErrorKind> for TryReserveError {
    fn from(kind: TryReserveErrorKind) -> Self {
        Self { kind }
    }
}

impl From<TryReserveError> for Error {
    fn from(error: TryReserveError) -> Self {
        match error.kind() {
            TryReserveErrorKind::CapacityOverflow => Self::ArithmeticOverflow,
            TryReserveErrorKind::AllocError { .. } => ErrorCode::HeapAllocationFailed.into(),
        }
    }
}

struct Allocation<T> {
    ptr: NonNull<T>,
    cap: usize,
}

impl<T> Drop for Allocation<T> {
    fn drop(&mut self) {
        if size_of::<T>() != 0 && self.cap != 0 {
            // SAFETY: nonempty allocations always have a valid layout for cap.
            let layout = Layout::array::<T>(self.cap).unwrap();
            unsafe { heap::dealloc(self.ptr.as_ptr().cast(), layout) };
        }
    }
}

/// A contiguous growable array with heap-backed elements. Capacity is measured
/// in elements and grows independently of the type of the vector.
/// On SBF, growth uses the program's allocator and follows its reclamation rules.
pub struct Vec<T> {
    allocation: Allocation<T>,
    len: usize,
}

// SAFETY: Vec owns its allocation and exposes elements through Rust borrows.
unsafe impl<T: Send> Send for Vec<T> {}
unsafe impl<T: Sync> Sync for Vec<T> {}

impl<T> Vec<T> {
    pub const fn new() -> Self {
        Self {
            allocation: Allocation {
                ptr: NonNull::dangling(),
                cap: if size_of::<T>() == 0 { usize::MAX } else { 0 },
            },
            len: 0,
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self::try_with_capacity(capacity).expect("vector allocation failed")
    }

    pub fn try_with_capacity(capacity: usize) -> Result<Self, TryReserveError> {
        let mut value = Self::new();
        if capacity > value.capacity() {
            value.grow(capacity)?;
        }
        Ok(value)
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub const fn capacity(&self) -> usize {
        self.allocation.cap
    }

    /// Converts to a standard vector without allocating or moving elements.
    #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
    pub fn into_vec(self) -> std::vec::Vec<T> {
        let value = ManuallyDrop::new(self);
        // SAFETY: Both vectors use the global allocator and Layout::array::<T>(cap).
        // The pointer is aligned even without an allocation, and the first len
        // elements are initialized. ManuallyDrop transfers ownership to std.
        unsafe {
            std::vec::Vec::from_raw_parts(
                value.allocation.ptr.as_ptr(),
                value.len,
                value.capacity(),
            )
        }
    }

    pub fn try_reserve(&mut self, additional: usize) -> Result<(), TryReserveError> {
        let required = self
            .len
            .checked_add(additional)
            .ok_or(TryReserveErrorKind::CapacityOverflow)?;
        if required <= self.capacity() {
            return Ok(());
        }

        let max_capacity = (isize::MAX as usize) / size_of::<T>();
        let doubled = self.capacity().saturating_mul(2).min(max_capacity);
        self.grow(required.max(doubled))
    }

    fn grow(&mut self, capacity: usize) -> Result<(), TryReserveError> {
        let layout =
            Layout::array::<T>(capacity).map_err(|_| TryReserveErrorKind::CapacityOverflow)?;
        // SAFETY: grow only runs for a nonzero layout. The old pointer and
        // capacity still describe the existing allocation if realloc fails.
        let pointer = unsafe {
            if self.capacity() == 0 {
                heap::alloc(layout)
            } else {
                let old_layout = Layout::array::<T>(self.capacity()).unwrap();
                heap::realloc(
                    self.allocation.ptr.as_ptr().cast(),
                    old_layout,
                    layout.size(),
                )
            }
        };
        self.allocation.ptr =
            NonNull::new(pointer.cast()).ok_or(TryReserveErrorKind::AllocError { layout })?;
        self.allocation.cap = capacity;
        Ok(())
    }

    pub fn push(&mut self, value: T) {
        assert!(self.try_push(value).is_ok(), "vector allocation failed");
    }

    pub fn try_push(&mut self, value: T) -> Result<(), T> {
        if self.try_reserve(1).is_err() {
            return Err(value);
        }
        // SAFETY: the slot at len is within capacity and uninitialized.
        unsafe { self.allocation.ptr.as_ptr().add(self.len).write(value) };
        self.len += 1;
        Ok(())
    }

    pub fn pop(&mut self) -> Option<T> {
        self.len = self.len.checked_sub(1)?;
        // SAFETY: the slot at the new len was initialized and is now removed.
        Some(unsafe { self.allocation.ptr.as_ptr().add(self.len).read() })
    }

    pub fn truncate(&mut self, len: usize) {
        if len < self.len {
            let removed = self.len - len;
            self.len = len;
            // SAFETY: the removed elements were initialized and are excluded
            // from len before dropping them, including if a destructor panics.
            unsafe {
                ptr::drop_in_place(ptr::slice_from_raw_parts_mut(
                    self.allocation.ptr.as_ptr().add(len),
                    removed,
                ));
            }
        }
    }

    pub fn clear(&mut self) {
        self.truncate(0);
    }

    pub fn as_slice(&self) -> &[T] {
        // SAFETY: the first len elements are initialized.
        unsafe { slice::from_raw_parts(self.allocation.ptr.as_ptr(), self.len) }
    }

    pub fn as_mut_slice(&mut self) -> &mut [T] {
        // SAFETY: the first len elements are initialized and exclusively borrowed.
        unsafe { slice::from_raw_parts_mut(self.allocation.ptr.as_ptr(), self.len) }
    }
}

impl<T: PartialEq> Vec<T> {
    pub fn dedup(&mut self) {
        let mut kept = self.len.min(1);
        for index in 1..self.len {
            if self[index] != self[kept - 1] {
                self.swap(kept, index);
                kept += 1;
            }
        }
        self.truncate(kept);
    }
}

impl<T: Clone> Vec<T> {
    pub fn extend_from_slice(&mut self, values: &[T]) {
        self.try_extend_from_slice(values)
            .expect("vector allocation failed");
    }

    pub fn try_extend_from_slice(&mut self, values: &[T]) -> Result<(), TryReserveError> {
        self.try_reserve(values.len())?;
        for value in values {
            self.push(value.clone());
        }
        Ok(())
    }

    pub fn try_resize(&mut self, len: usize, value: T) -> Result<(), TryReserveError> {
        self.truncate(len);
        let additional = len - self.len;
        if additional != 0 {
            self.try_reserve(additional)?;
            for _ in 1..additional {
                self.push(value.clone());
            }
            self.push(value);
        }
        Ok(())
    }
}

impl Vec<u8> {
    pub fn as_bytes(&self) -> &[u8] {
        self.as_slice()
    }
}

impl<T> Default for Vec<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Drop for Vec<T> {
    fn drop(&mut self) {
        self.clear();
    }
}

impl<T: Clone> Clone for Vec<T> {
    fn clone(&self) -> Self {
        let mut copy = Self::with_capacity(self.len);
        copy.extend_from_slice(self);
        copy
    }
}

impl<T: fmt::Debug> fmt::Debug for Vec<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_slice().fmt(formatter)
    }
}

impl<T: PartialEq> PartialEq for Vec<T> {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl<T: Eq> Eq for Vec<T> {}

impl<T> Deref for Vec<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<T> DerefMut for Vec<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut_slice()
    }
}

impl<T> AsRef<[T]> for Vec<T> {
    fn as_ref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T> Extend<T> for Vec<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        let iter = iter.into_iter();
        self.try_reserve(iter.size_hint().0)
            .expect("vector allocation failed");
        for item in iter {
            self.push(item);
        }
    }
}

impl<T> FromIterator<T> for Vec<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut result = Self::new();
        result.extend(iter);
        result
    }
}

impl<T, const N: usize> From<[T; N]> for Vec<T> {
    fn from(values: [T; N]) -> Self {
        values.into_iter().collect()
    }
}

impl<T: Clone> From<&[T]> for Vec<T> {
    fn from(values: &[T]) -> Self {
        let mut result = Self::with_capacity(values.len());
        result.extend_from_slice(values);
        result
    }
}

#[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
impl<T> From<Vec<T>> for std::vec::Vec<T> {
    fn from(value: Vec<T>) -> Self {
        value.into_vec()
    }
}

pub struct IntoIter<T> {
    allocation: Allocation<T>,
    remaining: Range<usize>,
}

impl<T> Iterator for IntoIter<T> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        let index = self.remaining.next()?;
        // SAFETY: the range only includes initialized elements not yet moved out.
        Some(unsafe { self.allocation.ptr.as_ptr().add(index).read() })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.remaining.size_hint()
    }
}

impl<T> ExactSizeIterator for IntoIter<T> {}

impl<T> Drop for IntoIter<T> {
    fn drop(&mut self) {
        // SAFETY: these are exactly the initialized elements still owned by the iterator.
        unsafe {
            let start = self.allocation.ptr.as_ptr().add(self.remaining.start);
            ptr::drop_in_place(ptr::slice_from_raw_parts_mut(start, self.remaining.len()));
        }
    }
}

impl<T> IntoIterator for Vec<T> {
    type Item = T;
    type IntoIter = IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        let value = ManuallyDrop::new(self);
        IntoIter {
            // SAFETY: ManuallyDrop prevents Vec from freeing this allocation.
            allocation: unsafe { ptr::read(&value.allocation) },
            remaining: 0..value.len,
        }
    }
}

impl<'a, T> IntoIterator for &'a Vec<T> {
    type Item = &'a T;
    type IntoIter = slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, T> IntoIterator for &'a mut Vec<T> {
    type Item = &'a mut T;
    type IntoIter = slice::IterMut<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl wincode::io::Writer for Vec<u8> {
    fn write(&mut self, src: &[u8]) -> wincode::io::WriteResult<()> {
        self.try_extend_from_slice(src)
            .map_err(|_| wincode::io::WriteError::WriteSizeLimit(src.len()))
    }
}

// Preserve the configured sequence encoding used by Rust's Vec.
unsafe impl<T, C> wincode::SchemaWrite<C> for Vec<T>
where
    C: wincode::config::Config,
    T: wincode::SchemaWrite<C, Src = T>,
{
    type Src = Self;

    fn size_of(value: &Self::Src) -> wincode::WriteResult<usize> {
        let length = C::LengthEncoding::write_bytes_needed_prealloc_check::<T>(value.len())?;
        value
            .iter()
            .map(T::size_of)
            .try_fold(length, |total, size| {
                size.and_then(|size| {
                    total.checked_add(size).ok_or(wincode::WriteError::Custom(
                        "vector serialized size overflow",
                    ))
                })
            })
    }

    fn write(mut writer: impl wincode::io::Writer, value: &Self::Src) -> wincode::WriteResult<()> {
        C::LengthEncoding::prealloc_check::<T>(value.len())?;
        C::LengthEncoding::write(writer.by_ref(), value.len())?;
        for item in value {
            T::write(writer.by_ref(), item)?;
        }
        Ok(())
    }
}

unsafe impl<'de, T, C> wincode::SchemaRead<'de, C> for Vec<T>
where
    C: wincode::config::Config,
    T: wincode::SchemaRead<'de, C, Dst = T>,
{
    type Dst = Self;

    fn read(
        mut reader: impl wincode::io::Reader<'de>,
        dst: &mut MaybeUninit<Self::Dst>,
    ) -> wincode::ReadResult<()> {
        let length = C::LengthEncoding::read_prealloc_check::<T>(reader.by_ref())?;
        let mut value = Self::try_with_capacity(length)
            .map_err(|_| wincode::ReadError::Custom("vector allocation failed"))?;
        for _ in 0..length {
            value.push(T::get(reader.by_ref())?);
        }
        dst.write(value);
        Ok(())
    }
}
