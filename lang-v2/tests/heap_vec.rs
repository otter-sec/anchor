use {
    anchor_lang::{Error, ErrorCode, TryReserveError, TryReserveErrorKind, Vec},
    core::{alloc::Layout, cell::Cell},
};

#[test]
fn grows_and_preserves_elements_across_failed_reservation() {
    let mut values = Vec::with_capacity(2);
    values.extend_from_slice(&[10, 20]);
    values.push(30);
    assert_eq!(&*values, &[10, 20, 30]);

    let capacity = values.capacity();
    let error = values.try_reserve(usize::MAX).unwrap_err();
    assert_eq!(error.kind(), TryReserveErrorKind::CapacityOverflow);
    assert_eq!(Error::from(error), Error::ArithmeticOverflow);
    assert_eq!(values.capacity(), capacity);
    assert_eq!(values.pop(), Some(30));
    values.clear();
    assert!(values.is_empty());
    assert_eq!(values.capacity(), capacity);
    values.try_resize(2, 99).unwrap();
    assert_eq!(&*values, &[99, 99]);
}

#[test]
fn reserve_errors_distinguish_invalid_layouts_from_allocator_failure() {
    let error = Vec::<u64>::try_with_capacity(usize::MAX).unwrap_err();
    assert_eq!(error.kind(), TryReserveErrorKind::CapacityOverflow);

    let kind = TryReserveErrorKind::AllocError {
        layout: Layout::new::<u64>(),
    };
    let error = TryReserveError::from(kind);
    assert_eq!(error.kind(), kind);
    assert_eq!(
        Error::from(error),
        Error::from(ErrorCode::HeapAllocationFailed)
    );
}

#[test]
fn drops_initialized_values_after_growth_and_handles_zero_sized_elements() {
    struct CountDrop<'a>(&'a Cell<usize>);

    impl Drop for CountDrop<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    let drops = Cell::new(0);
    let mut values = Vec::with_capacity(1);
    values.push(CountDrop(&drops));
    values.push(CountDrop(&drops));
    assert_eq!(drops.get(), 0);
    values.truncate(1);
    assert_eq!(drops.get(), 1);
    values.push(CountDrop(&drops));
    let mut iter = values.into_iter();
    let first = iter.next().unwrap();
    drop(iter);
    assert_eq!(drops.get(), 2);
    drop(first);
    assert_eq!(drops.get(), 3);

    let mut units = Vec::new();
    units.push(());
    units.push(());
    assert_eq!(units.capacity(), usize::MAX);
    assert_eq!(units.pop(), Some(()));
    assert_eq!(units.as_slice(), &[()]);
    assert_eq!(units.into_iter().count(), 1);
    assert_eq!(Vec::<u8>::new().into_iter().count(), 0);
}

#[test]
fn dedup_removes_only_consecutive_duplicates() {
    let mut values = Vec::from([1, 1, 2, 2, 1]);
    values.dedup();
    assert_eq!(values.as_slice(), &[1, 2, 1]);
}

#[test]
fn converts_to_std_vec_without_reallocating() {
    #[repr(align(64))]
    struct CountDrop<'a>(&'a Cell<usize>);

    impl Drop for CountDrop<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    let drops = Cell::new(0);
    let mut values = Vec::with_capacity(4);
    values.push(CountDrop(&drops));
    let pointer = values.as_ptr();
    let mut converted = values.into_vec();
    assert_eq!(converted.as_ptr(), pointer);
    assert_eq!(converted.len(), 1);
    assert_eq!(converted.capacity(), 4);
    assert_eq!(drops.get(), 0);
    converted.reserve_exact(8);
    drop(converted);
    assert_eq!(drops.get(), 1);

    let empty = Vec::<u8>::new().into_vec();
    assert_eq!(empty.capacity(), 0);
    let units: std::vec::Vec<_> = Vec::from([(), ()]).into();
    assert_eq!(units.len(), 2);
    assert_eq!(units.capacity(), usize::MAX);
}
