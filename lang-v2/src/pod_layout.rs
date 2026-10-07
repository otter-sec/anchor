//! Type-level layout checks for zero-copy fields.

/// A POD field whose additional layout invariants can be checked at compile time.
/// The `Pod` bound supplies the byte-layout guarantee; `CHECK` can enforce
/// additional restrictions such as a bounded vector's length-prefix capacity.
#[doc(hidden)]
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a supported zero-copy field",
    note = "zero-copy fields require bytemuck::Pod and PodLayout; use fixed arrays or POD \
            wrappers for dynamic or restricted values"
)]
pub trait PodLayout: bytemuck::Pod {
    const CHECK: () = ();
}

macro_rules! pod_layout {
    ($($ty:ty),* $(,)?) => { $( impl PodLayout for $ty {} )* };
}
pod_layout!(
    (),
    u8,
    u16,
    u32,
    u64,
    u128,
    i8,
    i16,
    i32,
    i64,
    i128,
    f32,
    f64,
    crate::Address,
    crate::pod::PodBool,
    crate::pod::PodU16,
    crate::pod::PodU32,
    crate::pod::PodU64,
    crate::pod::PodU128,
    crate::pod::PodI16,
    crate::pod::PodI32,
    crate::pod::PodI64,
    crate::pod::PodI128,
);
impl<T: PodLayout, const N: usize> PodLayout for [T; N]
where
    [T; N]: bytemuck::Pod,
{
    const CHECK: () = T::CHECK;
}
