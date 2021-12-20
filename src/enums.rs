use std::convert::TryFrom;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Condition {
    /// `Condition::Execution` must be paired with `Size::One`.
    /// Any other size will result in the breakpoint not being hit.
    Execution = 0b00,
    Write = 0b01,
    ReadWrite = 0b11,
    IoReadWrite = 0b10,
}

impl Condition {
    pub const fn from_bits(bits: u8) -> Option<Condition> {
        match bits {
            0b00 => Some(Condition::Execution),
            0b01 => Some(Condition::Write),
            0b11 => Some(Condition::ReadWrite),
            0b10 => Some(Condition::IoReadWrite),
            _ => None,
        }
    }

    pub const fn as_bits(self) -> u8 {
        self as u8
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Index {
    First = 0,
    Second = 1,
    Third = 2,
    Fourth = 3,
}

impl Index {
    /// Returns the index of the breakpoint that triggered the current exception.
    ///
    /// Keep in mind that `Dr6` is not guaranteed to be automatically cleared, so you should clear
    /// it manually in your exception handler for it to contain useful information.
    ///
    /// If `Dr6` does not have exactly one hwbp flag set, this function will return `None`.
    pub fn by_dr6(dr6: usize) -> Option<Index> {
        match dr6 & 0b1111 {
            0b0001 => Some(Index::First),
            0b0010 => Some(Index::Second),
            0b0100 => Some(Index::Third),
            0b1000 => Some(Index::Fourth),
            _ => None,
        }
    }
}

impl TryFrom<u8> for Index {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            // TODO: When `inline-const` is stabilized, rewrite the branches below.
            // const { Index::First as _ } => ...,
            x if x == Index::First as _ => Ok(Index::First),
            x if x == Index::Second as _ => Ok(Index::Second),
            x if x == Index::Third as _ => Ok(Index::Third),
            x if x == Index::Fourth as _ => Ok(Index::Fourth),
            _ => Err(()),
        }
    }
}

// Since it is not obvious which representation this enum resolves to when `as` is used to cast it,
// we simply let it resolve to the default representation instead of picking one of the two.
//
// This lets us force the user to be explicit about which representation they want to use.
// Preferably we'd also forbid `as` from being used on this enum, but that's not possible yet.
//
// If it ever becomes possible to forbid `as` from being used on this enum, we should do so.

/// An enum representing the size of a hardware breakpoint.
///
/// **Avoid using `as` to cast this enum to a number, it will not return what you expect it to.**
///
/// Instead, use `Size::in_bytes` and `Size::as_bits`.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Size {
    One,
    Two,
    Four,
    /// Eight byte breakpoints are only supported in 64-bit context.
    Eight,
}

impl Size {
    /// Returns how many bytes a hardware breakpoint using this `Size` would cover.
    pub const fn in_bytes(self) -> usize {
        match self {
            Size::One => 1,
            Size::Two => 2,
            Size::Four => 4,
            Size::Eight => 8,
        }
    }

    /// Returns the two-bit representation used in `CONTEXT.Dr7`.
    pub const fn as_bits(self) -> u8 {
        match self {
            Size::One => 0b00,
            Size::Two => 0b01,
            Size::Four => 0b11,
            Size::Eight => 0b10,
        }
    }

    /// Returns the `Size` that corresponds to the two-bit representation in `CONTEXT.Dr7`.
    pub const fn from_bits(bits: u8) -> Option<Self> {
        match bits {
            0b00 => Some(Size::One),
            0b01 => Some(Size::Two),
            0b11 => Some(Size::Four),
            0b10 => Some(Size::Eight),
            _ => None,
        }
    }
}
