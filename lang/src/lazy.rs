use crate::{AnchorDeserialize, Pubkey};

/// A helper trait to make lazy deserialization work.
///
/// Currently this is only implemented for [`borsh`], as it's not necessary for zero copy via
/// [`bytemuck`]. However, the functionality can be extended when we support custom serialization
/// in the future.
///
/// # Note
///
/// You should avoid implementing this trait manually.
///
/// It's currently implemented automatically if you derive [`AnchorDeserialize`]:
///
/// ```ignore
/// #[derive(AnchorDeserialize)]
/// pub struct MyStruct {
///     field: u8,
/// }
/// ```
pub trait Lazy: AnchorDeserialize {
    /// Whether the type is a fixed-size type.
    const SIZED: bool = false;

    /// Get the serialized size of the type from the given buffer.
    ///
    /// For performance reasons, this method does not verify the validity of the data, and should
    /// never fail.
    ///
    /// # Panics
    ///
    /// If the given buffer cannot be used to deserialize the data e.g. it's shorter than the
    /// expected data. However, this doesn't mean it will panic **whenever** there is an incorrect
    /// data e.g. passing **any** data for `bool::size_of` works, even when the buffer is empty.
    fn size_of(buf: &[u8]) -> usize;
}

macro_rules! impl_sized {
    ($ty: ty) => {
        impl Lazy for $ty {
            const SIZED: bool = true;

            #[inline(always)]
            fn size_of(_buf: &[u8]) -> usize {
                ::core::mem::size_of::<$ty>()
            }
        }
    };
}

impl_sized!(bool);
impl_sized!(u8);
impl_sized!(u16);
impl_sized!(u32);
impl_sized!(u64);
impl_sized!(u128);
impl_sized!(i8);
impl_sized!(i16);
impl_sized!(i32);
impl_sized!(i64);
impl_sized!(i128);
impl_sized!(f32);
impl_sized!(f64);
impl_sized!(Pubkey);

impl<T: Lazy, const N: usize> Lazy for [T; N] {
    const SIZED: bool = N == 0 || T::SIZED;

    #[inline(always)]
    fn size_of(buf: &[u8]) -> usize {
        if Self::SIZED {
            N * T::size_of(buf)
        } else {
            (0..N).fold(0, |acc, _| acc + T::size_of(&buf[acc..]))
        }
    }
}

impl Lazy for String {
    const SIZED: bool = false;

    #[inline(always)]
    fn size_of(buf: &[u8]) -> usize {
        LEN + get_len(buf)
    }
}

impl<T: Lazy> Lazy for Option<T> {
    const SIZED: bool = false;

    #[inline(always)]
    fn size_of(buf: &[u8]) -> usize {
        1 + match buf.first() {
            Some(0) => 0,
            Some(1) => T::size_of(&buf[1..]),
            _ => unreachable!(),
        }
    }
}

impl<T: Lazy + Clone> Lazy for Box<T> {
    const SIZED: bool = T::SIZED;

    #[inline(always)]
    fn size_of(buf: &[u8]) -> usize {
        T::size_of(buf)
    }
}

impl<T: Lazy> Lazy for Vec<T> {
    const SIZED: bool = false;

    #[inline(always)]
    fn size_of(buf: &[u8]) -> usize {
        (0..get_len(buf)).fold(LEN, |acc, _| acc + T::size_of(&buf[acc..]))
    }
}

/// `borsh` length identifier of unsized types.
const LEN: usize = 4;

#[inline(always)]
fn get_len(buf: &[u8]) -> usize {
    u32::from_le_bytes((buf[..LEN].try_into()).unwrap())
        .try_into()
        .unwrap()
}

#[cfg(test)]
mod tests {
    use {super::*, crate::AnchorSerialize};

    macro_rules! len {
        ($val: expr) => {
            borsh::to_vec(&$val).unwrap().len()
        };
    }

    #[test]
    fn sized() {
        // Sized inputs don't care about the passed data
        const EMPTY: &[u8] = &[];
        assert_eq!(bool::size_of(EMPTY), len!(true));
        assert_eq!(u8::size_of(EMPTY), len!(0u8));
        assert_eq!(u16::size_of(EMPTY), len!(0u16));
        assert_eq!(u32::size_of(EMPTY), len!(0u32));
        assert_eq!(u64::size_of(EMPTY), len!(0u64));
        assert_eq!(u128::size_of(EMPTY), len!(0u128));
        assert_eq!(i8::size_of(EMPTY), len!(0i8));
        assert_eq!(i16::size_of(EMPTY), len!(0i16));
        assert_eq!(i32::size_of(EMPTY), len!(0i32));
        assert_eq!(i64::size_of(EMPTY), len!(0i64));
        assert_eq!(i128::size_of(EMPTY), len!(0i128));
        assert_eq!(f32::size_of(EMPTY), len!(0f32));
        assert_eq!(f64::size_of(EMPTY), len!(0f64));
        assert_eq!(Pubkey::size_of(EMPTY), len!(Pubkey::default()));
        assert_eq!(<[i32; 4]>::size_of(EMPTY), len!([0i32; 4]));
    }

