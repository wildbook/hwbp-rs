use std::{
    borrow::{Borrow, BorrowMut},
    convert::TryFrom,
};

use winapi::um::winnt::{CONTEXT, CONTEXT_DEBUG_REGISTERS};

use crate::{
    context::{ApplyContext, ApplyWith, FetchContext, FetchWith},
    raw,
    registers::{Dr6, Dr7},
    Hwbp, HwbpError, Index, PseudoUsize,
};

// The `align(16)` is required for [`CONTEXT`], and `winapi-rs` only left a comment reading
// "// FIXME align 16" next to the [`CONTEXT`] struct. This led to hours wasted debugging why
// the windows API was refusing to fill / apply contexts that were seemingly completely fine.
//
// If `winapi-rs` ever fixes this, or we swap to a crate that handles alignment properly to
// begin with, we can remove the explicit aligment here. Until then, keep it, or you'll get
// seemingly random failures based on where in memory [`HwbpContext`] happens to be placed.
#[repr(align(16))]
pub struct HwbpContext<T>(T);

impl HwbpContext<CONTEXT> {
    /// Retrieves the [`HwbpContext`] for the current thread.
    pub fn get() -> Result<Self, HwbpError> {
        Self::get_with(FetchWith::GetThreadContext)
    }

    /// Retrieves a [`HwbpContext`].
    ///
    /// ```
    /// # use hwbp::{HwbpContext, context::FetchWith};
    /// HwbpContext::get_with(FetchWith::GetThreadContext)
    ///     .expect("failed to get context");
    /// ```
    pub fn get_with(with: impl FetchContext) -> Result<Self, HwbpError> {
        // We're creating a blank context and setting the ContextFlags field before passing it
        // to GetThreadContext, which reads the field and returns the appropriate data
        let mut context: Self = unsafe { std::mem::zeroed() };
        context.0.ContextFlags = CONTEXT_DEBUG_REGISTERS;

        with.fetch_context(&mut context.0)?;
        Ok(context)
    }
}

impl<C: Borrow<CONTEXT>> HwbpContext<C> {
    /// Be careful with this function if you plan on calling [`HwbpContext::apply`] / [`HwbpContext::apply_with`].
    ///
    /// When [`HwbpContext`] grabs a context it sets `.ContextFlags = CONTEXT_DEBUG_REGISTERS`.
    /// To avoid requiring `&mut self` instead of `&self` when applying the context later, it
    /// assumes that `CONTEXT_DEBUG_REGISTERS` is still the only flag in `.ContextFlags`, and
    /// sets the context as it is without modifications.
    ///
    /// If you use [`HwbpContext::from_context`] to create a [`HwbpContext`] from a context that
    /// has other flags set, those flags will specify which parts of the context get applied when
    /// [`HwbpContext`] applies the context. This might cause unexpected behavior if you are
    /// expecting apply to only apply breakpoints from the passed context.
    ///
    /// The reason this function does not set `.ContextFlags = CONTEXT_DEBUG_REGISTERS` is that
    /// in most cases you want to preserve the flags of the context you're modifying breakpoint
    /// data on. The most common case would be if you're modifying the context record inside an
    /// exception handler, in which case you don't want to modify anything but the parts you're
    /// explicitly modifiying.
    pub fn from_context(context: C) -> HwbpContext<C> {
        HwbpContext(context)
    }

    /// Retrieves the wrapped context.
    pub fn into_context(self) -> C {
        self.0
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
        /// # use hwbp::{HwbpContext, context::ApplyWith};
        /// # let mut ctx = HwbpContext::get().unwrap();
        /// ctx.apply_with(ApplyWith::SetThreadContext)
        ///     .expect("failed to apply context");
        /// # }
        /// ```
        pub unsafe fn apply_with(&self, with: impl ApplyContext) -> Result<(), HwbpError> {
            with.apply_context(self.0.borrow())
        }
    );

