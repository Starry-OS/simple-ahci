#![allow(non_snake_case, clippy::upper_case_acronyms)]

use core::fmt;

use bitfield_struct::bitfield;
use volatile::{VolatileFieldAccess, access::ReadOnly};

#[derive(VolatileFieldAccess)]
#[repr(C)]
pub struct AhciMmio {
    pub host: GenericHostControl,
    _res: [u8; 0xd0],
    pub ports: [PortRegisters; 32],
}

const _: () = assert!(core::mem::offset_of!(AhciMmio, ports) == 0x100);

#[derive(VolatileFieldAccess)]
#[repr(C)]
pub struct GenericHostControl {
    /// CAP – HBA Capabilities
    ///
    /// This register indicates basic capabilities of the HBA to driver
    /// software.
    pub cap: CAP,

    /// GHC – Global HBA Control
    ///
    /// This register controls the overall operation of the HBA.
    pub ghc: GHC,

    /// IS – Interrupt Status Register
    ///
    /// This register indicates which of the ports within the controller have an
    /// interrupt pending and require service.
    ///
    /// # Interrupt Pending Status (IPS)
    ///
    /// If set, indicates that the corresponding
    /// port has an interrupt pending. Software can use this information to
    /// determine which ports require service after an interrupt.
    ///
    /// The IPS[x] bit is only defined for ports that are implemented or for the
    /// command completion coalescing interrupt defined by CCC_CTL.INT. All
    /// other bits are reserved.
    pub is: u32,

    /// PI – Ports Implemented
    ///
    /// This register indicates which ports are exposed by the HBA. It is loaded
    /// by the BIOS. It indicates which ports that the HBA supports are
    /// available for software to use. For example, on an HBA that supports 6
    /// ports as indicated in CAP.NP, only ports 1 and 3 could be available,
    /// with ports 0, 2, 4, and 5 being unavailable.
    ///
    /// Software must not read or write to registers within unavailable ports.
    ///
    /// The intent of this register is to allow system vendors to build
    /// platforms that support less than the full number of ports
    /// implemented on the HBA silicon.
    ///
    /// # Port Implemented (PI)
    ///
    /// This register is bit significant. If a bit is set to ‘1’, the
    /// corresponding port is available for software to use. If a bit is cleared
    /// to ‘0’, the port is not available for software to use. The maximum
    /// number of bits set to ‘1’ shall not exceed CAP.NP + 1, although the
    /// number of bits set in this register may be fewer than CAP.NP + 1. At
    /// least one bit shall be set to ‘1’.
    pub pi: u32,

    /// VS – AHCI Version
    #[access(ReadOnly)]
    pub vs: VS,

    /// CCC_CTL – Command Completion Coalescing Control
    ///
    /// Unused
    pub ccc_ctl: u32,

    /// CCC_PORTS - Command Completion Coalescing Ports
    ///
    /// Unused
    pub ccc_ports: u32,

    /// EM_LOC – Enclosure Management Location
    ///
    /// Unused
    #[access(ReadOnly)]
    pub em_loc: u32,

    /// EM_CTL – Enclosure Management Control
    ///
    /// Unused
    pub em_ctl: u32,

    /// CAP2 – HBA Capabilities Extended
    ///
    /// This register indicates capabilities of the HBA to driver software.
    #[access(ReadOnly)]
    pub cap2: CAP2,
}

/// CAP – HBA Capabilities
///
/// This register indicates basic capabilities of the HBA to driver
/// software.
#[bitfield(u32, order = Msb)]
pub struct CAP {
    /// Supports 64-bit Addressing (S64A):
    ///
    /// Indicates whether the HBA can access 64-bit data structures.
    /// When set to ‘1’, the HBA shall make the 32-bit upper bits of the
    /// port DMA Descriptor, the PRD Base, and each PRD entry read/write.
    /// When cleared to ‘0’, these are read-only and treated as ‘0’ by the
    /// HBA.
    pub S64A: bool,

    /// Supports Native Command Queuing (SNCQ):
    ///
    /// Indicates whether the HBA supports Serial ATA native command
    /// queuing. If set to ‘1’, an HBA shall handle DMA Setup FISes
    /// natively, and shall handle the auto-activate optimization through
    /// that FIS. If cleared to ‘0’, native command queuing is not
    /// supported and software should not issue any native command queuing
    /// commands.
    pub SNCQ: bool,

