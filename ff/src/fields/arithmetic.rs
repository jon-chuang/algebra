/// This modular multiplication algorithm uses Montgomery
/// reduction for efficient implementation. It also additionally
/// uses the "no-carry optimization" outlined
/// [here](https://hackmd.io/@zkteam/modular_multiplication) if
/// `P::MODULUS` has (a) a non-zero MSB, and (b) at least one
/// zero bit in the rest of the modulus.
macro_rules! impl_field_mul_assign {
    ($limbs:expr) => {
        paste::paste! {
            #[inline(always)]
            #[ark_ff_asm::unroll_for_loops]
            fn [<mul_assign _id $limbs>]<P: FpParams<N>, const N: usize>(
                input: &mut [u64; N],
                other: [u64; N],
            ) {
                let mut r = [0u64; $limbs];
                let mut carry1 = 0u64;
                let mut carry2 = 0u64;

                for i in 0..$limbs {
                    r[0] = fa::mac(r[0], input[0], other[i], &mut carry1);
                    let k = r[0].wrapping_mul(P::INV);
                    fa::mac_discard(r[0], k, P::MODULUS.0[0], &mut carry2);
                    for j in 1..$limbs {
                        r[j] = mac_with_carry!(r[j], input[j], other[i], &mut carry1);
                        r[j - 1] = mac_with_carry!(r[j], k, P::MODULUS.0[j], &mut carry2);
                    }
                    r[$limbs - 1] = carry1 + carry2;
                }
                input.copy_from_slice(&r[..]);
            }
        }
    };
}

macro_rules! impl_field_into_repr {
    ($limbs:expr) => {
        paste::paste! {
            #[inline]
            #[ark_ff_asm::unroll_for_loops]
            fn [<into_repr _id $limbs>]<P: FpParams<N>, const N: usize>(
                input: [u64; N]
            ) -> BigInt<N> {
                let mut r = input;
                // Montgomery Reduction
                for i in 0..$limbs {
                    let k = r[i].wrapping_mul(P::INV);
                    let mut carry = 0;

                    mac_with_carry!(r[i], k, P::MODULUS.0[0], &mut carry);
                    for j in 1..$limbs {
                        r[(j + i) % $limbs] =
                            mac_with_carry!(r[(j + i) % $limbs], k, P::MODULUS.0[j], &mut carry);
                    }
                    r[i % $limbs] = carry;
                }
                BigInt::<N>(r)
            }
        }
    };
}
// macro_rules! impl_deserialize_flags {
macro_rules! impl_serialize_flags {
    ($limbs: expr) => {
        paste::paste! {
            #[inline(always)]
            fn [<serialize _id $limbs>]<P: FpParams<N>, W: ark_std::io::Write, F: Flags, const N: usize>(
                input: &Fp<P, N>,
                mut writer: W,
                flags: F,
            ) -> Result<(), SerializationError> {
                // Calculate the number of bytes required to represent a field element
                // serialized with `flags`. If `F::BIT_SIZE < 8`,
                // this is at most `$byte_size + 1`
                let output_byte_size = buffer_byte_size(P::MODULUS_BITS as usize + F::BIT_SIZE);

                // Write out `self` to a temporary buffer.
                // The size of the buffer is $byte_size + 1 because `F::BIT_SIZE`
                // is at most 8 bits.
                let mut bytes = [0u8; $limbs * 8 + 1];
                input.write(&mut bytes[..$limbs * 8])?;

                // Mask out the bits of the last byte that correspond to the flag.
                bytes[output_byte_size - 1] |= flags.u8_bitmask();

                writer.write_all(&bytes[..output_byte_size])?;
                Ok(())
            }
        }
    }
}

// macro_rules! impl_deserialize_flags {
macro_rules! impl_deserialize_flags {
    ($limbs: expr) => {
        paste::paste! {
            fn [<deserialize _id $limbs>]<P: FpParams<N>, R: ark_std::io::Read, F: Flags, const N: usize>(
                mut reader: R,
            ) -> Result<(Fp<P, N>, F), SerializationError> {
                // All reasonable `Flags` should be less than 8 bits in size
                // (256 values are enough for anyone!)
                if F::BIT_SIZE > 8 {
                    return Err(SerializationError::NotEnoughSpace);
                }
                // Calculate the number of bytes required to represent a field element
                // serialized with `flags`. If `F::BIT_SIZE < 8`,
                // this is at most `$byte_size + 1`
                let output_byte_size = buffer_byte_size(P::MODULUS_BITS as usize + F::BIT_SIZE);

                let mut masked_bytes = [0u8; $limbs * 8 + 1];
                reader.read_exact(&mut masked_bytes[..output_byte_size])?;

                let flags = F::from_u8_remove_flags(&mut masked_bytes[output_byte_size - 1])
                    .ok_or(SerializationError::UnexpectedFlags)?;

                Ok((<Fp<P, N>>::read(&masked_bytes[..])?, flags))
            }
        }
    }
}

