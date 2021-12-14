#![cfg(target_os = "windows")]

use std::os::windows::raw::HANDLE;
use std::{
    convert::{TryFrom, TryInto},
    error::Error,
    ffi::c_void,
    fmt::Display,
};
use winapi::um::processthreadsapi::{GetThreadContext, SetThreadContext};
use winapi::um::winnt::{RtlCaptureContext, RtlRestoreContext, CONTEXT, CONTEXT_DEBUG_REGISTERS};

#[cfg(target_pointer_width = "64")]
type WinAPIHatesUsize = u64;

#[cfg(target_pointer_width = "32")]
type WinAPIHatesUsize = u32;

#[allow(non_snake_case)]
fn GetCurrentThread() -> HANDLE {
    #[cfg(not(feature = "avoid_winapi"))]
    return unsafe { winapi::um::processthreadsapi::GetCurrentThread() };

    // GetCurrentThread() only calls NtCurrentThread(), which is hardcoded to always returns -2
    #[cfg(feature = "avoid_winapi")]
    return -2 as _;
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Condition {
    Execution = 0b00,
    Write = 0b01,
    ReadWrite = 0b11,
    IoReadWrite = 0b10,
}

impl TryFrom<u8> for Condition {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            x if x == Condition::Execution as _ => Ok(Condition::Execution),
            x if x == Condition::Write as _ => Ok(Condition::Write),
            x if x == Condition::ReadWrite as _ => Ok(Condition::ReadWrite),
            x if x == Condition::IoReadWrite as _ => Ok(Condition::IoReadWrite),
            _ => Err(()),
        }
    }
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Size {
    One = 0b00,
    Two = 0b01,
    Four = 0b11,
    #[cfg(target_pointer_width = "64")]
    Eight = 0b10,
}

impl TryFrom<u8> for Size {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            x if x == Size::One as _ => Ok(Size::One),
            x if x == Size::Two as _ => Ok(Size::Two),
            x if x == Size::Four as _ => Ok(Size::Four),
            #[cfg(target_pointer_width = "64")]
            x if x == Size::Eight as _ => Ok(Size::Eight),
            _ => Err(()),
        }
    }
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Index {
    First = 0,
    Second = 1,
    Third = 2,
    Fourth = 3,
}

impl TryFrom<u8> for Index {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            x if x == Index::First as _ => Ok(Index::First),
            x if x == Index::Second as _ => Ok(Index::Second),
            x if x == Index::Third as _ => Ok(Index::Third),
            x if x == Index::Fourth as _ => Ok(Index::Fourth),
            _ => Err(()),
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct HardwareBreakpoint {
    pub enabled: bool,
    pub index: Index,
    pub address: *const c_void,
    pub size: Size,
    pub condition: Condition,
}

impl HardwareBreakpoint {
    pub fn new(index: Index) -> Self {
        Self {
            enabled: false,
            index,
            address: std::ptr::null(),
            size: Size::Four,
            condition: Condition::Execution,
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum HwbpError {
    FailedGetThreadContext,
    FailedSetThreadContext,
}

impl Error for HwbpError {}

impl Display for HwbpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FailedGetThreadContext => write!(f, "failed to get thread context"),
            Self::FailedSetThreadContext => write!(f, "failed to set thread context"),
        }
    }
}

#[repr(align(16))]
pub struct HwbpContext(CONTEXT);

impl HwbpContext {
    pub fn from_context(context: CONTEXT) -> HwbpContext {
        HwbpContext(context)
    }

    pub fn into_context(self) -> CONTEXT {
        self.0
    }
}

impl HwbpContext {
    /// Uses GetThreadContext.
    pub fn get() -> Result<HwbpContext, HwbpError> {
        // We're creating a blank context and setting the ContextFlags field before passing it
        // to GetThreadContext, which reads the field and returns the appropriate data
        let mut context: HwbpContext = unsafe { std::mem::zeroed() };
        context.0.ContextFlags = CONTEXT_DEBUG_REGISTERS;

        match unsafe { GetThreadContext(GetCurrentThread(), &mut context.0) } {
            0 => Err(HwbpError::FailedGetThreadContext),
            _ => Ok(context),
        }
    }