    /// Supports SNotification Register (SSNTF):
    ///
    /// When set to ‘1’, the HBA supports the PxSNTF (SNotification)
    /// register and its associated functionality. When cleared to ‘0’, the
    /// HBA does not support the PxSNTF register and its associated
    /// functionality. Asynchronous notification with a directly attached
    /// device is always supported.
    pub SSNTF: bool,

    /// Supports Mechanical Presence Switch (SMPS):
    ///
    /// When set to ‘1’, the HBA supports mechanical presence switches on
    /// its ports for use in hot plug operations. When cleared to ‘0’, this
    /// function is not supported. This value is loaded by the BIOS prior
    /// to OS initialization.
    pub SMPS: bool,

    /// Supports Staggered Spin-up (SSS):
    ///
    /// When set to ‘1’, the HBA supports staggered spin-up on its ports,
    /// for use in balancing power spikes. When cleared to ‘0’, this
    /// function is not supported. This value is loaded by the BIOS prior
    /// to OS initialization.
    pub SSS: bool,

    /// Supports Aggressive Link Power Management (SALP):
    ///
    /// When set to ‘1’, the HBA can support auto-generating link
    /// requests to the Partial or Slumber states when there are no
    /// commands to process. When cleared to ‘0’, this function is not
    /// supported and software shall treat the PxCMD.ALPE and PxCMD.ASP
    /// bits as reserved.
    pub SALP: bool,

    /// Supports Activity LED (SAL):
    ///
    /// When set to ‘1’, the HBA supports a single activity indication
    /// output pin. This pin can be connected to an LED on the platform to
    /// indicate device activity on any drive. When cleared to ‘0’, this
    /// function is not supported.
    pub SAL: bool,

    /// Supports Command List Override (SCLO):
    ///
    /// When set to ‘1’, the HBA supports the PxCMD.CLO bit and its
    /// associated function. When cleared to ‘0’, the HBA is not capable
    /// of clearing the BSY and DRQ bits in the Status register in order
    /// to issue a software reset if these bits are still set from a
    /// previous operation.
    pub SCLO: bool,

    /// Interface Speed Support (ISS)
    ///
    /// Indicates the maximum speed the HBA can support on its ports.
    /// These encodings match the system software programmable
    /// PxSCTL.DET.SPD field.
    #[bits(4)]
    pub ISS: ISS,

    pub __: bool,

    /// Supports AHCI mode only (SAM):
    ///
    /// The SATA controller may optionally support AHCI access
    /// mechanisms only. When set to '1', the SATA controller does not
    /// implement a legacy, task-file based register interface (e.g.,
    /// SFF-8038i). When cleared to '0', in addition to the native AHCI
    /// mechanism (via ABAR), the controller also implements a legacy,
    /// task-file based register interface.
    pub SAM: bool,

    /// Supports Port Multiplier (SPM):
    ///
    /// Indicates whether the HBA can support a Port Multiplier. When
    /// set, a Port Multiplier using command-based switching is supported
    /// and FIS-based switching may be supported. When cleared to '0', a
    /// Port Multiplier is not supported, and a Port Multiplier may not be
    /// attached to this HBA.
    pub SPM: bool,

    /// FIS-based Switching Supported (FBSS):
    ///
    /// When set to '1', indicates that the HBA supports Port Multiplier
    /// FIS-based switching. When cleared to '0', indicates that the HBA
    /// does not support FIS-based switching. This bit shall only be set
    /// to '1' if the SPM bit is set.
    pub FBSS: bool,

    /// PIO Multiple DRQ Block (PMD):
    ///
    /// If set to '1', the HBA supports multiple DRQ block data
    /// transfers for the PIO command protocol. If cleared to '0' the HBA
    /// only supports single DRQ block data transfers for the PIO command
    /// protocol. AHCI 1.2 HBAs shall have this bit set to '1'.
    pub PMD: bool,

