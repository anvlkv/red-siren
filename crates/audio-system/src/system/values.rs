use fundsp::prelude::*;

// Keep all fine-tuned entries in the macro invocation below.
// Add/remove values there and all generated structs/impls update together.
macro_rules! define_fine_tuned_values {
    ($(($field:ident, $const_name:ident, $default:expr)),* $(,)?) => {
        #[derive(Clone)]
        pub struct FineTunedValues {
            $(pub $field: An<FineTunedValue>,)*
        }

        #[cfg(feature = "editor")]
        pub struct FineTunedSharedValues {
            $(pub $field: Shared,)*
        }

        $(pub(crate) const $const_name: f32 = $default;)*

        #[cfg(feature = "editor")]
        impl Default for FineTunedSharedValues {
            fn default() -> Self {
                Self {
                    $($field: shared($const_name),)*
                }
            }
        }

        #[cfg(feature = "editor")]
        impl FineTunedSharedValues {
            pub fn new() -> Self {
                Self::default()
            }
        }

        #[allow(clippy::new_without_default)]
        impl FineTunedValues {
            #[cfg(feature = "editor")]
            pub fn new(shared_values: &FineTunedSharedValues) -> Self {
                Self {
                    $($field: var(&shared_values.$field),)*
                }
            }

            #[cfg(not(feature = "editor"))]
            pub fn new() -> Self {
                Self {
                    $($field: constant($const_name),)*
                }
            }
        }

        impl std::fmt::Debug for FineTunedValues {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let mut dbg = f.debug_struct("FineTunedValues");
                $(dbg.field(stringify!($field), &self.$field.value());)*
                dbg.finish()
            }
        }
    };
}

#[cfg(not(feature = "editor"))]
pub type FineTunedValue = Constant<U1>;
#[cfg(feature = "editor")]
pub type FineTunedValue = Var;

define_fine_tuned_values! {
    (formants_q, FORMANTS_Q, 3.7),
    (band_bell_q, BAND_BELL_Q, 0.7),
    (band_bell_gain, BAND_BELL_GAIN, 0.5),
    (band_shelf_q, BAND_SHELF_Q, 2.2),
    (band_shelf_gain, BAND_SHELF_GAIN, 1.5),
}
