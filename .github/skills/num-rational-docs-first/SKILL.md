---
name: num-rational-docs-first
description: 'Use num-rational effectively from official docs/source: Ratio invariants, constructors, checked arithmetic, float conversion, parsing/formatting, and feature selection.'
---

# num-rational Docs-First

## When to Use
- Adding or reviewing code that uses `num_rational::Ratio`.
- Deciding between `Rational32`, `Rational64`, and `BigRational`.
- Handling denominator invariants, normalization, and zero-denominator failure modes.
- Implementing exact arithmetic, checked arithmetic, float conversion, parsing, formatting, or serde support with `num-rational`.

## Procedure
1. Pick the representation by documented guarantees, not convenience aliases.
   - Prefer `Rational32` or `Rational64` for bounded integer domains.
   - Use `BigRational` when overflow risk or exact float decomposition matters.
   - Do not introduce the deprecated `Rational` alias (`Ratio<isize>`).

   **Example:**
   ```rust
   use num_rational::{Ratio, Rational64};
   
   // For 32-bit or 64-bit arithmetic with guaranteed overflow safety:
   let small: Rational64 = Ratio::new(3, 4);
   
   // For arbitrary precision when floats must be exact:
   #[cfg(feature = "num-bigint")]
   {
       use num_rational::BigRational;
       let exact = BigRational::from_float(0.123).unwrap();
   }
   ```

2. Choose constructors by invariant requirements.
   - Use `Ratio::new(n, d)` for normal construction: it reduces to lowest terms and keeps denominator positive.
   - Use `Ratio::from_integer(x)` for exact integer promotion.
   - Use `Ratio::new_raw(n, d)` only when intentionally bypassing checks/reduction and when later code can tolerate non-reduced or zero-denominator states.

   **Example:**
   ```rust
   use num_rational::Ratio;
   
   // Safe, reduced, denom > 0
   let r1 = Ratio::new(6, 8);  // Auto-reduces to 3/4
   assert_eq!(r1.numer(), &3);
   assert_eq!(r1.denom(), &4);
   
   // Integer promotion
   let r2 = Ratio::from_integer(5i64);
   assert!(r2.is_integer());
   
   // Dangerous: bypasses checks
   // Use only if you control invariants downstream:
   let r3 = Ratio::<i64>::new_raw(1, 0);
   // ⚠️  Several methods (recip, reduced, etc.) will panic on denom==0
   ```

3. Enforce denominator safety explicitly at boundaries.
   - `Ratio::new` panics for `denom == 0`.
   - `new_raw` allows `denom == 0`; several methods panic on such values.
   - Prefer fallible validation before construction when data is external.

   **Example:**
   ```rust
   use num_rational::Ratio;
   
   fn safe_from_external(numer: i64, denom: i64) -> Result<Ratio<i64>, String> {
       if denom == 0 {
           Err("denominator is zero".into())
       } else {
           Ok(Ratio::new(numer, denom))
       }
   }
   
   // vs. panicking:
   fn unsafe_from_external(numer: i64, denom: i64) -> Ratio<i64> {
       Ratio::new(numer, denom)  // panics if denom == 0
   }
   ```

4. Select arithmetic APIs based on overflow behavior.
   - For bounded integer `T`, prefer `CheckedAdd`, `CheckedSub`, `CheckedMul`, `CheckedDiv` when overflow or invalid division is possible.
   - Use operator overloads when panic/overflow behavior is acceptable under your type and input constraints.
   - Remember reciprocal semantics: `recip()` panics on zero.

   **Example:**
   ```rust
   use num_rational::Ratio;
   use num_traits::ops::checked::{CheckedAdd, CheckedMul};
   
   let a: Ratio<u8> = Ratio::new(200u8, 1);
   let b: Ratio<u8> = Ratio::new(100u8, 1);
   
   // Safe: returns None on overflow
   let maybe_sum = a.checked_add(&b);
   assert_eq!(maybe_sum, None);  // 255 + overflow
   
   // Operator panics on overflow (avoid with bounded int):
   // let sum = a + b;  // panic!
   
   // Reciprocal safety:
   let zero: Ratio<i64> = Ratio::new(0, 1);
   // zero.recip();  // panics!
   ```

