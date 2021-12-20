use winapi::um::winnt::CONTEXT;

use crate::{registers::Dr7, Hwbp, Index};

/// Writes a breakpoint to the provided context.
pub fn set_breakpoint(context: &mut CONTEXT, bp: Hwbp) {
    *match bp.index {
        Index::First => &mut context.Dr0,
        Index::Second => &mut context.Dr1,
        Index::Third => &mut context.Dr2,
        Index::Fourth => &mut context.Dr3,
    } = bp.address as _;

    // Set the condition, size, and enabled bits.
    context.Dr7 = Dr7(context.Dr7 as _)
        .with_size(bp.index, bp.size)
        .with_condition(bp.index, bp.condition)
        .with_enabled_local(bp.index, bp.enabled)
        .0 as _;
}

/// Reads a breakpoint from the provided context.
#[must_use]
pub fn get_breakpoint(context: &CONTEXT, index: Index) -> Hwbp {
    let address = match index {
        Index::First => context.Dr0,
        Index::Second => context.Dr1,
        Index::Third => context.Dr2,
        Index::Fourth => context.Dr3,
    } as _;

    let dr7 = Dr7(context.Dr7 as _);
    Hwbp {
        enabled: dr7.enabled_local(index),
        index,
        address,
        size: dr7.size(index),
        condition: dr7.condition(index),
    }
}

/// Returns a currently unused breakpoint, unless all are already in-use.
pub fn unused_breakpoint(context: &CONTEXT) -> Option<Hwbp> {
    get_breakpoints(context).find(|bp| !bp.enabled)
}

/// Returns all breakpoints that overlap with the specified address.
///
/// This does not check if the breakpoints are enabled or not.
pub fn get_breakpoints_by_address<'a, T: 'a>(
    context: &'a CONTEXT,
    address: *const T,
) -> impl Iterator<Item = Hwbp> + 'a {
    get_breakpoints(context).filter(move |x| unsafe {
        let from = x.address.cast::<u8>();
        let to = from.add(x.size.in_bytes());
        (from..to).contains(&(address as _))
    })
}

/// Returns all breakpoints.
pub fn get_breakpoints(context: &CONTEXT) -> impl Iterator<Item = Hwbp> + '_ {
    [Index::First, Index::Second, Index::Third, Index::Fourth]
        .into_iter()
        .map(move |idx| get_breakpoint(context, idx))
}

/// Fully clear any currently set breakpoints.
pub fn clear_breakpoints(context: &mut CONTEXT) {
    context.Dr7 = Dr7(context.Dr7 as _).clear_breakpoints().0 as _;
    context.Dr0 = 0;
    context.Dr1 = 0;
    context.Dr2 = 0;
    context.Dr3 = 0;
}