macro_rules! impl_field_square_in_place {
    ($limbs: expr) => {
        paste::paste! {
            #[inline(always)]
            #[ark_ff_asm::unroll_for_loops]
            #[allow(unused_braces, clippy::absurd_extreme_comparisons)]
            fn [<square_in_place _id $limbs>]<P: FpParams<N>, const N: usize>(
                input: &mut [u64; N],
            ) {
                let mut r = [0u64; $limbs * 2];
                let mut carry = 0;
                for i in 0..$limbs {
                    if i < $limbs - 1 {
                        for j in 0..$limbs {
                            if j > i {
                                r[i + j] =
                                    mac_with_carry!(r[i + j], input[i], input[j], &mut carry);
                            }
                        }
                        r[$limbs + i] = carry;
                        carry = 0;
                    }
                }
                r[$limbs * 2 - 1] = r[$limbs * 2 - 2] >> 63;
                for i in 0..$limbs {
                    // This computes `r[2 * ($limbs - 1) - (i + 1)]`, but additionally
                    // handles the case where the index underflows.
                    // Note that we should never hit this case because it only occurs
                    // when `$limbs == 1`, but we handle that separately above.
                    let subtractor = (2 * ($limbs - 1usize))
                        .checked_sub(i + 1)
                        .map(|index| r[index])
                        .unwrap_or(0);
                    r[2 * ($limbs - 1) - i] = (r[2 * ($limbs - 1) - i] << 1) | (subtractor >> 63);
                }
                for i in 3..$limbs {
                    r[$limbs + 1 - i] = (r[$limbs + 1 - i] << 1) | (r[$limbs - i] >> 63);
                }
                r[1] <<= 1;

                for i in 0..$limbs {
                    r[2 * i] = mac_with_carry!(r[2 * i], input[i], input[i], &mut carry);
                    // need unused assignment because the last iteration of the loop produces an
                    // assignment to `carry` that is unused.
                    #[allow(unused_assignments)]
                    {
                        r[2 * i + 1] = adc!(r[2 * i + 1], 0, &mut carry);
                    }
                }
                // Montgomery reduction
                let mut _carry2 = 0;
                for i in 0..$limbs {
                    let k = r[i].wrapping_mul(P::INV);
                    let mut carry = 0;
                    mac_with_carry!(r[i], k, P::MODULUS.0[0], &mut carry);
                    for j in 1..$limbs {
                        r[j + i] = mac_with_carry!(r[j + i], k, P::MODULUS.0[j], &mut carry);
                    }
                    r[$limbs + i] = adc!(r[$limbs + i], _carry2, &mut carry);
                    _carry2 = carry;
                }
                input.copy_from_slice(&r[N..]);
            }
        }
    };
}

macro_rules! impl_prime_field_from_int {
    ($int: ident) => {
        impl<P: FpParams<N>, const N: usize> From<$int> for Fp<P, N> {
            fn from(other: $int) -> Self {
                if N == 1 {
                    Self::from_repr(P::BigInt::from(u64::from(other) % P::MODULUS.0[0])).unwrap()
                } else {
                    Self::from_repr(P::BigInt::from(u64::from(other))).unwrap()
                }
            }
        }
    };
}

macro_rules! sqrt_impl {
    ($Self:ident, $P:tt, $self:expr) => {{
        // https://eprint.iacr.org/2012/685.pdf (page 12, algorithm 5)
        // Actually this is just normal Tonelli-Shanks; since `P::Generator`
        // is a quadratic non-residue, `P::ROOT_OF_UNITY = P::GENERATOR ^ t`
        // is also a quadratic non-residue (since `t` is odd).
        if $self.is_zero() {
            return Some($Self::zero());
        }
        // Try computing the square root (x at the end of the algorithm)
        // Check at the end of the algorithm if x was a square root
        // Begin Tonelli-Shanks
        let mut z = $Self::qnr_to_t();
        let mut w = $self.pow($P::T_MINUS_ONE_DIV_TWO);
        let mut x = w * $self;
        let mut b = x * &w;

        let mut v = $P::TWO_ADICITY as usize;

        while !b.is_one() {
            let mut k = 0usize;

            let mut b2k = b;
            while !b2k.is_one() {
                // invariant: b2k = b^(2^k) after entering this loop
                b2k.square_in_place();
                k += 1;
            }

            if k == ($P::TWO_ADICITY as usize) {
                // We are in the case where self^(T * 2^k) = x^(P::MODULUS - 1) = 1,
                // which means that no square root exists.
                return None;
            }
            let j = v - k;
            w = z;
            for _ in 1..j {
                w.square_in_place();
            }

            z = w.square();
            b *= &z;
            x *= &w;
            v = k;
        }
        // Is x the square root? If so, return it.
        if (x.square() == *$self) {
            return Some(x);
        } else {
            // Consistency check that if no square root is found,
            // it is because none exists.
            #[cfg(debug_assertions)]
            {
                use crate::fields::LegendreSymbol::*;
                if ($self.legendre() != QuadraticNonResidue) {
                    panic!("Input has a square root per its legendre symbol, but it was not found")
                }
            }
            None
        }
    }};
}

