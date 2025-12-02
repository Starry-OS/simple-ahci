use alloc::alloc::alloc_zeroed;
use core::{alloc::Layout, marker::PhantomData, ptr::NonNull};

use log::{debug, error, info, warn};
use volatile::VolatilePtr;

use crate::{
    Hal,
    ata::{
        ATA_CMD_ID_ATA, ATA_CMD_READ, ATA_CMD_READ_EXT, ATA_CMD_WRITE, ATA_CMD_WRITE_EXT,
        ATA_ID_FW_REV, ATA_ID_FW_REV_LEN, ATA_ID_PROD, ATA_ID_PROD_LEN, ATA_ID_SERNO,
        ATA_ID_SERNO_LEN, ATA_ID_WORDS, SATA_FIS_TYPE_REGISTER_H2D, ata_id_has_lba48,
        ata_id_n_sectors, ata_id_to_string,
    },
    hal::wait_until_timeout,
    mmio::{
        AhciMmio, AhciMmioVolatileFieldAccess, CAP, GenericHostControlVolatileFieldAccess, ICC,
        PortRegisters, PortRegistersVolatileFieldAccess, PxCMD, PxI,
    },
    types::{
        AHCI_MAX_BYTES_PER_CMD, AHCI_MAX_BYTES_PER_SG, AHCI_MAX_SG, ahci_cmd_hdr, ahci_cmd_list,
        ahci_cmd_tbl, ahci_cmd_tblVolatileFieldAccess, ahci_rx_fis, ahci_sg, sata_fis_h2d,
    },
};

fn alloc<T: Sized>(align: usize) -> VolatilePtr<'static, T> {
    unsafe {
        VolatilePtr::new(NonNull::new_unchecked(
            alloc_zeroed(Layout::from_size_align(size_of::<T>(), align).unwrap()).cast(),
        ))
    }
}

struct AhciPort<H> {
    port: VolatilePtr<'static, PortRegisters>,

    cmd_list: VolatilePtr<'static, ahci_cmd_list>,
    #[allow(dead_code)]
    fis: VolatilePtr<'static, ahci_rx_fis>,
    cmd_tbl: VolatilePtr<'static, ahci_cmd_tbl>,

    _h: PhantomData<H>,
}

