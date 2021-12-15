#![cfg(target_os = "windows")]

//! Hardware Breakpoints for Windows
//! ================================
//!
//! `hwbp-rs` is a thin Rust wrapper around Windows' hardware breakpoint APIs.
//!
//! For a general primer on hardware breakpoints, see [this great article](https://ling.re/hardware-breakpoints/) by ling.re / LingSec.
//!
//! This crate is assuming that you are in user mode and not kernel mode, and all hardware breakpoints are per-thread.
//!
//! Documentation
//! =============
//!
//! To open the documentation, run `cargo doc -p hwbp --open` after adding the `hwbp` crate to your `Cargo.toml`.
//!
//! Examples
//! ========
//!
//! Using `HardwareBreakpoint`:
//! ```
//! # unsafe {
//! # use hwbp::*;
//!
//! // Construct a `HardwareBreakpoint` representing the first hwbp.
//! let hwbp = HardwareBreakpoint::first();
//!
//! // Or just get any unused one, if you don't want to manage them yourself.
//! let hwbp = HardwareBreakpoint::unused()
//!     .expect("failed to get context")
//!     .expect("all breakpoints are in use");
//!
//! // Configure the breakpoint.
//! hwbp.with_size(Size::One)
//!     .with_condition(Condition::ReadWrite)
//!     .with_address(0 as *const ())
//!     // And finally, enable it.
//!     .enable()
//!     .expect("failed to enable hwbp");
//! # }
//! ```
//!
//! If you want to modify an existing `CONTEXT`, or modify multiple breakpoints at once, you can use an
//! instance of `HwbpContext` instead of `HardwareBreakpoint`. It gives you a bit more control over the
//! breakpoints, but it's also more verbose:
//! ```
//! # unsafe {
//! # use hwbp::*;
//! // Get a context by calling one of these two:
//!
//! // Get a `HwbpContext`.
//! let mut context = HwbpContext::get()
//!     .expect("failed to get context");
//!
//! // Get the first unused breakpoint.
//! let breakpoint = context.unused_breakpoint()
//!     .expect("all breakpoints are in use")
//!     // Configure the breakpoint.
//!     .with_size(Size::One)
//!     .with_condition(Condition::ReadWrite)
//!     .with_address(0 as *const ())
//!     .with_enabled(true); // <- Don't forget this one!
//!
//! // Write the modified breakpoint to the context.
//! context.set_breakpoint(breakpoint);
//!
//! // And finally, apply the context.
//! context.apply().expect("failed to apply context");
//! # }
//! ```
//!
//! You'll most likely also want to handle the resulting exceptions, which you can do like this:
//! ```
//! # unsafe {
//! # use winapi::um::errhandlingapi::{AddVectoredExceptionHandler, RemoveVectoredExceptionHandler};
//! # use winapi::um::minwinbase::EXCEPTION_SINGLE_STEP;
//! # use winapi::um::winnt::{PEXCEPTION_POINTERS, LONG};
//! # use winapi::vc::excpt::{EXCEPTION_CONTINUE_EXECUTION, EXCEPTION_CONTINUE_SEARCH};
//! # use hwbp::*;
//! // The example below assumes you're using `winapi-rs` or `windows-sys` or similar.
//! // This library on its own does not provide a way to manage exception handlers.
//!
//! unsafe extern "system" fn handler(ex: PEXCEPTION_POINTERS) -> LONG {
//!     if let Some(ex) = ex.as_ref() {
//!         let cr = ex.ContextRecord.as_mut();
//!         let er = ex.ExceptionRecord.as_mut();
//!
//!         if let (Some(cr), Some(er)) = (cr, er) {
//!             if er.ExceptionCode == EXCEPTION_SINGLE_STEP {
//!                 // Since we're in an exception handler, the context record in `cr` is going to
//!                 // be applied when we return `EXCEPTION_CONTINUE_EXECUTION`.
//!                 //
//!                 // If you want to modify hardware breakpoints in here, make sure to create the
//!                 // context by passing `cr` to `HwbpContext::from_context` instead of capturing
//!                 // and modifying our current context. Modifying the current context will only
//!                 // affect the current context, which will be thrown away when `cr` is applied.
//!                 //
//!                 // Of course, if you *do* want to modify the current context (e.g. to have a
//!                 // hwbp set during the exception handler), you can just retrieve the current
//!                 // context like you normally would and ignore the advice above.
//!                 let mut context = HwbpContext::from_context(*cr);
//!
//!                 // Reset the debug status register.
//!                 // This is especially important if you're using HwbpContext::breakpoint_by_dr6.
//!                 let dr6 = reset_dr6(cr);
//!
//!                 // Retrieve the breakpoint that triggered the exception.
//!                 let hwbp = context.breakpoint_by_dr6(dr6);
//!
//!                 // [Make any desired modifications to the context here.]
//!
//!                 // And finally, overwrite the existing context with the modified one.
//!                 *cr = context.into_context();
//!
//!                 return EXCEPTION_CONTINUE_EXECUTION;
//!             }
//!         }
//!     }
//!
//!     EXCEPTION_CONTINUE_SEARCH
//! }
//!
//! // Register the exception handler.
//! let veh = AddVectoredExceptionHandler(1, Some(handler as _));
//! assert_ne!(veh, std::ptr::null_mut(), "failed to add exception handler");
//!
//! // [Playing with breakpoints here is left as an exercise for the reader.]
//!
//! // Remove the exception handler again.
//! let res = RemoveVectoredExceptionHandler(veh);
//! assert_ne!(res, 0, "failed to remove exception handler");
//! # }
//! ```
mod context;
pub mod raw;
pub use context::{reset_dr6, ApplyWith, FetchWith};
use winapi::um::winnt::CONTEXT;

