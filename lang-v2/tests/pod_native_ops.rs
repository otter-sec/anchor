use {
    anchor_lang::pod::{PodI64, PodU64},
    core::fmt::Debug,
    std::panic::{self, UnwindSafe},
};

fn assert_matches_native_operator<T: Debug + PartialEq>(
    native: impl FnOnce() -> T + UnwindSafe,
    pod: impl FnOnce() -> T + UnwindSafe,
) {
    let native = panic::catch_unwind(native);
    let pod = panic::catch_unwind(pod);
    assert_eq!(
        pod.is_ok(),
        native.is_ok(),
        "Pod operator panic behavior should match native operator behavior",
    );
    if let (Ok(pod), Ok(native)) = (pod, native) {
        assert_eq!(pod, native);
    }
}

#[test]
fn overflowing_pod_operators_match_native_behavior() {
    let max_u64 = u64::MAX;
    let one_u64 = 1u64;
    assert_matches_native_operator(
        || max_u64 + one_u64,
        || (PodU64::from(max_u64) + one_u64).get(),
    );

    let zero_u64 = 0u64;
    assert_matches_native_operator(
        || zero_u64 - one_u64,
        || (PodU64::from(zero_u64) - one_u64).get(),
    );

    let half_u64 = (u64::MAX / 2) + 1;
    assert_matches_native_operator(|| half_u64 * 2, || (PodU64::from(half_u64) * 2u64).get());

    let min_i64 = i64::MIN;
    assert_matches_native_operator(|| -min_i64, || (-PodI64::from(min_i64)).get());
}
