use alloc::alloc::alloc_zeroed;
use core::{alloc::Layout, marker::PhantomData, ptr::NonNull};

use log::{debug, error, info, warn};
use volatile::VolatilePtr;

use crate::{
    Hal,
    ata::{
        ATA_CMD_ID_ATA, ATA_ID_FW_REV, ATA_ID_FW_REV_LEN, ATA_ID_PROD, ATA_ID_PROD_LEN,
        ATA_ID_SERNO, ATA_ID_SERNO_LEN, ATA_ID_WORDS, SATA_FIS_TYPE_REGISTER_H2D, ata_id_to_string,
    },
    hal::{wait_until, wait_until_timeout},
    mmio::{
        AhciMmio, AhciMmioVolatileFieldAccess, CAP, GenericHostControlVolatileFieldAccess, ICC,
        PortRegisters, PortRegistersVolatileFieldAccess, PxCMD,
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

        // ensure sata is in idle state
        let cmd = port.CMD().read();
        if cmd.CR() || cmd.FR() || cmd.FRE() || cmd.ST() {
            cmd.with_ST(false);
            port.CMD().write(cmd);
            wait_until(|| !port.CMD().read().CR());
        }

        // spin up
        port.CMD().update(|cmd| cmd.with_SUD(true));
        if !wait_until_timeout::<H>(|| port.CMD().read().SUD(), 1000) {
            warn!("Port {i} set Spin-Up Device timeout");
            return None;
        }

        // port link up
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

        // clear serr
        port.SERR().update(|e| e);
        // ack any pending irq events for this port
        port.IS().update(|i| i);

        host.host().is().write(1 << i);

        if port.SSTS().read().DET() != 3 {
            warn!("Port {i} physical link not established");
            return None;
        }

        let cmd_list = alloc::<ahci_cmd_list>(1024);
        let cmd_list_addr = H::virt_to_phys(cmd_list.as_raw_ptr().addr().get());
        port.CLB().write(cmd_list_addr as u32);
        port.CLBU().write((cmd_list_addr >> 32) as u32);

        let fis = alloc::<ahci_rx_fis>(256);
        let fis_addr = H::virt_to_phys(fis.as_raw_ptr().addr().get());
        port.FB().write(fis_addr as u32);
        port.FBU().write((fis_addr >> 32) as u32);

        let cmd_tbl = alloc::<ahci_cmd_tbl>(256);

        port.CMD().write(
            PxCMD::new()
                .with_ICC(ICC::Active)
                .with_FR(true)
                .with_POD(true)
                .with_SUD(true)
                .with_ST(true),
        );

        if !wait_until_timeout::<H>(
            || {
                let tfd = port.TFD().read();
                debug!("Port {i} TFD: {tfd:?}");
                !(tfd.STS_ERR() | tfd.STS_DRQ() | tfd.STS_BSY())
            },
            u64::MAX,
        ) {
            warn!("Port {i} start timeout");
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
        let ci = self.port.CI().read();
        let slot = ci.trailing_ones();
        if slot == 32 {
            error!("No available slot");
            return;
        }

        if buf.len() > AHCI_MAX_BYTES_PER_CMD {
            error!("Exceeding max transfer data limit");
            return;
        }

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
                    flags_size: 0x3fffff | (len - 1) as u32, // 0x3fffff means last sg
                    ..Default::default()
                });

                remaining -= len;
            }

            sg_cnt
        } else {
            0
        };

        let opts = (size_of::<sata_fis_h2d>() as u64 >> 2
            | (sg_cnt << 16) as u64
            | ((is_write as u64) << 6)) as u32;

        let cmd_tbl_addr = H::virt_to_phys((&raw const self.cmd_tbl).addr());

        unsafe {
            self.cmd_list
                .map(|list| list.cast::<ahci_cmd_hdr>().add(slot as usize))
        }
        .write(ahci_cmd_hdr {
            opts,
            tbl_addr_lo: cmd_tbl_addr as u32,
            tbl_addr_hi: (cmd_tbl_addr >> 32) as u32,
            ..Default::default()
        });

        H::flush_dcache();

        self.port.CI().write(1 << slot);
        wait_until(|| self.port.CI().read() & (1 << slot) == 0);

        H::flush_dcache();
    }
}

pub struct AhciDriver<H> {
    mmio: VolatilePtr<'static, AhciMmio>,
    port: AhciPort<H>,

    _h: PhantomData<H>,
}

impl<H: Hal> AhciDriver<H> {
    pub fn try_new(base: usize) -> Option<Self> {
        let mmio = unsafe { VolatilePtr::new(NonNull::new(base as *mut _).unwrap()) };
        let host = mmio.host();

        // reset ahci controller
        host.ghc().update(|mut ghc| {
            if !ghc.HR() {
                ghc.set_HR(true);
            }
            ghc
        });
        wait_until(|| !host.ghc().read().HR());

        // enable ahci
        host.ghc().update(|ghc| ghc.with_AE(true));

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

        Some(Self {
            mmio,
            port,
            _h: PhantomData,
        })
    }
}