5. Use documented rounding and decomposition semantics.
   - `to_integer()` and `trunc()` round toward zero.
   - `floor()`, `ceil()`, `round()` have distinct behavior; `round()` rounds half-way cases away from zero.
   - `fract()` satisfies `self == self.trunc() + self.fract()`.

   **Example:**
   ```rust
   use num_rational::Ratio;
   use num_traits::Zero;
   
   let r = Ratio::new(7, 3);  // 2.333...
   
   assert_eq!(r.trunc(), Ratio::new(2, 1));
   assert_eq!(r.floor(), Ratio::new(2, 1));
   assert_eq!(r.ceil(), Ratio::new(3, 1));
   assert_eq!(r.round(), Ratio::new(2, 1));  // rounds away from zero
   
   // Decomposition invariant:
   let fract = r.fract();
   assert_eq!(r, r.trunc() + fract);
   
   // Negative rounding differs:
   let neg = Ratio::new(-7, 3);  // -2.333...
   assert_eq!(neg.floor(), Ratio::new(-3, 1));  // toward -∞
   assert_eq!(neg.ceil(), Ratio::new(-2, 1));   // toward +∞
   ```

6. Handle float interop by exactness goals.
   - `BigRational::from_float(f)` returns exact decomposition for finite `f`, else `None`.
   - `Ratio::<T>::approximate_float` and `approximate_float_unsigned` use bounded continued-fraction approximation and return `None` on invalid/overflow cases.
   - Avoid silent precision assumptions when converting back with `ToPrimitive::to_f64`.

   **Example:**
   ```rust
   use num_rational::Ratio;
   use num_traits::FromPrimitive;
   
   // Exact decomposition for BigRational:
   #[cfg(feature = "num-bigint")]
   {
       use num_rational::BigRational;
       let exact = BigRational::from_float(0.125).unwrap();
       assert_eq!(exact, Ratio::new(1u8, 8));
   }
   
   // Approximate for bounded integers:
   let approx: Ratio<i32> = Ratio::approximate_float(0.333f32).unwrap();
   // Returns a best-rational approximation within tolerance
   
   // Invalid/infinite cases return None:
   assert_eq!(Ratio::<i64>::approximate_float(f32::NAN), None);
   assert_eq!(Ratio::<i64>::approximate_float(f32::INFINITY), None);
   ```

7. Use parsing and formatting from documented contracts.
   - `FromStr` accepts `numer/denom` or `numer`.
   - Parsing rejects zero denominators with `ParseRatioError`.
   - Formatting traits (`Display`, `Binary`, `Octal`, `LowerHex`, `UpperHex`, `LowerExp`, `UpperExp`) preserve ratio structure (`a/b`) unless denominator is one.

   **Example:**
   ```rust
   use std::str::FromStr;
   use num_rational::Ratio;
   
   // Parse from strings:
   let r1: Ratio<i64> = "3/4".parse().unwrap();
   let r2: Ratio<i64> = "5".parse().unwrap();  // integer
   
   // Reject zero denominator:
   assert!("3/0".parse::<Ratio<i64>>().is_err());
   
   // Format differently based on denominator:
   let r = Ratio::new(5i64, 1);
   assert_eq!(format!("{}", r), "5");  // integer, no slash
   
   let r = Ratio::new(3i64, 4);
   assert_eq!(format!("{}", r), "3/4");
   assert_eq!(format!("{:b}", r), "11/100");  // binary
   ```

