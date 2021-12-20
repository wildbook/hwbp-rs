use crate::{Condition, Index, Size};

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct EFlags<T>(pub T);

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct Dr6<T>(pub T);

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct Dr7<T>(pub T);

macro_rules! impl_eflags {
    ($( $type:ty ),* ) => {$(
        impl EFlags<$type> {
            #[inline(always)] fn read(&self) -> $type { self.0 }
            #[inline(always)] fn write(&mut self, value: $type) { self.0 = value; }
        }

        impl EFlags<&mut $type> {
            #[inline(always)] fn read(&self) -> $type { *self.0 }
            #[inline(always)] fn write(&mut self, value: $type) { *self.0 = value; }
        }

        impl EFlags<&$type> {
            #[inline(always)] fn read(&self) -> $type { *self.0 }
        }

        impl_eflags!(@READ  $type => $type, &mut $type, &$type);
        impl_eflags!(@WRITE $type => $type, &mut $type);
    )*};

    (@READ $inner_type:ty => $( $type:ty ),*) => {$(
        impl EFlags<$type> {
            /// Returns whether the trap flag is set.
            #[must_use]
            pub fn trap(&self) -> bool {
                self.read() & 1 << 8 != 0
            }

            /// Returns whether the resume flag is set.
            #[must_use]
            pub fn resume(&self) -> bool {
                self.read() & 1 << 16 != 0
            }
        }
    )*};

    (@WRITE $inner_type:ty => $( $type:ty ),*) => {$(
        impl EFlags<$type> {
            /// Sets the trap flag.
            pub fn set_trap(&mut self, value: bool) {
                self.write(self.read() | (value as $inner_type) << 8);
            }

            /// Sets the resume flag.
            pub fn set_resume(&mut self, value: bool) {
                self.write(self.read() | (value as $inner_type) << 16);
            }
        }
    )*};
}