use context::{ApplyContext, FetchContext};
use std::{convert::TryFrom, error::Error, ffi::c_void, fmt::Display};
use winapi::um::winnt::CONTEXT_DEBUG_REGISTERS;

#[cfg(target_pointer_width = "64")]
type WinAPIHatesUsize = u64;

#[cfg(target_pointer_width = "32")]
type WinAPIHatesUsize = u32;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Condition {
    /// `Condition::Execution` must be paired with `Size::One`.
    /// Any other size will result in the breakpoint not being hit.
    Execution = 0b00,
    Write = 0b01,
    ReadWrite = 0b11,
    IoReadWrite = 0b10,
}

impl Condition {
    pub const fn from_bits(bits: u8) -> Option<Condition> {
        match bits {
            0b00 => Some(Condition::Execution),
            0b01 => Some(Condition::Write),
            0b11 => Some(Condition::ReadWrite),
            0b10 => Some(Condition::IoReadWrite),
            _ => None,
        }
    }

    pub const fn as_bits(self) -> u8 {
        self as u8
    }
}

// Since it is not obvious which representation this enum resolves to when `as` is used to cast it,
// we simply let it resolve to the default representation instead of picking one of the two.
//
// This lets us force the user to be explicit about which representation they want to use.
// Preferably we'd also forbid `as` from being used on this enum, but that's not possible yet.
//
// If it ever becomes possible to forbid `as` from being used on this enum, we should do so.

/// An enum representing the size of a hardware breakpoint.
///
/// **Avoid using `as` to cast this enum to a number, it will not return what you expect it to.**
///
/// Instead, use `Size::in_bytes` and `Size::as_bits`.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Size {
    One,
    Two,
    Four,
    /// Eight byte breakpoints are only supported in 64-bit context.
    Eight,
}

impl Size {
    /// Returns how many bytes a hardware breakpoint using this `Size` would cover.
    pub const fn in_bytes(self) -> usize {
        match self {
            Size::One => 1,
            Size::Two => 2,
            Size::Four => 4,
            Size::Eight => 8,
        }
    }

    /// Returns the two-bit representation used in `CONTEXT.Dr7`.
    pub const fn as_bits(self) -> u8 {
        match self {
            Size::One => 0b00,
            Size::Two => 0b01,
            Size::Four => 0b11,
            Size::Eight => 0b10,
        }
    }

