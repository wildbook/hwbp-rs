use winapi::um::processthreadsapi::{GetThreadContext, SetThreadContext};
use winapi::um::winnt::{RtlCaptureContext, CONTEXT, HANDLE};

use crate::HwbpError;

fn current_thread() -> HANDLE {
    // WinAPI's GetCurrentThread() only calls NtCurrentThread(), which is hardcoded to always returns -2.
    -2 as _
}

pub trait FetchContext {
    fn fetch_context(self, context: &mut CONTEXT) -> Result<(), HwbpError>;
}

pub trait ApplyContext {
    fn apply_context(self, context: &CONTEXT) -> Result<(), HwbpError>;
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ApplyWith {
    SetThreadContext,
    SetThreadContextOther(HANDLE),
    #[cfg(target_arch = "x86_64")]
    RtlRestoreContext,
    #[cfg(feature = "ntapi")]
    NtContinue,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FetchWith {
    GetThreadContext,
    GetThreadContextOther(HANDLE),
    RtlCaptureContext,
}

impl FetchContext for FetchWith {
    fn fetch_context(self, context: &mut CONTEXT) -> Result<(), HwbpError> {
        unsafe {
            match self {
                FetchWith::RtlCaptureContext => Ok(RtlCaptureContext(context)),
                FetchWith::GetThreadContext => match GetThreadContext(current_thread(), context) {
                    0 => Err(HwbpError::FailedFetchContext),
                    _ => Ok(()),
                },
                FetchWith::GetThreadContextOther(h) => match GetThreadContext(h, context) {
                    0 => Err(HwbpError::FailedFetchContext),
                    _ => Ok(()),
                },
            }
        }
    }
}

impl ApplyContext for ApplyWith {
    fn apply_context(self, context: &CONTEXT) -> Result<(), HwbpError> {
        let as_mut = context as *const CONTEXT as *mut CONTEXT;
        unsafe {
            match self {
                ApplyWith::SetThreadContext => match SetThreadContext(current_thread(), context) {
                    0 => Err(HwbpError::FailedApplyContext),
                    _ => Ok(()),
                },
                ApplyWith::SetThreadContextOther(h) => match SetThreadContext(h, context) {
                    0 => Err(HwbpError::FailedApplyContext),
                    _ => Ok(()),
                },
                #[cfg(target_arch = "x86_64")]
                ApplyWith::RtlRestoreContext => {
                    use winapi::um::winnt::RtlRestoreContext;

                    Ok(RtlRestoreContext(as_mut, std::ptr::null_mut()))
                }
                #[cfg(feature = "ntapi")]
                ApplyWith::NtContinue => {
                    use ntapi::ntxcapi::NtContinue;
                    use winapi::shared::ntdef::NT_SUCCESS;

                    match NT_SUCCESS(NtContinue(as_mut, 1)) {
                        true => Ok(()),
                        false => Err(HwbpError::FailedApplyContext),
                    }
                }
            }
        }
    }
}

/// Resets Dr6 and returns the previous value, leaving only bit 16 set.
///
/// Page 581 of [Intel® 64 and IA-32 Architectures Software Developer’s Manual Volume 3 (3A, 3B, 3C & 3D): System Programming Guide](https://www.intel.com/content/dam/www/public/us/en/documents/manuals/64-ia-32-architectures-software-developer-system-programming-manual-325384.pdf):
///
/// • **RTM (restricted transactional memory) flag (bit 16)** — Indicates (when **clear**) that a debug exception
///   (#DB) or breakpoint exception (#BP) occurred inside an RTM region while advanced debugging of RTM trans-
///   actional regions was enabled (see Section 17.3.3). This bit is set for any other debug exception (including all
///   those that occur when advanced debugging of RTM transactional regions is not enabled). This bit is always 1 if
///   the processor does not support RTM.
pub fn reset_dr6(ctx: &mut CONTEXT) -> usize {
    std::mem::replace(&mut ctx.Dr6, 1 << 16) as usize
}