    /// Uses RtlCaptureContext.
    pub fn get_rtl() -> HwbpContext {
        // We're creating a blank context and setting the ContextFlags field before passing it
        // to GetThreadContext, which reads the field and returns the appropriate data
        let mut context: HwbpContext = unsafe { std::mem::zeroed() };
        context.0.ContextFlags = CONTEXT_DEBUG_REGISTERS;

        unsafe { RtlCaptureContext(&mut context.0) };
        context
    }

    /// # Safety
    /// This function will never directly cause undefined behaviour, but the breakpoints it can be
    /// used to place will cause exceptions to be thrown when they are hit. Calling this function
    /// is therefore unsafe, as it might affect the program in unexpected ways if the caller doesn't
    /// properly set up some form of exception handling.
    ///
    /// Uses SetThreadContext.
    pub unsafe fn apply(&self) -> Result<(), HwbpError> {
        match SetThreadContext(GetCurrentThread(), &self.0) {
            0 => Err(HwbpError::FailedSetThreadContext),
            _ => Ok(()),
        }
    }

    /// # Safety
    /// This function will never directly cause undefined behaviour, but the breakpoints it can be
    /// used to place will cause exceptions to be thrown when they are hit. Calling this function
    /// is therefore unsafe, as it might affect the program in unexpected ways if the caller doesn't
    /// properly set up some form of exception handling.
    ///
    /// Uses RtlRestoreContext.
    pub unsafe fn apply_rtl(&self) {
        RtlRestoreContext(&self.0 as *const _ as *mut _, std::ptr::null_mut());
    }

    pub fn unused_breakpoint(&self) -> Option<HardwareBreakpoint> {
        raw::unused_breakpoint(&self.0)
    }
    pub fn set_breakpoint(&mut self, bp: &HardwareBreakpoint) {
        raw::set_breakpoint(&mut self.0, bp);
    }
    pub fn get_breakpoint(&self, index: Index) -> HardwareBreakpoint {
        raw::get_breakpoint(&self.0, index)
    }
    pub fn get_breakpoints(&self) -> [HardwareBreakpoint; 4] {
        raw::get_breakpoints(&self.0)
    }
    pub fn get_breakpoints_by_address(&self, address: *const c_void) -> Vec<HardwareBreakpoint> {
        raw::get_breakpoints_by_address(&self.0, address)
    }
    pub fn clear(&mut self) {
        raw::clear(&mut self.0);
    }
}

pub mod raw {
    use super::*;

