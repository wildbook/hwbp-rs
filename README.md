[![Continuous integration](https://github.com/3db6cd7f-e24c-4b34-917a-c79bc2c4c6cc/hwbp-rs/workflows/Continuous%20integration/badge.svg)](https://github.com/3db6cd7f-e24c-4b34-917a-c79bc2c4c6cc/hwbp-rs/actions)

Hardware Breakpoints for Windows
================================

`hwbp-rs` is a thin Rust wrapper around Windows' hardware breakpoint APIs.

This crate is assuming that you are in user mode and not kernel mode, and all hardware breakpoints are per-thread.

Examples
========

Setting up an exception handler:
```rs
// The example below assumes you're using `winapi-rs` or `windows-sys` or similar.
// This library on its own does not provide a way to manage exception handlers.

unsafe extern "system" fn handler(ex: PEXCEPTION_POINTERS) -> LONG {
    if let Some(ex) = ex.as_ref() {
        let cr = ex.ContextRecord.as_mut();
        let er = ex.ExceptionRecord.as_mut();

        if let (Some(cr), Some(er)) = (cr, er) {
            if er.ExceptionCode == EXCEPTION_SINGLE_STEP {
                // Since we're in an exception handler, the context record in `cr` is going to be
                // applied when we return `EXCEPTION_CONTINUE_EXECUTION`.
                // 
                // If you want to modify hardware breakpoints in here, make sure to create the 
                // context by passing `cr` to `HwbpContext::from_context` instead of capturing
                // and modifying our current context. Modifying the current context will only
                // affect the current context, which will be thrown away when `cr` is applied.
                // 
                // Of course, if you *do* want to modify the current context (e.g. to have a hwbp
                // set during the exception handler), you can just retrieve the current context
                // like you normally would and ignore the advice above.
                let mut context = HwbpContext::from_context(*cr);

                // [Make any desired modifications to the context here.]

                // And finally, overwrite the existing context with the modified one.
                *cr = context.into_context();

                // Clear the debug status register.
                cr.Dr6 = 0;

                return EXCEPTION_CONTINUE_EXECUTION;
            }
        }
    }

    EXCEPTION_CONTINUE_SEARCH
}

// Register the exception handler.
let veh = AddVectoredExceptionHandler(1, Some(handler as _));
assert_ne!(veh, 0, "failed to add exception handler");

// [Playing with breakpoints here is left as an exercise for the reader.]

// Remove the exception handler again.
let res = RemoveVectoredExceptionHandler(veh);
assert_ne!(res, 0, "failed to remove exception handler");
```

Using `HardwareBreakpoint`:
```rs
// Create or retrieve a `HardwareBreakpoint` instance through either of these functions:

// Create a `HardwareBreakpoint` representing the first hwbp.
let mut breakpoint = HardwareBreakpoint::new(Index::First);
// Get the first unused hwbp by calling `RtlCaptureContext`.
let mut breakpoint = HardwareBreakpoint::unused_rtl()
    .expect("all breakpoints are in use");
// Get the first unused hwbp by calling `GetThreadContext`.
let mut breakpoint = HardwareBreakpoint::unused()
    .expect("failed to get context")
    .expect("all breakpoints are in use");

// Configure the breakpoint.
breakpoint.size = Size::One;
breakpoint.enabled = true;
breakpoint.address = ...;
breakpoint.condition = Condition::ReadWrite;

// Then set it using one of these:

// Set the breakpoint using `GetThreadContext` and `SetThreadContext`.
breakpoint.set().expect("failed to set breakpoint");
// Set the breakpoint using `RtlCaptureContext` and `RtlRestoreContext`.
breakpoint.set_rtl();

// You can also write it to a `HwbpContext` by calling `HwbpContext::set_breakpoint(&breakpoint)`.
```

Using `HwbpContext`:
```rs
// Get a context by calling one of these two:

// Get a `HwbpContext` by calling `RtlGetContext`.
let mut context = HwbpContext::get_rtl();
// Get a `HwbpContext` by calling `GetThreadContext`.
let mut context = HwbpContext::get()
    .expect("failed to get context");

// Get the first unused breakpoint.
let mut breakpoint = context.unused_breakpoint()
    .expect("all breakpoints are in use");

// Configure the breakpoint.
breakpoint.enabled = true;
breakpoint.address = ...;
breakpoint.condition = ...;

// Write the modified breakpoint to the context.
context.set_breakpoint(&breakpoint);

// Then to apply the context again, call either of the following:

// Apply the context using `RtlRestoreContext`.
context.apply_rtl();
// Apply the context using `SetThreadContext`.
context.apply()
    .expect("failed to apply context");
```
