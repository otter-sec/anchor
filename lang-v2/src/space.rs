//! Serialized sizes resolved through Rust traits rather than type spelling.

use {
    alloc::{boxed::Box, string::String, vec::Vec},
    core::marker::PhantomData,
};

/// Compile-time maximum Borsh size, excluding the account discriminator.
/// Derive with `InitSpace`; variable-length fields require `#[max_len(...)]`.
#[diagnostic::on_unimplemented(
    message = "`{Self}` has no bounded serialized size",
    note = "derive InitSpace for fixed-size types; String and Vec fields need #[max_len(...)]"
)]
pub trait Space {
    const INIT_SPACE: usize;
}

/// End of a field's `max_len` capacity list.
#[doc(hidden)]
pub struct NoLimits;
/// One capacity, followed by capacities for nested variable-length values.
#[doc(hidden)]
pub struct Limits<const N: usize, Tail = NoLimits>(PhantomData<Tail>);

#[doc(hidden)]
pub trait SpaceLimits {
    const IS_EMPTY: bool;
}
impl SpaceLimits for NoLimits {
    const IS_EMPTY: bool = true;
}
impl<const N: usize, Tail: SpaceLimits> SpaceLimits for Limits<N, Tail> {
    const IS_EMPTY: bool = false;
}

/// Maximum serialized size while consuming capacities in depth-first order.
/// Custom collections can implement this to participate in `InitSpace`.
#[doc(hidden)]
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot compute a size with these max_len capacities",
    note = "provide one #[max_len(...)] capacity per String or Vec layer"
)]
pub trait BoundedSpace<L: SpaceLimits> {
    const SPACE: usize;
    type Remaining: SpaceLimits;
}

impl<T: Space> BoundedSpace<NoLimits> for T {
    const SPACE: usize = T::INIT_SPACE;
    type Remaining = NoLimits;
}

macro_rules! primitive_space {
    ($($ty:ty => $size:expr),* $(,)?) => { $(
        impl Space for $ty { const INIT_SPACE: usize = $size; }
        impl<const N: usize, Tail: SpaceLimits> BoundedSpace<Limits<N, Tail>> for $ty {
            const SPACE: usize = $size;
            type Remaining = Limits<N, Tail>;
        }
    )* };
}
primitive_space! {
    bool => 1, i8 => 1, u8 => 1, i16 => 2, u16 => 2,
    i32 => 4, u32 => 4, f32 => 4, i64 => 8, u64 => 8, f64 => 8,
    i128 => 16, u128 => 16, crate::Address => 32, () => 0,
}

impl<T: Space, const N: usize> Space for [T; N] {
    const INIT_SPACE: usize = N * T::INIT_SPACE;
}
impl<T: Space> Space for Option<T> {
    const INIT_SPACE: usize = 1 + T::INIT_SPACE;
}

impl<const N: usize, Tail: SpaceLimits> BoundedSpace<Limits<N, Tail>> for String {
    const SPACE: usize = 4 + N;
    type Remaining = Tail;
}
impl<T: BoundedSpace<Tail>, const N: usize, Tail: SpaceLimits> BoundedSpace<Limits<N, Tail>>
    for Vec<T>
{
    const SPACE: usize = 4 + N * T::SPACE;
    type Remaining = T::Remaining;
}
impl<T: BoundedSpace<Limits<N, Tail>>, const N: usize, Tail: SpaceLimits>
    BoundedSpace<Limits<N, Tail>> for Option<T>
{
    const SPACE: usize = 1 + T::SPACE;
    type Remaining = T::Remaining;
}
impl<T: BoundedSpace<Limits<N, Tail>>, const N: usize, Tail: SpaceLimits>
    BoundedSpace<Limits<N, Tail>> for Box<T>
{
    const SPACE: usize = T::SPACE;
    type Remaining = T::Remaining;
}
impl<T: BoundedSpace<Limits<N, Tail>>, const N: usize, Tail: SpaceLimits, const COUNT: usize>
    BoundedSpace<Limits<N, Tail>> for [T; COUNT]
{
    const SPACE: usize = COUNT * T::SPACE;
    type Remaining = T::Remaining;
}

macro_rules! tuple_space {
    ($($ty:ident),+) => {
        impl<$($ty: Space),+> Space for ($($ty,)+) { const INIT_SPACE: usize = 0 $(+ $ty::INIT_SPACE)+; }
        tuple_space!(@bounded [Limits<N, Tail>] [] [] [] $($ty),+);
    };
    (@bounded [$limits:ty] [$($bounds:tt)*] [$($sizes:tt)*] [$($all:ident,)*] $head:ident $(, $tail:ident)*) => {
        tuple_space!(@bounded [<$head as BoundedSpace<$limits>>::Remaining]
            [$($bounds)* $head: BoundedSpace<$limits>,]
            [$($sizes)* + <$head as BoundedSpace<$limits>>::SPACE]
            [$($all,)* $head,] $($tail),*);
    };
    (@bounded [$remaining:ty] [$($bounds:tt)*] [$($sizes:tt)*] [$($all:ident,)*]) => {
        impl<const N: usize, Tail: SpaceLimits, $($all,)*> BoundedSpace<Limits<N, Tail>> for ($($all,)*)
        where $($bounds)* {
            const SPACE: usize = 0 $($sizes)*;
            type Remaining = $remaining;
        }
    };
}
tuple_space!(A);
tuple_space!(A, B);
tuple_space!(A, B, C);
tuple_space!(A, B, C, D);
tuple_space!(A, B, C, D, E);
tuple_space!(A, B, C, D, E, F);
tuple_space!(A, B, C, D, E, F, G);
tuple_space!(A, B, C, D, E, F, G, H);
tuple_space!(A, B, C, D, E, F, G, H, I);
tuple_space!(A, B, C, D, E, F, G, H, I, J);
tuple_space!(A, B, C, D, E, F, G, H, I, J, K);
tuple_space!(A, B, C, D, E, F, G, H, I, J, K, L);