    pub fn unused_breakpoint(context: &CONTEXT) -> Option<HardwareBreakpoint> {
        [Index::First, Index::Second, Index::Third, Index::Fourth]
            .iter()
            .map(|&index| get_breakpoint(context, index))
            .find(|bp| !bp.enabled)
    }
    pub fn set_breakpoint(context: &mut CONTEXT, bp: &HardwareBreakpoint) {
        let hwbp_index = bp.index as u8;

        match bp.index {
            Index::First => context.Dr0 = bp.address as _,
            Index::Second => context.Dr1 = bp.address as _,
            Index::Third => context.Dr2 = bp.address as _,
            Index::Fourth => context.Dr3 = bp.address as _,
        }

        let when = bp.condition as WinAPIHatesUsize;
        let size = bp.size as WinAPIHatesUsize;

        // Wipe any current setting for hwbp_index' condition, then set it to `when`
        context.Dr7 &= !(0b11 << ((16 + (hwbp_index * 4)) as usize)) as WinAPIHatesUsize;
        context.Dr7 &= !(0b11 << ((16 + (hwbp_index * 4)) as usize)) as WinAPIHatesUsize;
        context.Dr7 |= ((when & 0b10) << ((16 + (hwbp_index * 4)) as usize)) as WinAPIHatesUsize;
        context.Dr7 |= ((when & 0b01) << ((16 + (hwbp_index * 4)) as usize)) as WinAPIHatesUsize;

        // Wipe any current setting for hwbp_index' size, then set it to `size`
        context.Dr7 &= !(0b11 << ((18 + (hwbp_index * 4)) as usize)) as WinAPIHatesUsize;
        context.Dr7 &= !(0b11 << ((18 + (hwbp_index * 4)) as usize)) as WinAPIHatesUsize;
        context.Dr7 |= ((size & 0b10) << ((18 + (hwbp_index * 4)) as usize)) as WinAPIHatesUsize;
        context.Dr7 |= ((size & 0b01) << ((18 + (hwbp_index * 4)) as usize)) as WinAPIHatesUsize;

        // Clear and set enabled status
        context.Dr7 &= !(1 << (hwbp_index * 2));
        context.Dr7 |= (bp.enabled as WinAPIHatesUsize) << (hwbp_index * 2);
    }
    pub fn get_breakpoint(context: &CONTEXT, index: Index) -> HardwareBreakpoint {
        let num_index = index as u8;

        let address = match index {
            Index::First => context.Dr0,
            Index::Second => context.Dr1,
            Index::Third => context.Dr2,
            Index::Fourth => context.Dr3,
        } as _;

        let size = (((context.Dr7 >> (18 + (num_index * 4))) & 0b11) as u8)
            .try_into()
            .unwrap();
        let cond = (((context.Dr7 >> (16 + (num_index * 4))) & 0b11) as u8)
            .try_into()
            .unwrap();

        HardwareBreakpoint {
            enabled: context.Dr7 & (1 << (num_index * 2)) != 0,
            index,
            address,
            size,
            condition: cond,
        }
    }
    pub fn get_breakpoints_by_address(
        context: &CONTEXT,
        address: *const c_void,
    ) -> Vec<HardwareBreakpoint> {
        get_breakpoints(context)
            .iter()
            .copied()
            .filter(|x| x.address == address)
            .collect()
    }
    pub fn get_breakpoints(context: &CONTEXT) -> [HardwareBreakpoint; 4] {
        [
            get_breakpoint(context, Index::First),
            get_breakpoint(context, Index::Second),
            get_breakpoint(context, Index::Third),
            get_breakpoint(context, Index::Fourth),
        ]
    }
    pub fn clear(context: &mut CONTEXT) {
        set_breakpoint(context, &HardwareBreakpoint::new(Index::First));
        set_breakpoint(context, &HardwareBreakpoint::new(Index::Second));
        set_breakpoint(context, &HardwareBreakpoint::new(Index::Third));
        set_breakpoint(context, &HardwareBreakpoint::new(Index::Fourth));
    }
}

impl HardwareBreakpoint {
    /// # Safety
    /// This function will never directly cause undefined behaviour, but the breakpoint it places
    /// will for obvious reasons be a breakpoint, meaning it will cause an exception to be thrown
    /// when it is hit. Calling this function is therefore unsafe, as it might affect the program
    /// in unexpected ways if the caller doesn't properly set up some form of exception handling.
    pub unsafe fn set(&self) -> Result<(), HwbpError> {
        let mut context = HwbpContext::get()?;
        context.set_breakpoint(self);
        context.apply()?;
        Ok(())
    }

    /// # Safety
    /// This function will never directly cause undefined behaviour, but the breakpoint it places
    /// will for obvious reasons be a breakpoint, meaning it will cause an exception to be thrown
    /// when it is hit. Calling this function is therefore unsafe, as it might affect the program
    /// in unexpected ways if the caller doesn't properly set up some form of exception handling.
    pub unsafe fn set_rtl(&self) {
        let mut context = HwbpContext::get_rtl();
        context.set_breakpoint(self);
        context.apply_rtl();
    }

