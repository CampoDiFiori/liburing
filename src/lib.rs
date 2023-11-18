#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

// include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
pub mod bindings;
pub use bindings::*;

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