impl<H: Hal> AhciPort<H> {
    fn try_new(host: &VolatilePtr<'static, AhciMmio>, i: u8) -> Option<Self> {
        let port = unsafe {
            host.ports()
                .map(|ports| ports.cast::<PortRegisters>().add(i as usize))
        };

        // 1. Stop the port (ST=0, FRE=0)
        port.CMD().update(|cmd| cmd.with_ST(false).with_FRE(false));

        // Wait for CR and FR to clear
        if !wait_until_timeout::<H>(|| !port.CMD().read().CR(), 500) {
            warn!("Port {i} stop engine timeout (CR)");
        }
        if !wait_until_timeout::<H>(|| !port.CMD().read().FR(), 500) {
            warn!("Port {i} stop FIS receive timeout (FR)");
        }

        // 2. Check if device is busy (BSY or DRQ) and try CLO
        let tfd = port.TFD().read();
        if tfd.STS_BSY() || tfd.STS_DRQ() {
            debug!("Port {i} busy (TFD: {tfd:?}), trying CLO");
            let cap = host.host().cap().read();
            if cap.SCLO() {
                port.CMD().update(|cmd| cmd.with_CLO(true));
                if !wait_until_timeout::<H>(|| !port.CMD().read().CLO(), 1000) {
                    warn!("Port {i} CLO timeout");
                }
            }
        }

        // 3. Spin up
        port.CMD().update(|cmd| cmd.with_SUD(true));
        if !wait_until_timeout::<H>(|| port.CMD().read().SUD(), 1000) {
            warn!("Port {i} set Spin-Up Device timeout");
            return None;
        }

        // 4. Wait for Link Up
        if !wait_until_timeout::<H>(
            || {
                let det = port.SSTS().read().DET();
                det == 0x1 || det == 0x3
            },
            1000,
        ) {
            warn!("Port {i} sata link timeout");
            return None;
        }
        debug!("Port {i} sata link up");

        // 5. Clear Errors
        port.SERR().write(port.SERR().read());
        port.IS().write(port.IS().read());

        // 6. Enable Interrupts
        port.IE().write(PxI::default_enable().with_DP(true));

        host.host().is().write(1 << i);

        if port.SSTS().read().DET() != 3 {
            // Try to wait a bit more if it is 1
            if !wait_until_timeout::<H>(|| port.SSTS().read().DET() == 3, 1000) {
                warn!(
                    "Port {i} physical link not established (DET={})",
                    port.SSTS().read().DET()
                );
                return None;
            }
        }

        let cmd_list = alloc::<ahci_cmd_list>(1024);
        let cmd_list_addr = H::virt_to_phys(cmd_list.as_raw_ptr().addr().get());
        debug!(
            "Port {i} cmd_list va={:#x} pa={:#x}",
            cmd_list.as_raw_ptr().addr().get(),
            cmd_list_addr
        );
        port.CLB().write(cmd_list_addr as u32);
        port.CLBU().write((cmd_list_addr >> 32) as u32);

        let fis = alloc::<ahci_rx_fis>(256);
        let fis_addr = H::virt_to_phys(fis.as_raw_ptr().addr().get());
        debug!(
            "Port {i} fis va={:#x} pa={:#x}",
            fis.as_raw_ptr().addr().get(),
            fis_addr
        );
        port.FB().write(fis_addr as u32);
        port.FBU().write((fis_addr >> 32) as u32);

        let cmd_tbl = alloc::<ahci_cmd_tbl>(128);
        debug!(
            "Port {i} cmd_tbl va={:#x} pa={:#x}",
            cmd_tbl.as_raw_ptr().addr().get(),
            H::virt_to_phys(cmd_tbl.as_raw_ptr().addr().get())
        );

        // Note: We used to check for BSY/DRQ here, but some devices (like QEMU)
        // might be busy after spin-up/link-up. The original driver for reference
        // proceeds to start the port without waiting for BSY to clear here.
        // It waits for BSY to clear *after* setting the start bits.

        port.CMD().write(
            PxCMD::new()
                .with_ICC(ICC::Active)
                .with_FRE(true)
                .with_POD(true)
                .with_SUD(true)
                .with_ST(true),
        );

        if !wait_until_timeout::<H>(
            || {
                let tfd = port.TFD().read();
                if tfd.STS_ERR() {
                    // warn!("Port {i} error after start (TFD: {:?})", tfd);
                }
                !(tfd.STS_ERR() | tfd.STS_DRQ() | tfd.STS_BSY())
            },
            1000,   //try not to wait too long
        ) {
            warn!("Port {i} start timeout (TFD: {:?})", port.TFD().read());
            return None;
        }

        Some(Self {
            port,
            cmd_list,
            fis,
            cmd_tbl,
            _h: PhantomData,
        })
    }

