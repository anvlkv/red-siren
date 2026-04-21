pub mod excitor;
pub mod values;

#[cfg(feature = "editor")]
use values::FineTunedValues;

/// Legacy output-system entry point kept only as an explicit migration stub.
pub fn mount_output_system() {
    todo!()
}

/// Legacy input-system entry point kept only as an explicit migration stub.
pub fn create_input_system() {
    todo!()
}
