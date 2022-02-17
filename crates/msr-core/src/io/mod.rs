//! I/O related utilities

use std::io::{Result, Write};

use crate::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

#[derive(Clone, Debug)]
pub struct BytesWritten(Arc<AtomicU64>);

impl BytesWritten {
    #[must_use]
    pub fn value(&self) -> u64 {
        self.0.load(Ordering::Relaxed)
    }
}

#[derive(Debug)]
pub struct CountingWrite<W: Write> {
    writer: W,
    bytes_written: Arc<AtomicU64>,
}

impl<W: Write> CountingWrite<W> {
    /// Wrap a writer and start counting
    pub fn from_writer(writer: W) -> (Self, BytesWritten) {
        let bytes_written = Arc::new(AtomicU64::new(0));
        (
            Self {
                writer,
                bytes_written: bytes_written.clone(),
            },
            BytesWritten(bytes_written),
        )
    }

    /// Dismantle the wrapped writer and stop counting
    pub fn into_value(self) -> W {
        self.writer
    }
}

impl<W: Write> Write for CountingWrite<W> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        let bytes_written = self.writer.write(buf)?;
        // self has exclusive mutable access on the number of octets written, i.e.
        // we can safely get-modify-set this value without race conditions here!
        let mut sum_bytes_written = self.bytes_written.load(Ordering::Relaxed);
        sum_bytes_written = sum_bytes_written.saturating_add(bytes_written as u64);
        self.bytes_written
            .store(sum_bytes_written, Ordering::Relaxed);
        Ok(bytes_written)
    }

    fn flush(&mut self) -> Result<()> {
        self.writer.flush()
    }
}

#[cfg(test)]
mod tests;