    fn exec_cmd(&mut self, cfis: sata_fis_h2d, buf: *mut [u8], is_write: bool) {
        // Always use slot 0 for simplicity (like reference driver)
        let slot: u32 = 0;

        // Wait for slot 0 to be free
        if !wait_until_timeout::<H>(|| self.port.CI().read() & 1 == 0, 1000) {
            error!("Slot 0 busy timeout");
            return;
        }

        if buf.len() > AHCI_MAX_BYTES_PER_CMD {
            error!("Exceeding max transfer data limit");
            return;
        }

        // Write command FIS to command table
        self.cmd_tbl.hdr().write(cfis);

        let sg_cnt = if !buf.is_null() && !buf.is_empty() {
            let sg_cnt = ((buf.len() - 1) / AHCI_MAX_BYTES_PER_SG) + 1;
            if sg_cnt > AHCI_MAX_SG {
                error!("Exceeding max sg limit");
                return;
            }

            let mut remaining = buf.len();
            for i in 0..sg_cnt {
                let offset = i * AHCI_MAX_BYTES_PER_SG;
                let len = remaining.min(AHCI_MAX_BYTES_PER_SG);

                let buf_addr = H::virt_to_phys(unsafe { (buf as *mut u8).add(offset).addr() });
                let sg = unsafe { &mut self.cmd_tbl.sgs().map(|sg| sg.cast::<ahci_sg>().add(i)) };
                sg.write(ahci_sg {
                    addr_lo: buf_addr as u32,
                    addr_hi: (buf_addr >> 32) as u32,
                    flags_size: (len - 1) as u32 & 0x3fffff, // DBC: Data Byte Count (0-based)
                    ..Default::default()
                });

                remaining -= len;
            }

            sg_cnt
        } else {
            0
        };

        // Build command header options:
        // Bits 0-4: Command FIS length in DWORDs (5 for sata_fis_h2d which is 20 bytes
        // = 5 DWORDs) Bit 6: Write (1) or Read (0)
        // Bits 16-31: PRDTL (Physical Region Descriptor Table Length)
        let cfl = size_of::<sata_fis_h2d>() / 4; // 20 / 4 = 5
        let opts = (cfl as u32) | ((sg_cnt as u32) << 16) | ((is_write as u32) << 6);

        let cmd_tbl_addr = H::virt_to_phys(self.cmd_tbl.as_raw_ptr().addr().get());

        debug!(
            "exec_cmd: slot={} opts={:#x} cmd_tbl_addr={:#x} sg_cnt={} buf_len={}",
            slot,
            opts,
            cmd_tbl_addr,
            sg_cnt,
            buf.len()
        );

        // Write command header to slot 0
        unsafe {
            self.cmd_list
                .map(|list| list.cast::<ahci_cmd_hdr>().add(slot as usize))
        }
        .write(ahci_cmd_hdr {
            opts,
            status: 0,
            tbl_addr_lo: cmd_tbl_addr as u32,
            tbl_addr_hi: (cmd_tbl_addr >> 32) as u32,
            reserved: [0; 4],
        });

        H::flush_dcache();

        // Issue command
        self.port.CI().write(1 << slot);

        // Wait for completion
        if !wait_until_timeout::<H>(|| self.port.CI().read() & (1 << slot) == 0, 1000) {
            let is = self.port.IS().read();
            let tfd = self.port.TFD().read();
            error!(
                "AHCI command timeout: CI={:#x} IS={:?} TFD={:?}",
                self.port.CI().read(),
                is,
                tfd
            );
        }

        H::flush_dcache();
    }
}

pub struct AhciDriver<H> {
    #[allow(dead_code)]
    mmio: VolatilePtr<'static, AhciMmio>,
    port: AhciPort<H>,

    block_size: usize,
    max_lba: u64,
    is_lba48: bool,

    _h: PhantomData<H>,
}

impl<H: Hal> AhciDriver<H> {
    /// Try to construct a new AHCI driver from the given MMIO base address.
    ///
    /// # Safety
    ///
    /// The caller must ensure that:
    /// - `base` is a valid virtual address pointing to the AHCI controller's MMIO register block.
    /// - The memory region starting at `base` is properly mapped and accessible.
    /// - No other code is concurrently accessing the same AHCI controller.
    /// - The AHCI controller hardware is present and functional at the given address.
    pub unsafe fn try_new(base: usize) -> Option<Self> {
        // SAFETY: The caller guarantees `base` is a valid AHCI MMIO base address.
        let mmio = unsafe { VolatilePtr::new(NonNull::new(base as *mut _).unwrap()) };
        let host = mmio.host();

        // reset ahci controller
        host.ghc().update(|mut ghc| {
            if !ghc.HR() {
                ghc.set_HR(true);
            }
            ghc
        });
        if !wait_until_timeout::<H>(|| !host.ghc().read().HR(), 1000) {
            error!("AHCI HBA reset timeout");
            return None;
        }

        // enable ahci
        host.ghc().update(|ghc| ghc.with_AE(true));
        wait_until_timeout::<H>(|| false, 1);

        // init cap and pi
        host.cap().write(CAP::new().with_SMPS(true).with_SSS(true));
        host.pi().write(0xf);

        let vs = host.vs().read();
        info!("AHCI ver {vs}");

        let cap = host.cap().read();
        info!("AHCI cap {cap}");

        let cap2 = host.cap2().read();
        info!("AHCI cap2 {cap2:?}");

        let pi = host.pi().read();
        info!("AHCI ports implemented {pi}");

        host.ghc().update(|ghc| ghc.with_IE(true));

        let mut port = None;
        for i in 0..cap.NP() + 1 {
            if let Some(p) = AhciPort::<H>::try_new(&mmio, i) {
                port = Some(p);
            }
        }

        let Some(mut port) = port else {
            error!("No AHCI ports initialized");
            return None;
        };

        let mut id = [0u16; ATA_ID_WORDS];
        port.exec_cmd(
            sata_fis_h2d {
                fis_type: SATA_FIS_TYPE_REGISTER_H2D,
                pm_port_c: 0x80,
                command: ATA_CMD_ID_ATA,
                ..Default::default()
            },
            unsafe {
                core::slice::from_raw_parts_mut(id.as_mut_ptr().cast::<u8>(), size_of_val(&id))
            },
            false,
        );

        let product = ata_id_to_string(&id, ATA_ID_PROD, ATA_ID_PROD_LEN);
        let serial = ata_id_to_string(&id, ATA_ID_SERNO, ATA_ID_SERNO_LEN);
        let rev = ata_id_to_string(&id, ATA_ID_FW_REV, ATA_ID_FW_REV_LEN);

        info!("AHCI device: {product} {serial} {rev}");

        let max_lba = ata_id_n_sectors(&id);
        let is_lba48 = ata_id_has_lba48(&id);
        let block_size = 512;

        Some(Self {
            mmio,
            port,
            block_size,
            max_lba,
            is_lba48,
            _h: PhantomData,
        })
    }

