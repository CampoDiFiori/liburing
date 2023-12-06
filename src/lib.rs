#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use libc::c_void;
use std::{collections::HashMap, os::fd::AsRawFd};

// include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
pub mod bindings;
pub mod op;
pub mod provided_buffers;

pub use bindings::*;
pub use op::*;
pub use provided_buffers::*;

/// use fixed fileset
pub const IOSQE_FIXED_FILE: u32 = 1 << IOSQE_FIXED_FILE_BIT;
/// issue after inflight IO
pub const IOSQE_IO_DRAIN: u32 = 1 << IOSQE_IO_DRAIN_BIT;
/// links next sqe
pub const IOSQE_IO_LINK: u32 = 1 << IOSQE_IO_LINK_BIT;
/// like LINK, but stronger
pub const IOSQE_IO_HARDLINK: u32 = 1 << IOSQE_IO_HARDLINK_BIT;
/// always go async
pub const IOSQE_ASYNC: u32 = 1 << IOSQE_ASYNC_BIT;
/// select buffer from sqe->buf_group
pub const IOSQE_BUFFER_SELECT: u32 = 1 << IOSQE_BUFFER_SELECT_BIT;
/// don't post CQE if request succeeded
pub const IOSQE_CQE_SKIP_SUCCESS: u32 = 1 << IOSQE_CQE_SKIP_SUCCESS_BIT;

pub struct IOUring {
    ring: io_uring,
    buffer_groups: HashMap<BgId, (*mut io_uring_buf_ring, *mut u8, REntrySize, u32)>,
}

impl IOUring {
    pub fn new(size: u32) -> std::io::Result<Self> {
        let ring = unsafe {
            let mut s = std::mem::MaybeUninit::<io_uring>::uninit();
            let ret = io_uring_queue_init(size, s.as_mut_ptr(), 0);
            if ret < 0 {
                return Err(std::io::Error::from_raw_os_error(ret));
            }
            s.assume_init()
        };

        Ok(Self {
            ring,
            buffer_groups: HashMap::new(),
        })
    }

    pub fn get_sqe(&mut self) -> Option<IOUringSqe> {
        unsafe {
            io_uring_get_sqe(&mut self.ring)
                .as_mut()
                .map(|sqe| IOUringSqe { sqe })
        }
    }

    pub fn submit(&mut self) -> std::io::Result<()> {
        let ret = unsafe { io_uring_submit(&mut self.ring as *mut _) };
        if ret < 0 {
            return Err(std::io::Error::from_raw_os_error(ret));
        }

        Ok(())
    }

    pub fn prep_read(&mut self, file: &std::fs::File, offset: u64, bgid: BgId) {
        let sqe = self
            .get_sqe()
            .expect("get sqe")
            .prep_read(file, std::ptr::null_mut(), 0, offset)
            .set_user_data(bgid as _);

        unsafe {
            io_uring_sqe_set_flags(sqe.sqe, IOSQE_BUFFER_SELECT);
            sqe.sqe.__bindgen_anon_4.buf_group = bgid;
        }
    }

    pub fn prep_multishot_accept(&mut self, sockfd: i32) {
        let sqe = self.get_sqe().expect("get multi-accept sqe");

        unsafe {
            io_uring_prep_multishot_accept(
                sqe.sqe,
                sockfd,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                0,
            );
        }
    }

    pub fn prep_multishot_receive(&mut self, sockfd: i32, bgid: BgId) {
        let sqe = self.get_sqe().expect("get multi-recv sqe");

        unsafe {
            io_uring_prep_recv_multishot(sqe.sqe, sockfd, std::ptr::null_mut(), 0, 0);
            io_uring_sqe_set_data64(sqe.sqe, IOUringOp::Recv { sockfd, bgid }.into_u64());
            io_uring_sqe_set_flags(sqe.sqe, IOSQE_BUFFER_SELECT);
            sqe.sqe.__bindgen_anon_4.buf_group = bgid;
        }
    }

    pub fn wait_cqe(&mut self) -> std::io::Result<IOUringCqe> {
        let mut cqe: *mut io_uring_cqe = unsafe { std::mem::zeroed() };
        let ret = unsafe { io_uring_wait_cqe(&mut self.ring, &mut cqe) };
        if ret < 0 {
            return Err(std::io::Error::from_raw_os_error(ret));
        }
        let cqe = unsafe {
            cqe.as_mut()
                .expect("BUG: cqe is null event though its initialization succeeded")
        };

        if cqe.res < 0 {
            return Err(std::io::Error::from_raw_os_error(-cqe.res));
        }

        unsafe {
            io_uring_cqe_seen(&mut self.ring, cqe);
        }

        Ok(IOUringCqe {
            op: IOUringOp::from_u64(cqe.user_data),
            res: cqe.res,
            flags: cqe.flags,
        })
    }
}

pub struct IOUringSqe<'a> {
    sqe: &'a mut io_uring_sqe,
}

impl<'a> IOUringSqe<'a> {
    pub fn prep_read(self, file: &std::fs::File, buf: *mut u8, cap: usize, offset: u64) -> Self {
        unsafe {
            io_uring_prep_read(
                self.sqe as *mut _,
                file.as_raw_fd(),
                buf as *mut c_void,
                cap as u32,
                offset,
            )
        }
        self
    }

    pub fn set_user_data(self, user_data: u64) -> Self {
        unsafe { io_uring_sqe_set_data64(self.sqe as *mut _, user_data) };
        self
    }
}

pub struct IOUringCqe {
    pub op: IOUringOp,
    pub res: i32,
    pub flags: u32,
}

#[cfg(test)]
mod tests {
    use std::io::Error;
    use std::mem;

    use crate::*;

    const QUEUE_DEPTH: u32 = 4;

    #[test]
    fn test_io_uring_queue_init() {
        let mut ring = unsafe {
            let mut s = mem::MaybeUninit::<io_uring>::uninit();
            let ret = io_uring_queue_init(QUEUE_DEPTH, s.as_mut_ptr(), 0);
            if ret < 0 {
                panic!("io_uring_queue_init: {:?}", Error::from_raw_os_error(ret));
            }
            s.assume_init()
        };

        loop {
            let sqe = unsafe { io_uring_get_sqe(&mut ring) };
            if sqe == std::ptr::null_mut() {
                break;
            }
            unsafe { io_uring_prep_nop(sqe) };
        }
        let ret = unsafe { io_uring_submit(&mut ring) };
        if ret < 0 {
            panic!("io_uring_submit: {:?}", Error::from_raw_os_error(ret));
        }

        let mut cqe: *mut io_uring_cqe = unsafe { std::mem::zeroed() };
        // let mut done = 0;
        let pending = ret;
        for _ in 0..pending {
            let ret = unsafe { io_uring_wait_cqe(&mut ring, &mut cqe) };
            if ret < 0 {
                panic!("io_uring_wait_cqe: {:?}", Error::from_raw_os_error(ret));
            }
            // done += 1;
            if unsafe { (*cqe).res } < 0 {
                eprintln!("(*cqe).res = {}", unsafe { (*cqe).res });
            }
            unsafe { io_uring_cqe_seen(&mut ring, cqe) };
        }

        // println!("Submitted={}, completed={}", pending, done);
        unsafe { io_uring_queue_exit(&mut ring) };
    }
}
