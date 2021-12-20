use crate::{Condition, Index, Size, WinAPIHatesUsize};

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct Dr7(pub usize);

impl Dr7 {
    #[must_use]
    pub fn enabled_local(self, index: Index) -> bool {
        let local_offset = 2 * index as WinAPIHatesUsize;
        self.0 & (1 << local_offset) != 0
    }

    #[must_use]
    pub fn with_enabled_local(mut self, index: Index, enabled: bool) -> Dr7 {
        let local_offset = 2 * index as WinAPIHatesUsize;
        self.0 &= !(1 << local_offset);
        self.0 |= (enabled as usize) << local_offset;
        self
    }

    #[must_use]
    pub fn enabled_global(self, index: Index) -> bool {
        let global_offset = 1 + 2 * index as WinAPIHatesUsize;
        self.0 & (1 << global_offset) != 0
    }

    #[must_use]
    pub fn with_enabled_global(mut self, index: Index, enabled: bool) -> Dr7 {
        let global_offset = 1 + 2 * index as WinAPIHatesUsize;
        self.0 &= !(1 << global_offset);
        self.0 |= (enabled as usize) << global_offset;
        self
    }

    #[must_use]
    pub fn condition(self, index: Index) -> Condition {
        let cond_offset = 16 + 4 * index as WinAPIHatesUsize;
        Condition::from_bits((self.0 >> cond_offset & 0b11) as u8)
            .expect("Can not be hit since all patterns & 0b11 are valid.")
    }

    #[must_use]
    pub fn with_condition(mut self, index: Index, condition: Condition) -> Dr7 {
        let cond_offset = 16 + 4 * index as WinAPIHatesUsize;
        self.0 &= !(0b11 << cond_offset);
        self.0 |= (condition.as_bits() as usize) << cond_offset;
        self
    }

    #[must_use]
    pub fn size(self, index: Index) -> Size {
        let size_offset = 18 + 4 * index as WinAPIHatesUsize;
        Size::from_bits((self.0 >> size_offset & 0b11) as u8)
            .expect("Can not be hit since all patterns & 0b11 are valid.")
    }

    #[must_use]
    pub fn with_size(mut self, index: Index, size: Size) -> Dr7 {
        let size_offset = 18 + 4 * index as WinAPIHatesUsize;
        self.0 &= !(0b11 << size_offset);
        self.0 |= (size.as_bits() as usize) << size_offset;
        self
    }

    #[must_use]
    pub fn clear_breakpoints(mut self) -> Dr7 {
        self.0 &= 0b00000000000000001111111100000000;
        self
    }

    #[must_use]
    pub fn clear_breakpoint(self, index: Index) -> Dr7 {
        self.with_enabled_local(index, false)
            .with_enabled_global(index, false)
            .with_condition(index, Condition::Execution) // 0b00
            .with_size(index, Size::One) // 0b00
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct Dr6(pub usize);

impl Dr6 {
    /// Returns whether the exception was caused by a hardware breakpoint.
    #[must_use]
    pub fn breakpoint(self) -> bool {
        self.0 & 0b1111 != 0
    }

    /// Returns whether the exception was caused by the hardware breakpoint at a specific index.
    pub fn breakpoint_at(self, index: Index) -> bool {
        self.breakpoints()[index as usize]
    }

    /// Returns an array of bools representing whether the hardware breakpoint at the respective indices triggered the exception.
    ///
    /// Page 581 of [Intel® 64 and IA-32 Architectures Software Developer’s Manual Volume 3 (3A, 3B, 3C & 3D): System Programming Guide](https://www.intel.com/content/dam/www/public/us/en/documents/manuals/64-ia-32-architectures-software-developer-system-programming-manual-325384.pdf):
    ///
    /// **• B0 through B3 (breakpoint conditiondetected) flags (bits 0 through 3)** — Indicates (when set) that its
    /// associated breakpoint condition was met when a debug exception was generated. These flags are set if the
    /// condition described for each breakpoint by the LENn, and R/Wn flags in debug control register DR7 is true.
    /// They may or may not be set if the breakpoint is not enabled by the Ln or the Gn flags in register DR7. Therefore
    /// on a #DB, a debug handler should check only those B0-B3 bits which correspond to an enabled breakpoint.
    #[must_use]
    pub fn breakpoints(self) -> [bool; 4] {
        [
            self.0 & 0b0001 != 0,
            self.0 & 0b0010 != 0,
            self.0 & 0b0100 != 0,
            self.0 & 0b1000 != 0,
        ]
    }

    /// Returns whether the exception was caused by the thread accessing a debug register.
    ///
    /// Page 581 of [Intel® 64 and IA-32 Architectures Software Developer’s Manual Volume 3 (3A, 3B, 3C & 3D): System Programming Guide](https://www.intel.com/content/dam/www/public/us/en/documents/manuals/64-ia-32-architectures-software-developer-system-programming-manual-325384.pdf):
    ///
    /// **• BD (debug register access detected) flag (bit 13)** — Indicates that the next instruction in the instruction
    /// stream accesses one of the debug registers (DR0 through DR7). This flag is enabled when the GD (general
    /// detect) flag in debug control register DR7 is set. See Section 17.2.4, “Debug Control Register (DR7),” for
    /// further explanation of the purpose of this flag.
    #[must_use]
    pub fn debug_register_access(self) -> bool {
        self.0 & 1 << 13 != 0
    }

    /// Returns whether the exception was caused by single-stepping.
    ///
    /// Page 581 of [Intel® 64 and IA-32 Architectures Software Developer’s Manual Volume 3 (3A, 3B, 3C & 3D): System Programming Guide](https://www.intel.com/content/dam/www/public/us/en/documents/manuals/64-ia-32-architectures-software-developer-system-programming-manual-325384.pdf):
    ///
    /// **• BS (single step) flag (bit 14)** — Indicates (when set) that the debug exception was triggered by the single-
    /// step execution mode (enabled with the TF flag in the EFLAGS register). The single-step mode is the highest-
    /// priority debug exception. When the BS flag is set, any of the other debug status bits also may be set.
    #[must_use]
    pub fn single_step(self) -> bool {
        self.0 & 1 << 14 != 0
    }

    /// Returns whether the exception was caused by a task switch.
    ///
    /// Page 581 of [Intel® 64 and IA-32 Architectures Software Developer’s Manual Volume 3 (3A, 3B, 3C & 3D): System Programming Guide](https://www.intel.com/content/dam/www/public/us/en/documents/manuals/64-ia-32-architectures-software-developer-system-programming-manual-325384.pdf):
    ///
    /// **• BT (task switch) flag (bit 15)** — Indicates (when set) that the debug exception resulted from a task switch
    /// where the T flag (debug trap flag) in the TSS of the target task was set. See Section 7.2.1, “Task-State
    /// Segment (TSS),” for the format of a TSS. There is no flagin debug control register DR7 to enable or disable this
    /// exception; the T flag of the TSS is the only enabling flag.
    #[must_use]
    pub fn task_switch(self) -> bool {
        self.0 & 1 << 15 != 0
    }
}