    pub fn capacity(&self) -> u64 {
        self.max_lba
    }

    pub fn block_size(&self) -> usize {
        self.block_size
    }

    pub fn read(&mut self, block_id: u64, buf: &mut [u8]) -> bool {
        self.rw_common(block_id, buf, false)
    }

    pub fn write(&mut self, block_id: u64, buf: &[u8]) -> bool {
        // Cast to mut ptr for internal handling, but we won't modify it if it's write
        let buf_mut =
            unsafe { core::slice::from_raw_parts_mut(buf.as_ptr() as *mut u8, buf.len()) };
        self.rw_common(block_id, buf_mut, true)
    }

    fn rw_common(&mut self, block_id: u64, buf: &mut [u8], is_write: bool) -> bool {
        let mut start = block_id;
        let mut remaining_bytes = buf.len();
        let mut buf_offset = 0;

        while remaining_bytes > 0 {
            let sectors = remaining_bytes.div_ceil(self.block_size);
            let max_sectors = if self.is_lba48 { 65536 } else { 256 };
            let count = sectors.min(max_sectors);
            let byte_count = count * self.block_size;
            let current_bytes = byte_count.min(remaining_bytes);

            // Construct FIS
            let mut fis = sata_fis_h2d {
                fis_type: SATA_FIS_TYPE_REGISTER_H2D,
                pm_port_c: 0x80,
                ..Default::default()
            };

            if self.is_lba48 {
                fis.command = if is_write {
                    ATA_CMD_WRITE_EXT
                } else {
                    ATA_CMD_READ_EXT
                };
                fis.lba_low = start as u8;
                fis.lba_mid = (start >> 8) as u8;
                fis.lba_high = (start >> 16) as u8;
                fis.lba_low_exp = (start >> 24) as u8;
                fis.lba_mid_exp = (start >> 32) as u8;
                fis.lba_high_exp = (start >> 40) as u8;
                fis.device = 0x40; // LBA mode
                fis.sector_count = (count & 0xff) as u8;
                fis.sector_count_exp = ((count >> 8) & 0xff) as u8;
            } else {
                fis.command = if is_write {
                    ATA_CMD_WRITE
                } else {
                    ATA_CMD_READ
                };
                fis.lba_low = start as u8;
                fis.lba_mid = (start >> 8) as u8;
                fis.lba_high = (start >> 16) as u8;
                fis.device = 0x40 | ((start >> 24) as u8 & 0x0f); // LBA mode + top 4 bits
                fis.sector_count = (count & 0xff) as u8;
            }

            let slice = &mut buf[buf_offset..buf_offset + current_bytes];
            self.port.exec_cmd(fis, slice, is_write);

            start += count as u64;
            remaining_bytes -= current_bytes;
            buf_offset += current_bytes;
        }
        true
    }
}
