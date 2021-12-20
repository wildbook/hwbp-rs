use crate::{Condition, Hwbp, HwbpContext, Size};
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
            let mut ctx = HwbpContext::from_context(cr);

            if er.ExceptionCode == EXCEPTION_SINGLE_STEP {
                // Increase flag hits by one
                FLAG_HITS += 1;

                // If we want to clear the breakpoint when it's hit, do so
                if CLEAR_BP_ON_HIT {
                    ctx.clear_breakpoints();
                }

                // Reset the debug status
                ctx.dr6_mut().reset();

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
        HwbpContext::get().unwrap().clear_breakpoints();

        // --- --- --- --- --- TESTS START HERE

        // --- Test Condition::ReadWrite
        {
            // Prepare
            FLAG_HITS = 0;
            CLEAR_BP_ON_HIT = false;

            // Prepare and set the breakpoint
            Hwbp::first()
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
            Hwbp::first()
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
            Hwbp::first()
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
            Hwbp::first()
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
            Hwbp::first()
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
            Hwbp::first()
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
            Hwbp::first()
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
        HwbpContext::get().unwrap().clear_breakpoints();

        // Remove the exception handler
        RemoveVectoredExceptionHandler(veh);
    }
}
