#![allow(non_camel_case_types)]

use volatile::VolatileFieldAccess;

#[derive(Debug, Default, Clone, Copy)]
#[repr(C)]
pub struct ahci_cmd_hdr {
    pub opts: u32,
    pub status: u32,
    pub tbl_addr_lo: u32,
    pub tbl_addr_hi: u32,
    pub reserved: [u32; 4],
}

const AHCI_MAX_CMDS: usize = 32;

pub type ahci_cmd_list = [ahci_cmd_hdr; AHCI_MAX_CMDS];

pub type ahci_rx_fis = [u8; 256];

#[derive(Debug, Default, Clone, Copy)]
#[repr(C)]
pub struct ahci_sg {
    pub addr_lo: u32,
    pub addr_hi: u32,
    pub reserved: u32,
    pub flags_size: u32,
}

pub const AHCI_MAX_SG: usize = 56;
pub const AHCI_MAX_BYTES_PER_SG: usize = 4 * 1024 * 1024; // 4 MiB
pub const AHCI_MAX_BYTES_PER_CMD: usize = AHCI_MAX_SG * AHCI_MAX_BYTES_PER_SG;

#[derive(Debug, Default, Clone, Copy)]
#[repr(C)]
pub struct sata_fis_h2d {
    pub fis_type: u8,
    pub pm_port_c: u8,
    pub command: u8,
    pub features: u8,
    pub lba_low: u8,
    pub lba_mid: u8,
    pub lba_high: u8,
    pub device: u8,
    pub lba_low_exp: u8,
    pub lba_mid_exp: u8,
    pub lba_high_exp: u8,
    pub features_exp: u8,
    pub sector_count: u8,
    pub sector_count_exp: u8,
    pub res1: u8,
    pub control: u8,
    pub res2: [u8; 4],
}

#[derive(Debug, Clone)]
#[repr(C)]
#[derive(VolatileFieldAccess)]
pub struct ahci_cmd_tbl {
    pub hdr: sata_fis_h2d,
    res: [u8; 0x6c],
    pub sgs: [ahci_sg; AHCI_MAX_SG],
}