macro_rules! impl_dr6 {
    ($( $type:ty ),* ) => {$(
        impl Dr6<$type> {
            #[inline(always)] fn read(&self) -> $type { self.0 }
            #[inline(always)] fn write(&mut self, value: $type) { self.0 = value; }
        }

        impl Dr6<&mut $type> {
            #[inline(always)] fn read(&self) -> $type { *self.0 }
            #[inline(always)] fn write(&mut self, value: $type) { *self.0 = value; }
        }

        impl Dr6<&$type> {
            #[inline(always)] fn read(&self) -> $type { *self.0 }
        }

        impl_dr6!(@READ  $type => $type, &mut $type, &$type);
        impl_dr6!(@WRITE $type => $type, &mut $type);
    )*};

    (@READ $inner_type:ty => $( $type:ty ),*) => {$(
        impl Dr6<$type> {
            /// Returns whether the exception was caused by a hardware breakpoint.
            #[must_use]
            pub fn breakpoint(&self) -> bool {
                self.read() & 0b1111 != 0
            }

            /// Returns whether the exception was caused by the hardware breakpoint at a specific index.
            pub fn breakpoint_at(&self, index: Index) -> bool {
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
            pub fn breakpoints(&self) -> [bool; 4] {
                [
                    self.read() & 0b0001 != 0,
                    self.read() & 0b0010 != 0,
                    self.read() & 0b0100 != 0,
                    self.read() & 0b1000 != 0,
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
            pub fn debug_register_access(&self) -> bool {
                self.read() & 1 << 13 != 0
            }

            /// Returns whether the exception was caused by single-stepping.
            ///
            /// Page 581 of [Intel® 64 and IA-32 Architectures Software Developer’s Manual Volume 3 (3A, 3B, 3C & 3D): System Programming Guide](https://www.intel.com/content/dam/www/public/us/en/documents/manuals/64-ia-32-architectures-software-developer-system-programming-manual-325384.pdf):
            ///
            /// **• BS (single step) flag (bit 14)** — Indicates (when set) that the debug exception was triggered by the single-
            /// step execution mode (enabled with the TF flag in the EFLAGS register). The single-step mode is the highest-
            /// priority debug exception. When the BS flag is set, any of the other debug status bits also may be set.
            #[must_use]
            pub fn single_step(&self) -> bool {
                self.read() & 1 << 14 != 0
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
            pub fn task_switch(&self) -> bool {
                self.read() & 1 << 15 != 0
            }
        }
    )*};

    (@WRITE $inner_type:ty => $( $type:ty ),*) => {$(
        impl Dr6<$type> {
            /// Resets Dr6 and returns the previous value, leaving only bit 16 set.
            ///
            /// Page 581 of [Intel® 64 and IA-32 Architectures Software Developer’s Manual Volume 3 (3A, 3B, 3C & 3D): System Programming Guide](https://www.intel.com/content/dam/www/public/us/en/documents/manuals/64-ia-32-architectures-software-developer-system-programming-manual-325384.pdf):
            ///
            /// • **RTM (restricted transactional memory) flag (bit 16)** — Indicates (when **clear**) that a debug exception
            ///   (#DB) or breakpoint exception (#BP) occurred inside an RTM region while advanced debugging of RTM trans-
            ///   actional regions was enabled (see Section 17.3.3). This bit is set for any other debug exception (including all
            ///   those that occur when advanced debugging of RTM transactional regions is not enabled). This bit is always 1 if
            ///   the processor does not support RTM.
            pub fn reset(&mut self) -> $inner_type {
                let old = self.read();
                self.write(1 << 16);
                old
            }
        }
    )*};
}

macro_rules! impl_dr7 {
    ($( $type:ty ),* ) => {$(
        impl Dr7<$type> {
            #[inline(always)] fn read(&self) -> $type { self.0 }
            #[inline(always)] fn write(&mut self, value: $type) { self.0 = value; }
        }

        impl Dr7<&mut $type> {
            #[inline(always)] fn read(&self) -> $type { *self.0 }
            #[inline(always)] fn write(&mut self, value: $type) { *self.0 = value; }
        }

        impl Dr7<&$type> {
            #[inline(always)] fn read(&self) -> $type { *self.0 }
        }

        impl_dr7!(@READ  $type => $type, &mut $type, &$type);
        impl_dr7!(@WRITE $type => $type, &mut $type);
    )*};

    (@READ $inner_type:ty => $( $type:ty ),*) => {$(
        impl Dr7<$type> {
            #[must_use]
            pub fn enabled_local(&self, index: Index) -> bool {
                let local_offset = 2 * index as $inner_type;
                self.read() & (1 << local_offset) != 0
            }

            #[must_use]
            pub fn enabled_global(&self, index: Index) -> bool {
                let global_offset = 1 + 2 * index as $inner_type;
                self.read() & (1 << global_offset) != 0
            }

            #[must_use]
            pub fn condition(&self, index: Index) -> Condition {
                let cond_offset = 16 + 4 * index as $inner_type;
                Condition::from_bits((self.read() >> cond_offset & 0b11) as u8)
                    .expect("Can not be hit since all patterns & 0b11 are valid.")
            }

            #[must_use]
            pub fn size(&self, index: Index) -> Size {
                let size_offset = 18 + 4 * index as $inner_type;
                Size::from_bits((self.read() >> size_offset & 0b11) as u8)
                    .expect("Can not be hit since all patterns & 0b11 are valid.")
            }
        }
    )*};

    (@WRITE $inner_type:ty => $( $type:ty ),*) => {$(
        impl Dr7<$type> {
            pub fn set_enabled_local(&mut self, index: Index, enabled: bool) {
                let local_offset = 2 * index as $inner_type;
                self.write(self.read() & !(1 << local_offset));
                self.write(self.read() | (enabled as $inner_type) << local_offset);
            }

            pub fn set_enabled_global(&mut self, index: Index, enabled: bool) {
                let global_offset = 1 + 2 * index as $inner_type;
                self.write(self.read() & !(1 << global_offset));
                self.write(self.read() | (enabled as $inner_type) << global_offset);
            }

            pub fn set_condition(&mut self, index: Index, condition: Condition) {
                let cond_offset = 16 + 4 * index as $inner_type;
                self.write(self.read() & !(0b11 << cond_offset));
                self.write(self.read() | (condition.as_bits() as $inner_type) << cond_offset);
            }

            pub fn set_size(&mut self, index: Index, size: Size) {
                let size_offset = 18 + 4 * index as $inner_type;
                self.write(self.read() & !(0b11 << size_offset));
                self.write(self.read() | (size.as_bits() as $inner_type) << size_offset);
            }

            pub fn clear_breakpoints(&mut self) {
                self.write(self.read() & 0b00000000000000001111111100000000);
            }

            pub fn clear_breakpoint(&mut self, index: Index) {
                self.set_enabled_local(index, false);
                self.set_enabled_global(index, false);
                self.set_condition(index, Condition::Execution);
                self.set_size(index, Size::One);
            }
        }
    )*};
}

impl_eflags!(u32);
impl_dr6!(usize, u32, u64);
impl_dr7!(usize, u32, u64);