    /// Returns the `Size` that corresponds to the two-bit representation in `CONTEXT.Dr7`.
    pub const fn from_bits(bits: u8) -> Option<Self> {
        match bits {
            0b00 => Some(Size::One),
            0b01 => Some(Size::Two),
            0b11 => Some(Size::Four),
            0b10 => Some(Size::Eight),
            _ => None,
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Index {
    First = 0,
    Second = 1,
    Third = 2,
    Fourth = 3,
}

impl Index {
    /// Returns the index of the breakpoint that triggered the current exception.
    ///
    /// Keep in mind that `Dr6` is not automatically cleared, so you must clear it manually in your
    /// exception handler for it to contain useful information.
    ///
    /// If `Dr6` does not have exactly one hwbp flag set, this function will return `None`.
    pub fn by_dr6(dr6: usize) -> Option<Index> {
        match dr6 & 0b1111 {
            0b0001 => Some(Index::First),
            0b0010 => Some(Index::Second),
            0b0100 => Some(Index::Third),
            0b1000 => Some(Index::Fourth),
            _ => None,
        }
    }
}

impl TryFrom<u8> for Index {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            // TODO: When `inline-const` is stabilized, rewrite the branches below.
            // const { Index::First as _ } => ...,
            x if x == Index::First as _ => Ok(Index::First),
            x if x == Index::Second as _ => Ok(Index::Second),
            x if x == Index::Third as _ => Ok(Index::Third),
            x if x == Index::Fourth as _ => Ok(Index::Fourth),
            _ => Err(()),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct HardwareBreakpoint {
    pub enabled: bool,
    pub index: Index,
    pub address: *const c_void,
    pub size: Size,
    pub condition: Condition,
}

macro_rules! multidoc {
    ($(#[$meta:meta])* => $item:item $($items:item)*) => {
        $(#[$meta])*
        $item
        multidoc!($(#[$meta])* => $($items)*);
    };
    ($(#[$meta:meta])* => ) => {}
}

impl HardwareBreakpoint {
    fn new() -> Self {
        Self {
            enabled: false,
            index: Index::First,
            address: std::ptr::null(),
            size: Size::One,
            condition: Condition::ReadWrite,
        }
    }

    #[rustfmt::skip]
    multidoc! {
        /// Constructs a new hardware breakpoint.
        /// 
        /// ```compile_fail
        /// # use std::ptr::null;
        /// # use hwbp::{HardwareBreakpoint, Index, Size, Condition};
        /// HardwareBreakpoint {
        ///     enabled: false,
        ///     index: ...,
        ///     address: null(),
        ///     size: Size::One,
        ///     condition: Condition::ReadWrite,
        /// };
        /// ```
        =>
        pub fn first() -> Self { Self::new().with_index(Index::First) }
        pub fn second() -> Self { Self::new().with_index(Index::Second) }
        pub fn third() -> Self { Self::new().with_index(Index::Third) }
        pub fn fourth() -> Self { Self::new().with_index(Index::Fourth) }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum HwbpError {
    FailedFetchContext,
    FailedApplyContext,
}

impl Error for HwbpError {}
impl Display for HwbpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FailedFetchContext => write!(f, "failed to fetch thread context"),
            Self::FailedApplyContext => write!(f, "failed to apply thread context"),
        }
    }
}

// The `align(16)` is required for `CONTEXT`, and `winapi-rs` only left a comment reading
// "// FIXME align 16" next to the `CONTEXT` struct. This led to hours wasted debugging why
// the windows API was refusing to fill / apply contexts that were seemingly completely fine.
//
// If `winapi-rs` ever fixes this, or we swap to a crate that handles alignment properly to
// begin with, we can remove the explicit aligment here. Until then, keep it, or you'll get
// seemingly random failures based on where in memory `HwbpContext` happens to be placed.
#[repr(align(16))]
pub struct HwbpContext(CONTEXT);

impl HwbpContext {
    /// Be careful with this function.
    ///
    /// When `HwbpContext` grabs a context it sets `.ContextFlags = CONTEXT_DEBUG_REGISTERS`.
    /// To avoid requiring `&mut self` instead of `&self` when applying the context later, it
    /// assumes that `CONTEXT_DEBUG_REGISTERS` is still the only flag in `.ContextFlags`, and
    /// sets the context as it is without modifications.
    ///
    /// If you use `from_context` to create a `HwbpContext` from a context that has other flags
    /// set, those flags will specify which parts of the context get applied when `HwbpContext`
    /// applies the context. This might cause unexpected behavior if you are expecting apply to
    /// only apply breakpoints from the passed context.
    ///
    /// The reason this function does not set `.ContextFlags = CONTEXT_DEBUG_REGISTERS` is that
    /// in most cases you want to preserve the flags of the context you're modifying breakpoint
    /// data on. The most common case would be if you're modifying the context record inside an
    /// exception handler, in which case you don't want to modify anything but the parts you're
    /// explicitly modifiying.
    pub fn from_context(context: CONTEXT) -> HwbpContext {
        HwbpContext(context)
    }

    /// Retrieves the wrapped context.
    pub fn into_context(self) -> CONTEXT {
        self.0
    }
}

impl HwbpContext {
    /// Retrieves the `HwbpContext` for the current thread.
    pub fn get() -> Result<Self, HwbpError> {
        Self::get_with(FetchWith::GetThreadContext)
    }

    /// Retrieves a `HwbpContext`.
    ///
    /// ```
    /// # use hwbp::{HwbpContext, FetchWith};
    /// HwbpContext::get_with(FetchWith::GetThreadContext)
    ///     .expect("failed to get context");
    /// ```
    pub fn get_with(with: impl FetchContext) -> Result<HwbpContext, HwbpError> {
        // We're creating a blank context and setting the ContextFlags field before passing it
        // to GetThreadContext, which reads the field and returns the appropriate data
        let mut context: HwbpContext = unsafe { std::mem::zeroed() };
        context.0.ContextFlags = CONTEXT_DEBUG_REGISTERS;

        with.fetch_context(&mut context.0)?;
        Ok(context)
    }

    multidoc!(
        /// # Safety
        /// This function will never directly cause undefined behaviour, but the breakpoints it can be
        /// used to place will cause exceptions to be thrown when they are hit. Calling this function
        /// is therefore unsafe, as it might affect the program in unexpected ways if the caller doesn't
        /// properly set up some form of exception handling.
        =>
        pub unsafe fn apply(&self) -> Result<(), HwbpError> {
            self.apply_with(ApplyWith::SetThreadContext)
        }

        /// ```no_run
        /// # unsafe {
        /// # use hwbp::{HwbpContext, ApplyWith};
        /// # let mut ctx = HwbpContext::get().unwrap();
        /// ctx.apply_with(ApplyWith::SetThreadContext)
        ///     .expect("failed to apply context");
        /// # }
        /// ```
        pub unsafe fn apply_with(&self, with: impl ApplyContext) -> Result<(), HwbpError> {
            with.apply_context(&self.0)
        }
    );

    /// Returns a currently unused hardware breakpoint.
    pub fn unused_breakpoint(&self) -> Option<HardwareBreakpoint> {
        raw::unused_breakpoint(&self.0)
    }

    /// Writes a breakpoint to the wrapped context.
    pub fn set_breakpoint(&mut self, bp: HardwareBreakpoint) {
        raw::set_breakpoint(&mut self.0, bp);
    }

    /// Returns the breakpoint at the given index.
    pub fn breakpoint(&self, index: Index) -> HardwareBreakpoint {
        raw::get_breakpoint(&self.0, index)
    }

    /// Returns all hardware breakpoints.
    pub fn breakpoints(&self) -> impl Iterator<Item = HardwareBreakpoint> + '_ {
        raw::get_breakpoints(&self.0)
    }

    /// Returns breakpoints that overlap with the specified address.
    ///
    /// This does not check if the breakpoints are enabled or not.
    pub fn breakpoints_by_address<'a, T: 'a>(
        &'a self,
        address: *const T,
    ) -> impl Iterator<Item = HardwareBreakpoint> + 'a {
        raw::get_breakpoints_by_address(&self.0, address)
    }

    /// Returns the breakpoint that triggered the current exception.
    ///
    /// Keep in mind that `Dr6` is not automatically cleared, so you must clear it manually every
    /// time a hardware breakpoint is hit for it to contain useful information.
    ///
    /// If `Dr6` does not have exactly one hwbp flag set, this function will return `None`.
    pub fn breakpoint_by_dr6(&self, dr6: usize) -> Option<HardwareBreakpoint> {
        Some(self.breakpoint(Index::by_dr6(dr6)?))
    }

    /// Clears any currently set hardware breakpoints.
    pub fn clear(&mut self) {
        raw::clear_breakpoints(&mut self.0);
    }
}

impl HardwareBreakpoint {
    pub fn with_address<T>(mut self, address: *const T) -> HardwareBreakpoint {
        self.address = address.cast();
        self
    }

    pub fn with_condition(mut self, condition: Condition) -> HardwareBreakpoint {
        self.condition = condition;
        self
    }

    pub fn with_size(mut self, size: Size) -> HardwareBreakpoint {
        self.size = size;
        self
    }

    pub fn with_index(mut self, index: Index) -> HardwareBreakpoint {
        self.index = index;
        self
    }

    pub fn with_enabled(mut self, b: bool) -> HardwareBreakpoint {
        self.enabled = b;
        self
    }
}

impl HardwareBreakpoint {
    multidoc! {
        /// # Safety
        /// This function will never directly cause undefined behaviour, but the breakpoint it places
        /// will for obvious reasons be a breakpoint, meaning it will cause an exception to be thrown
        /// when it is hit. Calling this function is therefore unsafe, as it might affect the program
        /// in unexpected ways if the caller doesn't properly set up some form of exception handling.
        =>
        pub unsafe fn apply(self) -> Result<(), HwbpError> {
            let mut context = HwbpContext::get()?;
            context.set_breakpoint(self);
            context.apply()
        }

        pub unsafe fn apply_with(
            self,
            fetch: impl FetchContext,
            apply: impl ApplyContext,
        ) -> Result<(), HwbpError> {
            let mut context = HwbpContext::get_with(fetch)?;
            context.set_breakpoint(self);
            context.apply_with(apply)
        }
    }

    multidoc! {
        /// Enables and applies the breakpoint.
        ///
        /// # Safety
        /// This function will never directly cause undefined behaviour, but the breakpoint it places
        /// will for obvious reasons be a breakpoint, meaning it will cause an exception to be thrown
        /// when it is hit. Calling this function is therefore unsafe, as it might affect the program
        /// in unexpected ways if the caller doesn't properly set up some form of exception handling.
        =>
        pub unsafe fn enable(mut self) -> Result<(), HwbpError> {
            self.enabled = true;
            let mut context = HwbpContext::get()?;
            context.set_breakpoint(self);
            context.apply()
        }

        pub unsafe fn enable_with(
            mut self,
            fetch: impl FetchContext,
            apply: impl ApplyContext,
        ) -> Result<(), HwbpError> {
            self.enabled = true;
            let mut context = HwbpContext::get_with(fetch)?;
            context.set_breakpoint(self);
            context.apply_with(apply)
        }
    }

    /// Returns a currently unused hardware breakpoint.
    ///
    /// ```
    /// # use hwbp::{HardwareBreakpoint, Index};
    /// HardwareBreakpoint::unused()
    ///     .expect("failed to fetch context")
    ///     .expect("no unused breakpoints");
    /// ```
    pub fn unused() -> Result<Option<HardwareBreakpoint>, HwbpError> {
        Self::unused_with(FetchWith::GetThreadContext)
    }

    /// Returns a currently unused hardware breakpoint.
    ///
    /// ```
    /// # use hwbp::{HardwareBreakpoint, FetchWith};
    /// HardwareBreakpoint::unused_with(FetchWith::GetThreadContext)
    ///     .expect("failed to fetch context")
    ///     .expect("no unused breakpoints");
    /// ```
    pub fn unused_with(fetch: impl FetchContext) -> Result<Option<HardwareBreakpoint>, HwbpError> {
        Ok(HwbpContext::get_with(fetch)?.unused_breakpoint())
    }
}

#[cfg(test)]
mod tests {
    use crate::{reset_dr6, Condition, HardwareBreakpoint, HwbpContext, Size};
    use std::ptr::{null_mut, read_volatile, write_volatile};
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
                        crate::raw::clear_breakpoints(cr);
                    }

                    // Increase flag hits by one
                    FLAG_HITS += 1;

                    // Reset the debug status
                    reset_dr6(cr);

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

            // --- --- --- --- --- TESTS START HERE

            // --- Test Condition::ReadWrite
            {
                // Prepare
                FLAG_HITS = 0;
                CLEAR_BP_ON_HIT = false;

                // Prepare and set the breakpoint
                HardwareBreakpoint::first()
                    .with_size(Size::One)
                    .with_address(FLAG.as_ptr())
                    .with_condition(Condition::ReadWrite)
                    .enable()
                    .expect("failed to enable 1b read breakpoint");

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
                HardwareBreakpoint::first()
                    .with_size(Size::Eight)
                    .with_address(FLAG.as_ptr())
                    .with_condition(Condition::Write)
                    .enable()
                    .expect("failed to enable 8b write breakpoint");

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
                HardwareBreakpoint::first()
                    .with_size(Size::One)
                    .with_address(nop as *const ())
                    .with_condition(Condition::Execution)
                    .enable()
                    .expect("failed to set exec breakpoint");

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
                HardwareBreakpoint::first()
                    .with_size(Size::One)
                    .with_address(FLAG.as_ptr())
                    .with_condition(Condition::Write)
                    .enable()
                    .expect("failed to enable 1b write breakpoint");

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
                HardwareBreakpoint::first()
                    .with_size(Size::Two)
                    .with_address(FLAG.as_ptr())
                    .with_condition(Condition::Write)
                    .enable()
                    .expect("failed to enable 2b write breakpoint");

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
                HardwareBreakpoint::first()
                    .with_size(Size::Four)
                    .with_address(FLAG.as_ptr())
                    .with_condition(Condition::Write)
                    .enable()
                    .expect("failed to enable 4b write breakpoint");

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
                HardwareBreakpoint::first()
                    .with_size(Size::Eight)
                    .with_address(FLAG.as_ptr())
                    .with_condition(Condition::Write)
                    .enable()
                    .expect("failed to enable 8b write breakpoint");

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
