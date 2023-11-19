use crate::{
    io_uring_buf_reg, io_uring_buf_ring, io_uring_buf_ring_add, io_uring_buf_ring_advance,
    io_uring_buf_ring_init, io_uring_buf_ring_mask, io_uring_register_buf_ring, IOUring,
    IORING_CQE_BUFFER_SHIFT,
};

pub type BgId = u16;
pub type REntrySize = u32;

impl IOUring {
    pub fn setup_buffer_ring(
        mut self,
        rentry_size: REntrySize,
        rentries: u32,
        bgid: BgId,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut helper_buf: Vec<u8> = Vec::with_capacity((rentries * rentry_size) as _);
        let ring_space = helper_buf.as_mut_ptr();
        std::mem::forget(helper_buf);

        let buf_ring = unsafe {
            let buf_ring: *mut io_uring_buf_ring =
                std::alloc::alloc_zeroed(std::alloc::Layout::from_size_align(
                    rentries as usize * std::mem::size_of::<io_uring_buf_ring>(),
                    4096,
                )?)
                .cast();

            let mut reg = io_uring_buf_reg {
                ring_addr: buf_ring as u64,
                ring_entries: rentries,
                bgid,
                flags: 0,
                resv: [0; 3],
            };

            let ret = io_uring_register_buf_ring(&mut self.ring, &mut reg, 0);
            if ret < 0 {
                return Err(Box::new(std::io::Error::from_raw_os_error(-ret)));
            }

            io_uring_buf_ring_init(buf_ring);
            for i in 0..rentries {
                io_uring_buf_ring_add(
                    buf_ring,
                    ring_space.offset((i * rentry_size) as _).cast(),
                    rentry_size,
                    i as _,
                    io_uring_buf_ring_mask(rentries),
                    i as _,
                );
            }
            io_uring_buf_ring_advance(buf_ring, rentries as _);
            buf_ring
        };

        self.buffer_groups
            .insert(bgid, (buf_ring, ring_space, rentry_size, rentries));

        Ok(self)
    }

    pub fn wait_read(&mut self) -> std::io::Result<ProvidedBuffer> {
        let cqe = self.wait_cqe()?;
        let bgid = cqe.cqe.user_data;
        let len = cqe.cqe.res as usize;
        let buf_idx = cqe.cqe.flags >> IORING_CQE_BUFFER_SHIFT;
        drop(cqe);

        let (br, bufs, rentry_size, rentries) = self.buffer_groups[&(bgid as BgId)];

        unsafe {
            let ptr = bufs.offset((buf_idx * rentry_size) as _);
            Ok(ProvidedBuffer::new(
                ptr,
                len,
                br,
                rentry_size,
                rentries,
                buf_idx,
            ))
        }
    }
}

pub struct ProvidedBuffer {
    ptr: *mut u8,
    len: usize,
    br: *mut io_uring_buf_ring,
    rentry_size: REntrySize,
    rentries: u32,
    idx: u32,
}

impl ProvidedBuffer {
    fn new(
        ptr: *mut u8,
        len: usize,
        br: *mut io_uring_buf_ring,
        rentry_size: REntrySize,
        rentries: u32,
        idx: u32,
    ) -> Self {
        Self {
            ptr,
            len,
            br,
            rentry_size,
            rentries,
            idx,
        }
    }
}

impl AsRef<[u8]> for ProvidedBuffer {
    fn as_ref(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
    }
}

impl AsMut<[u8]> for ProvidedBuffer {
    fn as_mut(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.ptr, self.len) }
    }
}

impl Drop for ProvidedBuffer {
    fn drop(&mut self) {
        unsafe {
            io_uring_buf_ring_add(
                self.br,
                self.ptr as _,
                self.rentry_size,
                self.idx as _,
                io_uring_buf_ring_mask(self.rentries),
                self.idx as _,
            );
            io_uring_buf_ring_advance(self.br, self.rentries as _);
        }
    }
}