    #[test]
    fn r#unsized() {
        assert_eq!(String::size_of(&[1, 0, 0, 0, 65]), len!(String::from("a")));
        assert_eq!(<Option<u8>>::size_of(&[0]), len!(Option::<u8>::None));
        assert_eq!(<Option<u8>>::size_of(&[1, 1]), len!(Some(1u8)));
        assert_eq!(<Vec<u8>>::size_of(&[1, 0, 0, 0, 1]), len!(vec![1u8]));
        assert_eq!(
            <Vec<String>>::size_of(&[1, 0, 0, 0, 1, 0, 0, 0, 65]),
            len!(vec![String::from("a")])
        );
        assert_eq!(
            <Vec<String>>::size_of(&[2, 0, 0, 0, 1, 0, 0, 0, 65, 2, 0, 0, 0, 65, 66]),
            len!(vec![String::from("a"), String::from("ab")])
        );
    }

    #[test]
    fn defined() {
        // Struct
        #[derive(AnchorSerialize, AnchorDeserialize)]
        struct MyStruct {
            a: u8,
            b: Vec<u8>,
            c: Option<String>,
        }

        assert_eq!(
            MyStruct::size_of(&[1, 2, 0, 0, 0, 1, 2, 1, 1, 0, 0, 0, 65]),
            len!(MyStruct {
                a: 1,
                b: vec![1u8, 2],
                c: Some(String::from("a"))
            })
        );
        assert!(!MyStruct::SIZED);

        // Enum
        #[derive(AnchorSerialize, AnchorDeserialize)]
        enum MyEnum {
            Unit,
            Named { a: u8 },
            Unnamed(i16, i16),
        }

        assert_eq!(MyEnum::size_of(&[0]), len!(MyEnum::Unit));
        assert_eq!(MyEnum::size_of(&[1, 23]), len!(MyEnum::Named { a: 1 }));
        assert_eq!(
            MyEnum::size_of(&[2, 1, 2, 1, 2]),
            len!(MyEnum::Unnamed(1, 2))
        );
        assert!(!MyEnum::SIZED);

        #[derive(AnchorSerialize, AnchorDeserialize)]
        enum UnitEnum {
            A,
            B,
        }
        assert_eq!(UnitEnum::size_of(&[0]), len!(UnitEnum::A));
        assert_eq!(UnitEnum::size_of(&[1]), len!(UnitEnum::B));
        assert!(UnitEnum::SIZED);
    }

    #[test]
    fn generic() {
        #[derive(AnchorSerialize, AnchorDeserialize)]
        struct GenericStruct<T: Lazy + borsh::BorshSerialize> {
            t: T,
        }

        assert_eq!(
            GenericStruct::<i64>::size_of(&[1, 2, 3, 4, 5, 6, 7, 8]),
            len!(GenericStruct { t: 1i64 })
        );
        assert!(GenericStruct::<i64>::SIZED);

        assert_eq!(
            GenericStruct::<Vec<u8>>::size_of(&[8, 0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8]),
            len!(GenericStruct { t: vec![0u8; 8] })
        );
        assert!(!GenericStruct::<Vec<u8>>::SIZED);
    }

    #[test]
    fn enum_variable() {
        // Test that the size of variably-sized types in an enum is calculated correctly
        #[derive(AnchorSerialize, AnchorDeserialize, Debug, Clone)]
        enum State {
            User(Vec<u8>),
            Other(u8),
        }
        const VEC_LEN: usize = 32;
        let state = State::User(vec![1; VEC_LEN]);

        let state_bytes = borsh::to_vec(&state).unwrap();
        let actual_state_size = state_bytes.len();
        let lazy_state_size = <State as Lazy>::size_of(&state_bytes);
        assert_eq!(actual_state_size, lazy_state_size);
        // enum tag + vec length field + vec contents
        assert_eq!(lazy_state_size, 1 + LEN + VEC_LEN);
    }

    #[test]
    fn array_variable() {
        // Test that the size of an array of variably sized types is calculated correctly
        let messages: [Vec<u8>; 8] = std::array::from_fn(|i| vec![i as u8; i]);
        let serialized = borsh::to_vec(&messages).unwrap();
        let actual_size = serialized.len();
        let lazy_size = <[Vec<u8>; 8] as Lazy>::size_of(&serialized);
        assert_eq!(actual_size, lazy_size);
    }
}