    /// Slumber State Capable (SSC):
    ///
    /// Indicates whether the HBA can support transitions to the Slumber
    /// state. When cleared to '0', software must not allow the HBA to
    /// initiate transitions to the Slumber state via aggressive link
    /// power management nor the PxCMD.ICC field in each port, and the
    /// PxSCTL.IPM field in each port must be programmed to disallow
    /// device-initiated Slumber requests. When set to '1', HBA- and
    /// device-initiated Slumber requests can be supported.
    pub SSC: bool,

    /// Partial State Capable (PSC):
    ///
    /// Indicates whether the HBA can support transitions to the Partial
    /// state. When cleared to '0', software must not allow the HBA to
    /// initiate transitions to the Partial state via aggressive link
    /// power management nor the PxCMD.ICC field in each port, and the
    /// PxSCTL.IPM field in each port must be programmed to disallow
    /// device-initiated Partial requests. When set to '1', HBA- and
    /// device-initiated Partial requests can be supported.
    pub PSC: bool,

    /// Number of Command Slots (NCS):
    ///
    /// 0's-based value indicating the number of command slots per port
    /// supported by this HBA. A minimum of 1 and maximum of 32 slots per
    /// port can be supported. The same number of command slots is
    /// available on each implemented port.
    #[bits(5)]
    pub NCS: u8,

    /// Command Completion Coalescing Supported (CCCS):
    ///
    /// When set to '1', indicates that the HBA supports command
    /// completion coalescing. When supported, the HBA implements the
    /// CCC_CTL and CCC_PORTS global HBA registers. When cleared to '0',
    /// indicates that the HBA does not support command completion
    /// coalescing and those registers are not implemented.
    pub CCCS: bool,

    /// Enclosure Management Supported (EMS):
    ///
    /// When set to '1', indicates that the HBA supports enclosure
    /// management as defined in the specification. When supported, the
    /// HBA implements the EM_LOC and EM_CTL global HBA registers. When
    /// cleared to '0', enclosure management is not supported and those
    /// registers are not implemented.
    pub EMS: bool,

    /// Supports External SATA (SXS):
    ///
    /// When set to '1', indicates that the HBA has one or more ports with
    /// a signal only connector that is externally accessible (e.g.,
    /// eSATA). If this bit is '1', software may refer to PxCMD.ESP to
    /// determine whether a specific port has an externally accessible
    /// signal-only connector. When '0', indicates no such ports are
    /// present.
    pub SXS: bool,

    /// Number of Ports (NP):
    ///
    /// 0's-based value indicating the maximum number of ports supported
    /// by the HBA silicon. A maximum of 32 ports can be supported. A
    /// value of 0h indicates one port (minimum requirement). Note: This
    /// may be greater than the number of ports indicated in the PI
    /// register.
    #[bits(5)]
    pub NP: u8,
}

/// The maximum speed the HBA can support on its ports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ISS {
    /// Reserved
    Reserved = 0,
    /// Gen 1 (1.5 Gbps)
    Gen1     = 1,
    /// Gen 2 (3 Gbps)
    Gen2     = 2,
    /// Gen 3 (6 Gbps)
    Gen3     = 3,
}

impl ISS {
    pub const fn into_bits(self) -> u8 {
        self as _
    }

    pub const fn from_bits(bits: u8) -> Self {
        match bits {
            1 => ISS::Gen1,
            2 => ISS::Gen2,
            3 => ISS::Gen3,
            _ => ISS::Reserved,
        }
    }
}

impl fmt::Display for ISS {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ISS::Reserved => write!(f, "Reserved"),
            ISS::Gen1 => write!(f, "Gen 1 (1.5 Gbps)"),
            ISS::Gen2 => write!(f, "Gen 2 (3 Gbps)"),
            ISS::Gen3 => write!(f, "Gen 3 (6 Gbps)"),
        }
    }
}

