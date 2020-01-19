//! This crates exports an interface similar to what an svd2rust PAC would look like.

#![no_std]

pub use cortex_m_rt::{self, interrupt};

#[allow(non_camel_case_types)]
pub enum interrupt {
    INT0,
    INT1,
    INT2,
    INT3,
}
