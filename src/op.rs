use crate::BgId;

#[derive(Debug)]
pub enum IOUringOp {
    Accept,
    Send,
    Recv { sockfd: i32, bgid: BgId },
}

impl IOUringOp {
    pub fn from_u64(userdata: u64) -> Self {
        unsafe { std::mem::transmute(userdata) }
    }

    pub fn into_u64(self: Self) -> u64 {
        unsafe { std::mem::transmute(self) }
    }
}