8. Configure features intentionally.
   - Default features include `std` and `num-bigint`.
   - Disable default features for `no_std` use.
   - Enable `serde` only when serialization/deserialization is required.

   **Example (Cargo.toml):**
   ```toml
   # With all defaults (std + num-bigint):
   [dependencies]
   num-rational = "0.4"
   
   # Minimal, no-std:
   [dependencies]
   num-rational = { version = "0.4", default-features = false }
   
   # With serde support:
   [dependencies]
   num-rational = { version = "0.4", features = ["serde"] }
   ```

9. Prefer trait-driven generic code only when trait bounds are justified.
   - Base integer operations need `T: Clone + Integer`.
   - Checked ops require corresponding `Checked*` traits.
   - Signed/unsigned float approximation APIs require the matching `Signed` or `Unsigned` bounds.

   **Example:**
   ```rust
   use num_rational::Ratio;
   use num_traits::Integer;
   
   fn generic_ops<T: Clone + Integer>(a: Ratio<T>, b: Ratio<T>) -> Ratio<T> {
       a + b
   }
   
   // Only signed types can use approximate_float:
   use num_traits::{Signed, Bounded, NumCast};
   fn approx_signed<T: Integer + Signed + Bounded + NumCast + Clone>(
       f: f32
   ) -> Option<Ratio<T>> {
       Ratio::approximate_float(f)
   }
   ```

10. Validate behavior with focused tests tied to crate semantics.
   - Include zero-denominator handling, normalization, sign normalization (`denom > 0` after `new`), and parse/format round-trips.
   - Include overflow-path tests for checked arithmetic on bounded integer types.
   - Include float conversion tests for finite, NaN, and +/-infinity cases.

   **Example:**
   ```rust
   #[cfg(test)]
   mod tests {
       use super::*;
       use num_rational::Ratio;
       use num_traits::{FromPrimitive, Zero};
   
       #[test]
       fn test_zero_denom_safety() {
           // Panics at construction:
           let result = std::panic::catch_unwind(|| Ratio::new(1i64, 0));
           assert!(result.is_err());
       }
   
       #[test]
       fn test_normalization() {
           let r = Ratio::new(6i64, 8);
           assert_eq!(*r.numer(), 3);
           assert_eq!(*r.denom(), 4);
           assert!(*r.denom() > 0);
       }
   
       #[test]
       fn test_overflow_checked() {
           let a: Ratio<u8> = Ratio::new(255u8, 1);
           let b: Ratio<u8> = Ratio::new(1u8, 1);
           assert!(a.checked_add(&b).is_none());
       }
   
       #[test]
       fn test_float_round_trip() {
           assert_eq!(Ratio::<i64>::approximate_float(f32::NAN), None);
           assert_eq!(Ratio::<i64>::approximate_float(f32::INFINITY), None);
           
           let exact = Ratio::approximate_float(0.5f32).unwrap();
           assert_eq!(exact, Ratio::new(1, 2));
       }
   }
   ```

## Completion Checks
- Chosen ratio type matches numeric range and precision requirements.
- No accidental usage of deprecated `Rational` alias.
- Constructor choice (`new` vs `new_raw`) is deliberate and documented.
- Zero-denominator paths are prevented, handled, or tested.
- Overflow-sensitive arithmetic uses checked APIs where required.
- Float conversion exactness/approximation choice is explicit.
- Required feature flags (`std`, `num-bigint`, `serde`) are explicitly justified.

## References
- [num-rational crate docs](https://docs.rs/num-rational/latest/num_rational/)
- [Ratio API](https://docs.rs/num-rational/latest/num_rational/struct.Ratio.html)
- [BigRational alias and impls](https://docs.rs/num-rational/latest/num_rational/type.BigRational.html)
- [Feature flags](https://docs.rs/crate/num-rational/latest/features)
- [Source: lib.rs](https://docs.rs/crate/num-rational/latest/source/src/lib.rs)
- [Source: pow.rs](https://docs.rs/crate/num-rational/latest/source/src/pow.rs)