    pub fn unused() -> Result<Option<HardwareBreakpoint>, HwbpError> {
        let context = HwbpContext::get()?;
        Ok(context.unused_breakpoint())
    }

    pub fn unused_rtl() -> Option<HardwareBreakpoint> {
        let context = HwbpContext::get_rtl();
        context.unused_breakpoint()
    }
}

#[cfg(test)]
mod tests {
    use crate::{Condition, HardwareBreakpoint, HwbpContext, Index, Size};
    use std::{
        ffi::c_void,
        ptr::{null_mut, read_volatile, write_volatile},
    };
    use winapi::um::errhandlingapi::{AddVectoredExceptionHandler, RemoveVectoredExceptionHandler};
    use winapi::um::minwinbase::EXCEPTION_SINGLE_STEP;
    use winapi::um::winnt::{LONG, PEXCEPTION_POINTERS};
    use winapi::vc::excpt::{EXCEPTION_CONTINUE_EXECUTION, EXCEPTION_CONTINUE_SEARCH};

    static mut FLAG: [u8; 8] = [0; 8];
    static mut FLAG_HITS: u32 = 0;
    static mut CLEAR_BP_ON_HIT: bool = false;

    unsafe extern "system" fn handler(ex: PEXCEPTION_POINTERS) -> LONG {
        if let Some(ex) = ex.as_ref() {
            let cr = ex.ContextRecord.as_mut();
            let er = ex.ExceptionRecord.as_mut();

            if let (Some(cr), Some(er)) = (cr, er) {
                if er.ExceptionCode == EXCEPTION_SINGLE_STEP {
                    // If we want to clear the breakpoint when it's hit, do so
                    if CLEAR_BP_ON_HIT {
                        crate::raw::clear(cr);
                    }

                    // Increase flag hits by one
                    FLAG_HITS += 1;

                    // Reset the debug status
                    cr.Dr6 = 0;

                    return EXCEPTION_CONTINUE_EXECUTION;
                }
            }
        }

        EXCEPTION_CONTINUE_SEARCH
    }

