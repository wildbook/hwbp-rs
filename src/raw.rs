use winapi::um::winnt::CONTEXT;

use crate::{registers::Dr7, Hwbp, Index};

/// Reads a breakpoint from the provided context.
#[must_use]
pub fn get_breakpoint(context: &CONTEXT, index: Index) -> Hwbp {
    let address = match index {
        Index::First => context.Dr0,
        Index::Second => context.Dr1,
        Index::Third => context.Dr2,
        Index::Fourth => context.Dr3,
    } as _;

    let dr7 = Dr7(context.Dr7);
    Hwbp {
        enabled: dr7.enabled_local(index),
        index,
        address,
        size: dr7.size(index),
        condition: dr7.condition(index),
    }
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
