//! A writer which buffers the writes before it outputs it into the backing
//! storage.

use musli::context::Buffer;
use musli::Context;

use crate::fixed_bytes::{FixedBytes, FixedBytesOverflow};
use crate::writer::Writer;

/// A writer which buffers `N` bytes inline.
///
/// Once you're done you must call [BufferedWriter::finish] to flush the
/// underlying buffer.
pub struct BufferedWriter<const N: usize, W> {
    buf: FixedBytes<N>,
    writer: W,
}

impl<const N: usize, W> BufferedWriter<N, W>
where
    W: Writer,
{
    /// Construct a new buffered writer.
    pub fn new(writer: W) -> Self {
        Self {
            buf: FixedBytes::new(),
            writer,
        }
    }

    /// Finish writing.
    pub fn finish<C>(mut self, cx: &C) -> Result<(), C::Error>
    where
        C: Context<Input = W::Error>,
    {
        if !self.buf.is_empty() {
            self.writer.write_bytes(cx, self.buf.as_slice())?;
        }

        Ok(())
    }
}

impl<const N: usize, W> Writer for BufferedWriter<N, W>
where
    W: Writer,
    W::Error: From<FixedBytesOverflow>,
{
    type Error = W::Error;
    type Mut<'this> = &'this mut Self where Self: 'this;

    #[inline]
    fn borrow_mut(&mut self) -> Self::Mut<'_> {
        self
    }

    #[inline]
    fn write_buffer<C, B>(&mut self, cx: &C, buffer: B) -> Result<(), C::Error>
    where
        C: Context<Input = Self::Error>,
        B: Buffer,
    {
        // SAFETY: the buffer never outlives this function call.
        self.write_bytes(cx, unsafe { buffer.as_slice() })
    }

    #[inline]
    fn write_bytes<C>(&mut self, cx: &C, bytes: &[u8]) -> Result<(), C::Error>
    where
        C: Context<Input = Self::Error>,
    {
        if self.buf.remaining() < bytes.len() {
            self.writer.write_bytes(cx, self.buf.as_slice())?;
            self.buf.clear();
        }

        self.buf.write_bytes(cx.adapt(), bytes)
    }
}