    #[test]
    fn breakpoint_hits() {
        unsafe {
            // Add VEH handler
            let veh = AddVectoredExceptionHandler(1, Some(handler as _));

            // Make sure it got added
            assert_ne!(veh, null_mut());

            // Clear all hardware breakpoints to ensure there's none set
            HwbpContext::get().unwrap().clear();

            // Create a new hardware breakpoint
            let mut breakpoint = HardwareBreakpoint::new(Index::First);
            breakpoint.size = Size::One;
            breakpoint.enabled = true;
            breakpoint.address = &FLAG as *const _ as _;

            // --- --- --- --- --- TESTS START HERE

            // --- Test Condition::ReadWrite
            {
                // Prepare
                FLAG_HITS = 0;
                CLEAR_BP_ON_HIT = false;

                // Prepare and set the breakpoint
                breakpoint.size = Size::One;
                breakpoint.address = &FLAG as *const _ as _;
                breakpoint.condition = Condition::ReadWrite;
                breakpoint.set().expect("failed to set read breakpoint");

                // Reading
                {
                    // Read the flag
                    read_volatile(&FLAG);
                    // Ensure reading caused a hit
                    assert_eq!(FLAG_HITS, 1);
                }

                // Writing
                {
                    // Write to the flag
                    write_volatile(&mut FLAG[0], 0);
                    // Ensure writing caused a hit
                    assert_eq!(FLAG_HITS, 2);
                }
            }

            // --- Test Condition::Write
            {
                // Prepare
                FLAG_HITS = 0;
                CLEAR_BP_ON_HIT = false;

                // Prepare and set the breakpoint
                breakpoint.size = Size::One;
                breakpoint.address = &FLAG as *const _ as _;
                breakpoint.condition = Condition::Write;
                breakpoint.set().expect("failed to set write breakpoint");

                // Reading
                {
                    // Read the flag
                    read_volatile(&FLAG);
                    // Ensure reading did NOT cause a hit
                    assert_eq!(FLAG_HITS, 0);
                }

                // Writing
                {
                    // Write to the flag
                    write_volatile(&mut FLAG[0], 0);
                    // Ensure writing caused a hit
                    assert_eq!(FLAG_HITS, 1);
                }
            }

            // --- Test Condition::Execution
            {
                #[inline(never)]
                fn nop() {}

                // Prepare
                FLAG_HITS = 0;
                CLEAR_BP_ON_HIT = true;

                // Prepare and set the breakpoint
                breakpoint.size = Size::One;
                breakpoint.address = nop as *const c_void as _;
                breakpoint.condition = Condition::Execution;
                breakpoint.set().expect("failed to set exec breakpoint");

                // Call the function
                nop();

                // Ensure the breakpoint got hit
                assert_eq!(FLAG_HITS, 1);
            }

            // Used for the tests below
            // We're using write_volatile as optimization can affect tests if it causes multiple
            //   flags to be written at once
            unsafe fn set_all_flags() {
                write_volatile(&mut FLAG[0], 1);
                write_volatile(&mut FLAG[1], 1);
                write_volatile(&mut FLAG[2], 1);
                write_volatile(&mut FLAG[3], 1);
                write_volatile(&mut FLAG[4], 1);
                write_volatile(&mut FLAG[5], 1);
                write_volatile(&mut FLAG[6], 1);
                write_volatile(&mut FLAG[7], 1);
            }

            // --- Test Size::One
            {
                // Prepare
                FLAG_HITS = 0;
                CLEAR_BP_ON_HIT = false;

                // Prepare and set the breakpoint
                breakpoint.size = Size::One;
                breakpoint.address = &FLAG as *const _ as _;
                breakpoint.condition = Condition::Write;
                breakpoint.set().expect("failed to set write breakpoint");

                // Trigger all flags
                set_all_flags();

                // Check that Size::One hits once
                assert_eq!(FLAG_HITS, 1);
            }

            // --- Test Size::Two
            {
                // Prepare
                FLAG_HITS = 0;
                CLEAR_BP_ON_HIT = false;

                // Prepare and set the breakpoint
                breakpoint.size = Size::Two;
                breakpoint.address = &FLAG as *const _ as _;
                breakpoint.condition = Condition::Write;
                breakpoint.set().expect("failed to set write breakpoint");

                // Trigger all flags
                set_all_flags();

                // Check that Size::Two hits twice
                assert_eq!(FLAG_HITS, 2);
            }

            // --- Test Size::Four
            {
                // Prepare
                FLAG_HITS = 0;
                CLEAR_BP_ON_HIT = false;

                // Prepare and set the breakpoint
                breakpoint.size = Size::Four;
                breakpoint.address = &FLAG as *const _ as _;
                breakpoint.condition = Condition::Write;
                breakpoint.set().expect("failed to set write breakpoint");

                // Trigger all flags
                set_all_flags();

                // Check that Size::Four hits four times
                assert_eq!(FLAG_HITS, 4);
            }

            // --- Test Size::Eight
            #[cfg(target_pointer_width = "64")]
            {
                // Prepare
                FLAG_HITS = 0;
                CLEAR_BP_ON_HIT = false;

                // Prepare and set the breakpoint
                breakpoint.size = Size::Eight;
                breakpoint.address = &FLAG as *const _ as _;
                breakpoint.condition = Condition::Write;
                breakpoint.set().expect("failed to set write breakpoint");

                // Trigger all flags
                set_all_flags();

                // Check that Size::Eight hits eight times
                assert_eq!(FLAG_HITS, 8);
            }

            // --- --- --- --- --- TESTS END HERE

            // Clear any leftover breakpoints
            HwbpContext::get().unwrap().clear();

            // Remove the exception handler
            RemoveVectoredExceptionHandler(veh);
        }
    }
}
