use winapi::um::winnt::CONTEXT;

use crate::{Condition, HardwareBreakpoint, Index, Size, WinAPIHatesUsize};

/// Writes a breakpoint to the provided context.
pub fn set_breakpoint(context: &mut CONTEXT, bp: HardwareBreakpoint) {
    let hwbp_index = bp.index as WinAPIHatesUsize;
    let stat_offset = 2 * hwbp_index;
    let info_offset = 4 * hwbp_index + 16;
    let cond_offset = info_offset;
    let size_offset = info_offset + 2;

    *match bp.index {
        Index::First => &mut context.Dr0,
        Index::Second => &mut context.Dr1,
        Index::Third => &mut context.Dr2,
        Index::Fourth => &mut context.Dr3,
    } = bp.address as _;

    let cond = bp.condition as WinAPIHatesUsize;
    let size = bp.size.as_bits() as WinAPIHatesUsize;

    // Clear and write condition and size
    context.Dr7 &= !(0b1111 << info_offset);
    context.Dr7 |= (cond & 0b11) << cond_offset;
    context.Dr7 |= (size & 0b11) << size_offset;

    // Clear and write enabled flag
    context.Dr7 &= !(1 << stat_offset);
    context.Dr7 |= (bp.enabled as WinAPIHatesUsize) << stat_offset;
}

/// Reads a breakpoint from the provided context.
pub fn get_breakpoint(context: &CONTEXT, index: Index) -> HardwareBreakpoint {
    let hwbp_index = index as u8;
    let stat_offset = 2 * hwbp_index;
    let cond_offset = 4 * hwbp_index + 16;
    let size_offset = 4 * hwbp_index + 18;

    let address = match index {
        Index::First => context.Dr0,
        Index::Second => context.Dr1,
        Index::Third => context.Dr2,
        Index::Fourth => context.Dr3,
    } as _;

    let size = Size::from_bits((context.Dr7 >> size_offset & 0b11) as u8)
        .expect("Can not be hit since all patterns & 0b11 are valid.");

    let cond = Condition::from_bits((context.Dr7 >> cond_offset & 0b11) as u8)
        .expect("Can not be hit since all patterns & 0b11 are valid.");

    HardwareBreakpoint {
        enabled: context.Dr7 & (1 << stat_offset) != 0,
        index,
        address,
        size,
        condition: cond,
    }
}

/// Returns a currently unused breakpoint, unless all are already in-use.
pub fn unused_breakpoint(context: &CONTEXT) -> Option<HardwareBreakpoint> {
    get_breakpoints(context).find(|bp| !bp.enabled)
}

/// Returns all breakpoints that overlap with the specified address.
///
/// This does not check if the breakpoints are enabled or not.
pub fn get_breakpoints_by_address<'a, T: 'a>(
    context: &'a CONTEXT,
    address: *const T,
) -> impl Iterator<Item = HardwareBreakpoint> + 'a {
    get_breakpoints(context).filter(move |x| unsafe {
        let from = x.address.cast::<u8>();
        let to = from.add(x.size.in_bytes());
        (from..to).contains(&(address as _))
    })
}

/// Returns all breakpoints.
pub fn get_breakpoints(context: &CONTEXT) -> impl Iterator<Item = HardwareBreakpoint> + '_ {
    IntoIterator::into_iter([Index::First, Index::Second, Index::Third, Index::Fourth])
        .map(move |idx| get_breakpoint(context, idx))
}

/// Fully clear any currently set breakpoints.
pub fn clear_breakpoints(context: &mut CONTEXT) {
    context.Dr7 &= 0b00000000000000001111111100000000;
    context.Dr0 = 0;
    context.Dr1 = 0;
    context.Dr2 = 0;
    context.Dr3 = 0;
}
