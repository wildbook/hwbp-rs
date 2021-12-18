Hardware Breakpoints for Windows
================================

`hwbp-rs` is a thin Rust wrapper around Windows' hardware breakpoint APIs.

For a general primer on hardware breakpoints, see [this great article](https://ling.re/hardware-breakpoints/) by ling.re / LingSec.

This crate is assuming that you are in user mode and not kernel mode, and all hardware breakpoints are per-thread.

Documentation
=============

To open the documentation, run `cargo doc -p hwbp --open` after adding the `hwbp` crate to your `Cargo.toml`.

Examples
========
Using `HardwareBreakpoint`:
```rs
// Construct a `HardwareBreakpoint` representing the first hwbp.
let hwbp = HardwareBreakpoint::first();

// Or just get any unused one, if you don't want to manage them yourself.
let hwbp = HardwareBreakpoint::unused()
    .expect("failed to get context")
    .expect("all breakpoints are in use");

// Configure the breakpoint.
hwbp.with_size(Size::One)
    .with_condition(Condition::ReadWrite)
    .with_address(0 as *const ())
    // And finally, enable it.
    .enable()
    .expect("failed to enable hwbp");
```

If you want to modify an existing `CONTEXT`, or modify multiple breakpoints at once, you can use an
instance of `HwbpContext` instead of `HardwareBreakpoint`. It gives you a bit more control over the
breakpoints, but it's also more verbose:
```rs
// Get a context by calling one of these two:
// Get a `HwbpContext`.
let mut context = HwbpContext::get()
    .expect("failed to get context");

// Get the first unused breakpoint.
let breakpoint = context.unused_breakpoint()
    .expect("all breakpoints are in use")
    // Configure the breakpoint.
    .with_size(Size::One)
    .with_condition(Condition::ReadWrite)
    .with_address(0 as *const ())
    .with_enabled(true); // <- Don't forget this one!

// Write the modified breakpoint to the context.
context.set_breakpoint(breakpoint);

// And finally, apply the context.
context.apply().expect("failed to apply context");
```

You'll most likely also want to handle the resulting exceptions, which you can do like this:
```rs
// The example below assumes you're using `winapi-rs` or `windows-sys` or similar.
// This library on its own does not provide a way to manage exception handlers.

unsafe extern "system" fn handler(ex: PEXCEPTION_POINTERS) -> LONG {
    if let Some(ex) = ex.as_ref() {
        let cr = ex.ContextRecord.as_mut();
        let er = ex.ExceptionRecord.as_mut();

        if let (Some(cr), Some(er)) = (cr, er) {
            if er.ExceptionCode == EXCEPTION_SINGLE_STEP {
                // Reset the debug status register.
                // This is especially important if you're using HwbpContext::breakpoint_by_dr6.
                let dr6 = reset_dr6(cr);

                // Since we're in an exception handler, the context record in `cr` is going to
                // be applied when we return `EXCEPTION_CONTINUE_EXECUTION`.
                //
                // If you want to modify hardware breakpoints in here, make sure to create the
                // context by passing `cr` to `HwbpContext::from_context` instead of capturing
                // and modifying our current context. Modifying the current context will only
                // affect the current context, which will be thrown away when `cr` is applied.
                //
                // Of course, if you *do* want to modify the current context (e.g. to have a
                // hwbp set during the exception handler), you can just retrieve the current
                // context like you normally would and ignore the advice above.
                let mut context = HwbpContext::from_context(*cr);

                // Retrieve the breakpoint that triggered the exception.
                let hwbp = context.breakpoint_by_dr6(dr6);

                // [Make any desired modifications to the context here.]

                // And finally, overwrite the existing context with the modified one.
                *cr = context.into_context();

                return EXCEPTION_CONTINUE_EXECUTION;
            }
        }
    }
    EXCEPTION_CONTINUE_SEARCH
}

// Register the exception handler.
let veh = AddVectoredExceptionHandler(1, Some(handler as _));
assert_ne!(veh, std::ptr::null_mut(), "failed to add exception handler");

// [Playing with breakpoints here is left as an exercise for the reader.]

// Remove the exception handler again.
let res = RemoveVectoredExceptionHandler(veh);
assert_ne!(res, 0, "failed to remove exception handler");
```
