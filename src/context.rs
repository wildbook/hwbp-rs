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