#[macro_export]
macro_rules! impl_ops_from_ref {
    (
        $mod_name:ident,
        {<$($ops:ident),*>, $([$($iter_args:tt),*]),*}
        $type: ident,
        $([
            $type_params:ident,
            $bounds:ident$(<$($bound_params:tt),*>)?
            $(, $keyword:ident)?
        ]),*
    ) => {
        // We define a module, which we have the option to name, to hide the macros to prevent ambiguity
        mod $mod_name {
            use super::*;
            macro_rules! instantiate {
                ($d:tt) => {
                    macro_rules! result_body {
                        ($name:ident, $self:ident, $other:ident, $d ($d deref:tt)?) => {
                            paste::paste! {
                                let mut result = $self;
                                result.[<$name:snake _assign>](&$d($d deref)?$other);
                                return result;
                            }
                        }
                    }

                    macro_rules! assign_body {
                        ($name:ident, $self:ident, $other:ident, $d ($d deref:tt)?) => {
                            $self.$name(&$d($d deref)?$other)
                        }
                    }

                    macro_rules! ops {
                        (
                            $name:ident,
                            $body:ident
                            $d(, {$d output:ident $d ReturnType:ident})?
                            $d(, [$d self_mut:tt])?
                            $d(, <$d lifetime:tt $d mut:tt $d deref:tt>)?
                        ) => {
                            paste::paste! {
                                #[allow(unused_qualifications)]
                                impl<
                                    $d($d lifetime, )?
                                    $(
                                        $($keyword)?
                                        $type_params:
                                        $bounds$(<$($bound_params)*>)?
                                    ),*
                                > [<$name:camel>]<$d(&$d lifetime $d mut )?Self> for $type<$($type_params),*>
                                {
                                    $d(type $d output = Self;)?

                                    #[inline]
                                    fn $name(
                                        $d(&$d self_mut)?self,
                                        other: $d(&$d lifetime )?$d($d mut )?Self
                                    ) $d(-> $d ReturnType)? {
                                        $body!($name, self, other, $d($d deref)?);
                                    }
                                }
                            }
                        }
                    }

                    macro_rules! instantiate_ops {
                        ($d($d op:ident),*) => {
                            paste::paste! {
                                $d(
                                    ops!($d op, result_body, {Output Self});
                                    ops!($d op, result_body, {Output Self}, <'a mut *>);
                                    ops!([<$d op _assign>], assign_body, [mut]);
                                    ops!([<$d op _assign>], assign_body, [mut], <'a mut *>);
                                )*
                            }
                        }
                    }

                    macro_rules! iter {
                        (
                            $name:ident,
                            $ident:ident,
                            $op:ident
                            $d(, <$d lifetime:tt>)?
                        ) => {
                            paste::paste! {
                                #[allow(unused_qualifications)]
                                impl<
                                    $d($d lifetime, )?
                                    $(
                                        $($keyword)?
                                        $type_params:
                                        $bounds$(<$($bound_params)*>)?
                                    ),*
                                > core::iter::[<$name:camel>]<$d(&$d lifetime )?Self> for $type<$($type_params),*>
                                {
                                    fn $name<I: Iterator<Item = $d(&$d lifetime )?Self>>(iter: I) -> Self {
                                        iter.fold(Self::$ident(), [<$op:camel>]::$op)
                                    }
                                }
                            }
                        }
                    }
                }
            }
            instantiate!($);
            instantiate_ops!($($ops),*);
            $(
                iter!($($iter_args),*);
                iter!($($iter_args),*, <'a>);
            )*
        }

        pub use $mod_name::*;
    };
    // We instantiate default module name
    ({$($args0:tt)*}$($args:tt)*) => {
        impl_ops_from_ref!(default_ops_mod, {$($args0)*}$($args)*);
    };
    // Also, default ops
    ($($args:tt)*) => {
        impl_ops_from_ref!(
            default_ops_mod,
            {
                <add, sub, mul, div>,
                [sum, zero, add],
                [product, one, mul]
            }
            $($args)*
        );
    };
}
