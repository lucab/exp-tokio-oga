/*! Asynchronous I/O logic for virtio-serial devices.

This implements asynchrounous logic for reading and writing
from virtio serial ports (i.e. /dev/vport<X>n<Y>).
Those are character devices that can polled and support read()
and write() in non-blocking mode, but are not seekable.

References:
 * <https://www.linux-kvm.org/page/Virtio-serial_API>
!*/

use crate::errors;
use mio::event::Evented;
use mio::unix::EventedFd;
use mio::{Poll, PollOpt, Ready, Token};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;
use std::path::Path;
use tokio::io::PollEvented;

/// VirtIO serial port (guest side).
#[derive(Debug)]
pub struct VirtioPort {
    dev: File,
}

impl VirtioPort {
    /// Open a virtio-serial device at given path, in non-blocking mode.
    pub(crate) fn open(path: impl AsRef<Path>) -> Result<Self, errors::OgaError> {
        let dev = OpenOptions::new()
            .create(false)
            .read(true)
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(path.as_ref())
            .map_err(|e| format!("failed to open device '{}': {}", path.as_ref().display(), e))?;
        let vport = Self { dev };
        Ok(vport)
    }

    /// Trasnsform into a tokio-compatible evented object.
    pub(crate) fn evented(self) -> Result<PollEvented<VirtioPort>, errors::OgaError> {
        PollEvented::new(self)
            .map_err(|e| format!("failed to register pollable virtio port: {}", e).into())
    }
}

impl Evented for VirtioPort {
    fn register(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> std::io::Result<()> {
        EventedFd(&self.dev.as_raw_fd()).register(poll, token, interest, opts)
    }

    fn reregister(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> std::io::Result<()> {
        EventedFd(&self.dev.as_raw_fd()).reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &Poll) -> std::io::Result<()> {
        EventedFd(&self.dev.as_raw_fd()).deregister(poll)
    }
}

impl Write for VirtioPort {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.dev.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.dev.flush()
    }
}

impl Read for VirtioPort {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.dev.read(buf)
    }
}
