pub trait Hal {
    /// Convert a virtual address to a physical address.
    fn virt_to_phys(va: usize) -> usize;

    /// Current time in milliseconds
    fn current_ms() -> u64;

    /// Flush the Dcache.
    fn flush_dcache();
}

pub(crate) fn wait_until(cond: impl Fn() -> bool) {
    while !cond() {
        core::hint::spin_loop();
    }
}

pub(crate) fn wait_until_timeout<H: Hal>(cond: impl Fn() -> bool, timeout: u64) -> bool {
    let start = H::current_ms();
    loop {
        if cond() {
            return true;
        }
        if H::current_ms() - start > timeout {
            return false;
        }
        core::hint::spin_loop();
    }
}