impl fmt::Display for CAP {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.S64A() {
            write!(f, "64bit, ")?;
        }
        if self.SNCQ() {
            write!(f, "Native Command Queuing, ")?;
        }
        if self.SSNTF() {
            write!(f, "SNotification, ")?;
        }
        if self.SMPS() {
            write!(f, "Mechanical Presence Switch, ")?;
        }
        if self.SSS() {
            write!(f, "Staggered Spin-up, ")?;
        }
        if self.SALP() {
            write!(f, "Aggressive Link Power Management, ")?;
        }
        if self.SAL() {
            write!(f, "Activity LED, ")?;
        }
        if self.SCLO() {
            write!(f, "Command List Override, ")?;
        }
        if self.SAM() {
            write!(f, "AHCI only, ")?;
        }
        if self.SPM() {
            write!(f, "Port Multiplier, ")?;
        }
        if self.FBSS() {
            write!(f, "FIS-based Switching, ")?;
        }
        if self.PMD() {
            write!(f, "Multiple DRQ Block, ")?;
        }
        if self.SSC() {
            write!(f, "Slumber State Capable, ")?;
        }
        if self.PSC() {
            write!(f, "Partial State Capable, ")?;
        }
        if self.CCCS() {
            write!(f, "Command Completion Coalescing, ")?;
        }
        if self.EMS() {
            write!(f, "Enclosure Management, ")?;
        }
        if self.SXS() {
            write!(f, "External SATA, ")?;
        }
        write!(f, "Command slots: {}, ", self.NCS() + 1)?;
        write!(f, "Ports: {}", self.NP() + 1)?;
        Ok(())
    }
}

/// GHC – Global HBA Control
///
/// This register controls various global actions of the HBA.
#[bitfield(u32, order = Msb)]
pub struct GHC {
    /// AHCI Enable (AE):
    ///
    /// When set to '1', communication to the HBA shall be via AHCI
    /// mechanisms only. When cleared to '0', communication shall be via
    /// legacy mechanisms (e.g., SFF-8038i) and FISes are not posted to
    /// memory and no commands are sent via AHCI mechanisms. Software shall
    /// set this bit to '1' before accessing other AHCI registers. When
    /// clearing AE from '1' to '0', software shall write 00000000h to the
    /// entire GHC register (i.e., do not set any other bit in the same
    /// write).
    ///
    /// Implementation note: The reset value and access type of this bit
    /// depend on CAP.SAM. If CAP.SAM == '0', AE is RW and resets to '0'. If
    /// CAP.SAM == '1', AE is RO and resets to '1'.
    pub AE: bool,

    #[bits(28)]
    __: u32,

    /// MSI Revert to Single Message (MRSM):
    ///
    /// When set to '1' by hardware, indicates the HBA requested more than
    /// one MSI vector but has reverted to using the first vector only.
    /// Cleared to '0' when the four conditions are not all true, or when
    /// MC.MSIE=='1' and MC.MME==0h (programmed single MSI mode, not
    /// reverting). Software clears interrupts via the IS register when
    /// this is '1'. This field is set/cleared by hardware.
    pub MRSM: bool,

    /// Interrupt Enable (IE):
    ///
    /// Global interrupt enable. When '0' (reset default), all interrupt
    /// sources from all ports are disabled. When '1', interrupts are
    /// enabled.
    pub IE: bool,

    /// HBA Reset (HR):
    ///
    /// When set to '1' by software, causes an internal reset of the HBA.
    /// All data transfer and queuing state machines return to idle and all
    /// ports are re-initialized via COMRESET (unless staggered spin-up is
    /// supported, in which case software must spin up each port). Hardware
    /// shall clear this bit to '0' when the reset completes. Writing '0'
    /// has no effect.
    pub HR: bool,
}

/// VS – AHCI Version
///
/// This register indicates the major and minor version of the AHCI
/// specification that the HBA implementation supports. The upper two bytes
/// represent the major version number, and the lower two bytes represent
/// the minor version number. Example: Version 3.12 would be represented as
/// 00030102h. Three versions of the specification are valid: 0.95, 1.0, 1.1,
/// 1.2, 1.3, and 1.3.1.
#[bitfield(u32, order = Msb)]
pub struct VS {
    major_h: u8,
    major_l: u8,
    minor_h: u8,
    minor_l: u8,
}

impl fmt::Display for VS {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let major = self.major_h() * 0x10 + self.major_l();
        let minor = self.minor_h() * 0x10 + self.minor_l();
        write!(f, "{major:x}.{minor:x}")
    }
}

/// CAP2 – HBA Capabilities Extended
///
/// This register indicates capabilities of the HBA to driver software.
#[bitfield(u32, order = Msb)]
pub struct CAP2 {
    #[bits(26)]
    __: u32,

