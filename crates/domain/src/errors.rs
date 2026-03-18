#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    NullValueNotAllowed,
    ExpectedString,
    ExpectedInteger,
    ExpectedNumber,
    ExpectedBoolean,
    MoneyMustBeDecimalString,
    InvalidIpv4,
    EnumMustBeStringOptionKey,
    ExternalEntityMustBeIntegerId,
}

impl core::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let msg = match self {
            ValidationError::NullValueNotAllowed => "null is not an allowed stored tag value",
            ValidationError::ExpectedString => "expected a string value",
            ValidationError::ExpectedInteger => "expected an integer value",
            ValidationError::ExpectedNumber => "expected a numeric value",
            ValidationError::ExpectedBoolean => "expected a boolean value",
            ValidationError::MoneyMustBeDecimalString => "money value must be a decimal string",
            ValidationError::InvalidIpv4 => "invalid ipv4 value",
            ValidationError::EnumMustBeStringOptionKey => "enum value must be a string option key",
            ValidationError::ExternalEntityMustBeIntegerId => {
                "external entity value must be an integer id"
            }
        };

        f.write_str(msg)
    }
}

impl std::error::Error for ValidationError {}