    /// Returns a currently unused hardware breakpoint, or [`None`] if all are in use.
    pub fn unused_breakpoint(&self) -> Option<Hwbp> {
        raw::get_breakpoints(self.0.borrow()).find(|bp| !bp.enabled)
    }

    /// Returns the breakpoint at the given index.
    pub fn breakpoint(&self, index: Index) -> Hwbp {
        raw::get_breakpoint(self.0.borrow(), index)
    }

    /// Returns all hardware breakpoints.
    pub fn breakpoints(&self) -> impl Iterator<Item = Hwbp> + '_ {
        raw::get_breakpoints(self.0.borrow())
    }
}

impl<C: BorrowMut<CONTEXT>> HwbpContext<C> {
    /// Writes a breakpoint to the wrapped context.
    pub fn set_breakpoint(&mut self, bp: Hwbp) {
        let ctx = self.0.borrow_mut();

        *match bp.index {
            Index::First => &mut ctx.Dr0,
            Index::Second => &mut ctx.Dr1,
            Index::Third => &mut ctx.Dr2,
            Index::Fourth => &mut ctx.Dr3,
        } = bp.address as _;

        // Set the condition, size, and enabled bits.
        Dr7(&mut ctx.Dr7).set_size(bp.index, bp.size);
        Dr7(&mut ctx.Dr7).set_condition(bp.index, bp.condition);
        Dr7(&mut ctx.Dr7).set_enabled_local(bp.index, bp.enabled);
    }

    /// Returns breakpoints that overlap with the specified address.
    ///
    /// This does not check if the breakpoints are enabled or not.
    pub fn breakpoints_by_address<'a, T: 'a>(
        &'a self,
        address: *const T,
    ) -> impl Iterator<Item = Hwbp> + 'a {
        raw::get_breakpoints_by_address(self.0.borrow(), address)
    }

    multidoc! {
        /// Returns the breakpoint that triggered the current exception.
        ///
        /// Keep in mind that [`Dr6`] is not automatically reset, so you must reset it manually every
        /// time a hardware breakpoint is hit for it to contain useful information. There's a
        /// convenience function for this, [`HwbpContext::reset_dr6`].
        ///
        /// If [`Dr6`] does not have exactly one hwbp flag set, this function will return `None`.
        /// This can happen if multiple breakpoints are triggered on the same instruction, or if the
        /// [`Dr6`] register was not reset after a previous hwbp hit.
        =>
        pub fn breakpoints_by_dr6_value(&self, dr6: PseudoUsize) -> impl Iterator<Item = Hwbp> + '_  {
            Dr6(dr6).breakpoints().into_iter().enumerate().filter_map(move |(i, x)| {
                x.then(|| self.breakpoint(Index::try_from(i as u8).expect("can't fail")))
            })
        }

        pub fn breakpoints_by_dr6(&self) -> impl Iterator<Item = Hwbp> + '_ {
            self.breakpoints_by_dr6_value(self.0.borrow().Dr6 as _)
        }
    }

    /// Fully clears any currently set hardware breakpoints.
    pub fn clear_breakpoints(&mut self) {
        let context = self.0.borrow_mut();

        Dr7(&mut context.Dr7).clear_breakpoints();
        context.Dr0 = 0;
        context.Dr1 = 0;
        context.Dr2 = 0;
        context.Dr3 = 0;
    }

    pub fn dr6(&self) -> Dr6<PseudoUsize> {
        Dr6(self.0.borrow().Dr6)
    }

    pub fn dr7(&self) -> Dr7<PseudoUsize> {
        Dr7(self.0.borrow().Dr7)
    }

    pub fn dr6_mut(&mut self) -> Dr6<&mut PseudoUsize> {
        Dr6(&mut self.0.borrow_mut().Dr6)
    }

    pub fn dr7_mut(&mut self) -> Dr7<&mut PseudoUsize> {
        Dr7(&mut self.0.borrow_mut().Dr7)
    }
}
