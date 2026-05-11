#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// Payload is shorter than the minimum valid length.
    TooShort { expected: usize, actual: usize },
    /// Flags declare fields that extend beyond the payload.
    UnexpectedEnd { offset: usize, needed: usize, remaining: usize },
}

impl core::fmt::Display for ParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::TooShort { expected, actual } => {
                write!(f, "payload too short: expected at least {expected} bytes, got {actual}")
            }
            Self::UnexpectedEnd { offset, needed, remaining } => {
                write!(
                    f,
                    "unexpected end at offset {offset}: need {needed} bytes, {remaining} remaining"
                )
            }
        }
    }
}