    /// DevSleep Entrance from Slumber Only (DESO):
    ///
    /// This field specifies that the HBA
    /// shall only assert the DEVSLP signal if the interface is in Slumber. When
    /// this bit is set to ‘1’, the HBA shall ingnore software directed
    /// entrance to DevSleep via PxCMD.ICC unless PxSSTS.IPM = 6h.
    ///
    /// When this bit is cleared to ‘0’, the HBA may enter DevSleep from any
    /// link state (active, Partial, or Slumber).
    pub DESO: bool,

    /// Supports Aggressive Device Sleep Management (SADM):
    ///
    /// When set to ‘1’, the HBA
    /// supports hardware assertion of the DEVSLP signal after the idle timeout
    /// expires. When cleared to ‘0’, this function is not supported and
    /// software shall treat the PxDEVSLP.ADSE field as reserved. Refer to
    /// section 8.5.1.
    pub SADM: bool,

    /// Supports Device Sleep (SDS):
    ///
    /// When set to ‘1’, the HBA supports the Device Sleep
    /// feature. When cleared to ‘0’, DEVSLP is not supported and software shall
    /// not set PxCMD.ICC to ‘8h’
    pub SDS: bool,

    /// Automatic Partial to Slumber Transitions (APST):
    ///
    /// When set to ‘1’, the HBA supports
    /// Automatic Partial to Slumber Transitions. When cleared to ‘0’, Automatic
    /// Partial to Slumber Transitions are not supported. Please refer to
    /// section 10.16 for more information regarding Automatic Partial to
    /// Slumber transitions.
    pub APST: bool,

    /// NVMHCI Present (NVMP):
    ///
    /// When set to ‘1’, the HBA includes support for NVMHCI and
    /// the registers at offset 60h-9Fh are valid. When cleared to ‘0’, the HBA
    /// does not support NVMHCI. Please refer to section 10.15 for more
    /// information regarding NVMHCI.
    pub NVMP: bool,

    /// BIOS/OS Handoff (BOH):
    ///
    /// When set to ‘1’, the HBA supports the BIOS/OS handoff
    /// mechanism defined in section 10.6. When cleared to ‘0’, the HBA does not
    /// support the BIOS/OS handoff mechanism. When BIOS/OS handoff is
    /// supported, the HBA has implemented the BOHC global HBA register.
    /// When cleared to ‘0’, it indicates that the HBA does not support
    /// BIOS/OS handoff and the BOHC global HBA register is not implemented.
    pub BOH: bool,
}

#[derive(VolatileFieldAccess)]
#[repr(C)]
pub struct PortRegisters {
    /// Command List Base Address.
    CLB: u32,
    /// Command List Base Address Upper 32-Bits.
    CLBU: u32,
    /// FIS Base Address.
    FB: u32,
    /// FIS Base Address Upper 32-Bits.
    FBU: u32,
    /// Interrupt Status.
    pub IS: PxI,
    /// Interrupt Enable.
    pub IE: PxI,
    /// Command and Status.
    pub CMD: PxCMD,
    _res0: u32,
    /// Task File Data.
    #[access(ReadOnly)]
    pub TFD: PxTFD,
    /// Signature.
    #[access(ReadOnly)]
    pub SIG: PxSIG,
    /// Serial ATA Status (SCR0: SStatus).
    pub SSTS: PxSSTS,
    /// Serial ATA Control (SCR2: SControl).
    pub SCTL: u32,
    /// Serial ATA Error (SCR1: SError).
    pub SERR: PxSERR,
    /// Serial ATA Active. (SCR3: SActive).
    pub SACT: u32,
    /// Command Issue.
    pub CI: u32,
    /// Serial ATA Notification (SCR4: SNotification).
    pub SNTF: u32,
    /// FIS-based Switching Control.
    pub FBS: u32,
    /// Device Sleep.
    pub DEVSLP: u8,
    _reserved1: [u8; 0x28],
    /// Vendor Specific.
    pub vs: u128,
}

const _: () = assert!(size_of::<PortRegisters>() == 0x80);

// TODO: document

