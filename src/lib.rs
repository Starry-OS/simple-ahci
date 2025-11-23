#![no_std]

extern crate alloc;

mod ahci;
mod ata;
mod hal;
mod mmio;
mod types;

pub use ahci::AhciDriver;
pub use hal::Hal;
