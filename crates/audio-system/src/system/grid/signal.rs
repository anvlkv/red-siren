use fundsp::numeric_array::ArrayLength;
use fundsp::prelude::*;
use fundsp::typenum::Unsigned;

/// A type that can be transported through `Frame<f32, N>` by copying its raw bytes.
///
/// The unsafe contract is that `Self` must be valid when its bytes are copied into
/// and out of a frame of the declared size. The default methods perform the raw copy
/// so implementors do not need bespoke encode/decode logic.
pub unsafe trait FrameEncodedSignal: Sized {
    /// The size of the frame required to encode this signal. This must be large enough to hold the raw bytes of `Self`.
    ///
    /// The default implementation of `encode` and `decode` assumes that `Self` can be safely transmuted to and from a `Frame<f32, Self::Size>`.
    ///
    /// `f32` size is 4 bytes, so the size in bytes of the frame is `4 * Self::Size::USIZE`. Therefore, `Self` must be at most `4 * Self::Size::USIZE` bytes in size.
    type Size: Unsigned + ArrayLength + Size<f32>;

    fn encode(&self) -> Frame<f32, Self::Size> {
        assert_eq!(
            std::mem::size_of::<Self>(),
            std::mem::size_of::<Frame<f32, Self::Size>>()
        );

        let mut frame = std::mem::MaybeUninit::<Frame<f32, Self::Size>>::uninit();
        unsafe {
            std::ptr::copy_nonoverlapping(
                self as *const Self as *const u8,
                frame.as_mut_ptr() as *mut u8,
                std::mem::size_of::<Self>(),
            );
            frame.assume_init()
        }
    }

    fn decode(frame: &Frame<f32, Self::Size>) -> Self {
        assert_eq!(
            std::mem::size_of::<Self>(),
            std::mem::size_of::<Frame<f32, Self::Size>>()
        );

        let mut value = std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            std::ptr::copy_nonoverlapping(
                frame as *const Frame<f32, Self::Size> as *const u8,
                value.as_mut_ptr() as *mut u8,
                std::mem::size_of::<Self>(),
            );
            value.assume_init()
        }
    }

    fn size() -> usize {
        Self::Size::USIZE
    }
}
