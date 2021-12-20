#![cfg(target_os = "windows")]
#![allow(clippy::unit_arg)]

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
//! Using `Hwbp`:
//! ```
//! # unsafe {
//! # use hwbp::*;
//!
//! // Construct a `Hwbp` representing the first hwbp.
//! let hwbp = Hwbp::first();
//!
//! // Or just get any unused one, if you don't want to manage them yourself.
//! let hwbp = Hwbp::unused()
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
//! instance of `HwbpContext` instead of `Hwbp`. It gives you a bit more control over the
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
//!                 let mut context = HwbpContext::from_context(cr);
//!
//!                 // Retrieve the breakpoint(s) that triggered the exception.
//!                 let hwbp = context.breakpoints_by_dr6().next();
//!
//!                 // [Make any desired modifications to the context here.]
//!
//!                 // Reset the Dr6 register.
//!                 context.dr6_mut().reset();
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
pub mod context;
pub mod raw;
pub mod registers;

#[cfg(test)]
mod tests;

#[macro_use]
mod macros;

mod enums;
mod hwbp;
mod hwbp_context;

pub use crate::enums::{Condition, Index, Size};
pub use crate::hwbp::Hwbp;
pub use crate::hwbp_context::HwbpContext;

use std::{error::Error, fmt::Display};

#[cfg(target_pointer_width = "64")]
type PseudoUsize = u64;

#[cfg(target_pointer_width = "32")]
type PseudoUsize = u32;

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