#[bitfield(u32, order = Msb)]
pub struct PxI {
    pub CPD: bool,
    pub TFE: bool,
    pub HBF: bool,
    pub HBD: bool,
    pub IF: bool,
    pub INF: bool,
    __: bool,
    pub OF: bool,
    pub IPM: bool,
    pub PRC: bool,
    #[bits(14)]
    __: u16,
    pub DMP: bool,
    pub PC: bool,
    pub DP: bool,
    pub UF: bool,
    pub SDB: bool,
    pub DS: bool,
    pub PS: bool,
    pub DHR: bool,
}

impl PxI {
    pub fn default_enable() -> Self {
        Self::new()
            .with_TFE(true)
            .with_HBF(true)
            .with_HBD(true)
            .with_IF(true)
            .with_IPM(true)
            .with_PRC(true)
            .with_PC(true)
            .with_UF(true)
            .with_SDB(true)
            .with_DS(true)
            .with_PS(true)
            .with_DHR(true)
    }
}

#[bitfield(u32, order = Msb)]
pub struct PxCMD {
    #[bits(4)]
    pub ICC: ICC,
    pub ASP: bool,
    pub ALPE: bool,
    pub DLAE: bool,
    pub ATAPI: bool,
    pub APSTE: bool,
    #[bits(access = RO)]
    pub FBSCP: bool,
    #[bits(access = RO)]
    pub ESP: bool,
    #[bits(access = RO)]
    pub CPD: bool,
    #[bits(access = RO)]
    pub MPSP: bool,
    #[bits(access = RO)]
    pub HPCP: bool,
    pub PMA: bool,
    #[bits(access = RO)]
    pub CPS: bool,
    pub CR: bool,
    pub FR: bool,
    #[bits(access = RO)]
    pub MPSS: bool,
    #[bits(5, access = RO)]
    pub CCS: u8,
    #[bits(3)]
    pub __: u8,
    pub FRE: bool,
    pub CLO: bool,
    pub POD: bool,
    pub SUD: bool,
    pub ST: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ICC {
    #[default]
    Idle     = 0x0,
    Active   = 0x1,
    Partial  = 0x2,
    Slumber  = 0x6,
    DevSleep = 0x8,
    Reserved = 0xf,
}

impl ICC {
    pub const fn into_bits(self) -> u8 {
        self as _
    }

    pub const fn from_bits(bits: u8) -> Self {
        match bits {
            0x0 => Self::Idle,
            0x1 => Self::Active,
            0x2 => Self::Partial,
            0x6 => Self::Slumber,
            0x8 => Self::DevSleep,
            0xf => Self::Reserved,
            _ => Self::Reserved,
        }
    }
}

#[bitfield(u32, order = Msb)]
pub struct PxTFD {
    __: u16,
    pub ERR: u8,
    pub STS_BSY: bool,
    #[bits(3)]
    __: u8,
    pub STS_DRQ: bool,
    #[bits(2)]
    __: u8,
    pub STS_ERR: bool,
}

#[bitfield(u32, order = Msb)]
pub struct PxSIG {
    pub high: u8,
    pub mid: u8,
    pub low: u8,
    pub count: u8,
}

#[bitfield(u32, order = Msb)]
pub struct PxSSTS {
    #[bits(20)]
    __: u32,
    #[bits(4)]
    pub IPM: u8,
    #[bits(4)]
    pub SPD: u8,
    #[bits(4)]
    pub DET: u8,
}

#[bitfield(u32, order = Msb)]
pub struct PxSERR {
    #[bits(5)]
    __: u8,
    pub DIAG_X: bool,
    pub DIAG_F: bool,
    pub DIAG_T: bool,
    pub DIAG_S: bool,
    pub DIAG_H: bool,
    pub DIAG_C: bool,
    pub DIAG_D: bool,
    pub DIAG_B: bool,
    pub DIAG_W: bool,
    pub DIAG_I: bool,
    pub DIAG_N: bool,
    #[bits(4)]
    __: u8,
    pub ERR_E: bool,
    pub ERR_P: bool,
    pub ERR_C: bool,
    pub ERR_T: bool,
    #[bits(6)]
    __: u8,
    pub ERR_M: bool,
    pub ERR_I: bool,
}
