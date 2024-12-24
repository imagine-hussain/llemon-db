use libc;
use std::os::fd::AsRawFd;

pub struct Mmap {
    data: *mut u8,
    len: usize,
}

impl Mmap {
    pub unsafe fn map(file: &mut std::fs::File) -> Self {
        let filelen = file.metadata().expect("Failed to get file length").len();
        let res = libc::mmap(
            std::ptr::null_mut(),
            filelen as libc::size_t,
            libc::PROT_READ,
            libc::MAP_PRIVATE,
            file.as_raw_fd(),
            0
        );
        if res == libc::MAP_FAILED {
            panic!("Failed to map file. This should be handled at some point.");
        }

        Self {
            data: res as *mut u8,
            len: filelen as usize,
        }
    }

    pub fn leak(self) -> &'static [u8] {
        let mut s = self;

        let data = s.data;
        let len = s.len;
        s.data = std::ptr::null_mut();

        unsafe {
            std::slice::from_raw_parts(data, len)
        }
    }

    pub unsafe fn from_leaked_slice(data: &'static [u8]) -> Self {
        let len = data.len();
        let data = data.as_ptr() as *mut u8;

        Self::from_raw_parts(data, len)
    }

    pub unsafe fn from_raw_parts(data: *mut u8, len: usize) -> Self {
        Self { data, len }
    }
}

impl Drop for Mmap {
    fn drop(&mut self) {
        if self.data.is_null() {
            return;
        }
        unsafe {
            let res = libc::munmap(self.data as *mut std::ffi::c_void, self.len);
            if res == -1 {
                panic!("Failed to munmap file. This should be handled at some point.");
            }
        }
    }
}


impl AsRef<[u8]> for Mmap {
    fn as_ref(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.data, self.len) }
    }
}