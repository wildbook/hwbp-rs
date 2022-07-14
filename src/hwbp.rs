use std::{borrow::BorrowMut, ffi::c_void};

use winapi::um::winnt::CONTEXT;

use crate::{
    context::{ApplyContext, FetchContext, FetchWith},
    Condition, HwbpContext, HwbpError, Index, Size,
};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Hwbp {
    pub enabled: bool,
    pub index: Index,
    pub address: *const c_void,
    pub size: Size,
    pub condition: Condition,
}

impl Hwbp {
    #[must_use]
    fn new() -> Self {
        Self {
            enabled: false,
            index: Index::First,
            address: std::ptr::null(),
            size: Size::One,
            condition: Condition::ReadWrite,
        }
    }

    pub fn from_index(index: Index) -> Self {
        Self::new().with_index(index)
    }

    #[rustfmt::skip]
    multidoc! {
        /// Constructs a new hardware breakpoint.
        /// 
        /// ```compile_fail
        /// # use std::ptr::null;
        /// # use hwbp::{Hwbp, Index, Size, Condition};
        /// Hwbp {
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

impl Hwbp {
    #[must_use]
    pub fn with_address<T>(mut self, address: *const T) -> Hwbp {
        self.address = address.cast();
        self
    }

    #[must_use]
    pub fn with_condition(mut self, condition: Condition) -> Hwbp {
        self.condition = condition;
        self
    }

    #[must_use]
    pub fn with_size(mut self, size: Size) -> Hwbp {
        self.size = size;
        self
    }

    #[must_use]
    pub fn with_index(mut self, index: Index) -> Hwbp {
        self.index = index;
        self
    }

    #[must_use]
    pub fn with_enabled(mut self, b: bool) -> Hwbp {
        self.enabled = b;
        self
    }
}

impl Hwbp {
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

        pub fn apply_to(
            self,
            context: &mut HwbpContext<impl BorrowMut<CONTEXT>>,
        ) {
            context.set_breakpoint(self);
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
        pub unsafe fn enable(mut self) -> Result<Hwbp, HwbpError> {
            self.enabled = true;
            let mut context = HwbpContext::get()?;
            context.set_breakpoint(self);
            context.apply().map(|()| self)
        }

        pub unsafe fn enable_with(
            mut self,
            fetch: impl FetchContext,
            apply: impl ApplyContext,
        ) -> Result<Hwbp, HwbpError> {
            self.enabled = true;
            let mut context = HwbpContext::get_with(fetch)?;
            context.set_breakpoint(self);
            context.apply_with(apply).map(|()| self)
        }
    }

    multidoc! {
        /// Disables and applies the breakpoint.
        ///
        /// # Safety
        /// This function will never directly cause undefined behaviour, but the breakpoint it places
        /// will for obvious reasons be a breakpoint, meaning it will cause an exception to be thrown
        /// when it is hit. Calling this function is therefore unsafe, as it might affect the program
        /// in unexpected ways if the caller doesn't properly set up some form of exception handling.
        =>
        pub unsafe fn disable(mut self) -> Result<Hwbp, HwbpError> {
            self.enabled = false;
            let mut context = HwbpContext::get()?;
            context.set_breakpoint(self);
            context.apply().map(|()| self)
        }

        pub unsafe fn disable_with(
            mut self,
            fetch: impl FetchContext,
            apply: impl ApplyContext,
        ) -> Result<Hwbp, HwbpError> {
            self.enabled = false;
            let mut context = HwbpContext::get_with(fetch)?;
            context.set_breakpoint(self);
            context.apply_with(apply).map(|()| self)
        }
    }

    /// Returns a currently unused hardware breakpoint.
    ///
    /// ```
    /// # use hwbp::{Hwbp, Index};
    /// Hwbp::unused()
    ///     .expect("failed to fetch context")
    ///     .expect("no unused breakpoints");
    /// ```
    pub fn unused() -> Result<Option<Hwbp>, HwbpError> {
        Self::unused_with(FetchWith::GetThreadContext)
    }

    /// Returns a currently unused hardware breakpoint.
    ///
    /// ```
    /// # use hwbp::{Hwbp, context::FetchWith};
    /// Hwbp::unused_with(FetchWith::GetThreadContext)
    ///     .expect("failed to fetch context")
    ///     .expect("no unused breakpoints");
    /// ```
    pub fn unused_with(fetch: impl FetchContext) -> Result<Option<Hwbp>, HwbpError> {
        Ok(HwbpContext::get_with(fetch)?.unused_breakpoint())
    }
}
