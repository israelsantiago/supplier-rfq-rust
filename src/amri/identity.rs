use std::{fmt, num::NonZeroU32};

const MAX_IDENTIFIER_BYTES: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentifierError {
    Empty,
    TooLong,
    InvalidCharacter,
}

impl fmt::Display for IdentifierError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for IdentifierError {}

fn validate_identifier(value: &str) -> Result<(), IdentifierError> {
    if value.is_empty() {
        return Err(IdentifierError::Empty);
    }
    if value.len() > MAX_IDENTIFIER_BYTES {
        return Err(IdentifierError::TooLong);
    }

    let bytes = value.as_bytes();
    let first = bytes[0];
    if !first.is_ascii_alphanumeric()
        || !bytes[1..].iter().copied().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b':' | b'/')
        })
    {
        return Err(IdentifierError::InvalidCharacter);
    }
    Ok(())
}

macro_rules! identifier_type {
    ($name:ident) => {
        #[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, IdentifierError> {
                let value = value.into();
                validate_identifier(&value)?;
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter
                    .debug_tuple(stringify!($name))
                    .field(&self.0)
                    .finish()
            }
        }
    };
}

identifier_type!(InteractionId);
identifier_type!(InteractionDefinitionId);
identifier_type!(ParticipantId);
identifier_type!(FactId);
identifier_type!(FactTypeId);
identifier_type!(CorrelationId);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FingerprintError {
    InvalidLength,
    InvalidHex,
}

impl fmt::Display for FingerprintError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for FingerprintError {}

/// Opaque 256-bit digest supplied by an application-owned canonical encoder.
///
/// AMRI Core intentionally does not choose serialization or hashing rules. The application
/// computes the digest from a versioned canonical representation and passes the bytes here.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Fingerprint([u8; 32]);

impl Fingerprint {
    pub const LENGTH: usize = 32;

    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn from_hex(value: &str) -> Result<Self, FingerprintError> {
        if value.len() != Self::LENGTH * 2 {
            return Err(FingerprintError::InvalidLength);
        }
        if !value.is_ascii() {
            return Err(FingerprintError::InvalidHex);
        }

        let bytes = value.as_bytes();
        let mut digest = [0_u8; Self::LENGTH];
        for (index, pair) in bytes.chunks_exact(2).enumerate() {
            let high = hex_nibble(pair[0]).ok_or(FingerprintError::InvalidHex)?;
            let low = hex_nibble(pair[1]).ok_or(FingerprintError::InvalidHex)?;
            digest[index] = (high << 4) | low;
        }
        Ok(Self(digest))
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(self) -> String {
        let mut output = String::with_capacity(Self::LENGTH * 2);
        for byte in self.0 {
            use std::fmt::Write as _;
            let _ = write!(output, "{byte:02x}");
        }
        output
    }
}

impl fmt::Debug for Fingerprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("Fingerprint")
            .field(&self.to_hex())
            .finish()
    }
}

impl fmt::Display for Fingerprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_hex())
    }
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InteractionIdentity {
    interaction_id: InteractionId,
    definition_id: InteractionDefinitionId,
    definition_version: NonZeroU32,
    request_fingerprint: Fingerprint,
}

impl InteractionIdentity {
    pub fn new(
        interaction_id: InteractionId,
        definition_id: InteractionDefinitionId,
        definition_version: NonZeroU32,
        request_fingerprint: Fingerprint,
    ) -> Self {
        Self {
            interaction_id,
            definition_id,
            definition_version,
            request_fingerprint,
        }
    }

    pub fn interaction_id(&self) -> &InteractionId {
        &self.interaction_id
    }

    pub fn definition_id(&self) -> &InteractionDefinitionId {
        &self.definition_id
    }

    pub const fn definition_version(&self) -> NonZeroU32 {
        self.definition_version
    }

    pub const fn request_fingerprint(&self) -> Fingerprint {
        self.request_fingerprint
    }

    /// Confirms that two attempts represent the same logical interaction.
    /// Reusing an InteractionId with a different definition or request is rejected.
    pub fn ensure_same_logical_request(&self, candidate: &Self) -> Result<(), IdentityReuseError> {
        if self.interaction_id != candidate.interaction_id {
            return Err(IdentityReuseError::DifferentInteractionId);
        }
        if self.definition_id != candidate.definition_id {
            return Err(IdentityReuseError::DifferentDefinition);
        }
        if self.definition_version != candidate.definition_version {
            return Err(IdentityReuseError::DifferentDefinitionVersion);
        }
        if self.request_fingerprint != candidate.request_fingerprint {
            return Err(IdentityReuseError::RequestFingerprintMismatch);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityReuseError {
    DifferentInteractionId,
    DifferentDefinition,
    DifferentDefinitionVersion,
    RequestFingerprintMismatch,
}

impl fmt::Display for IdentityReuseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for IdentityReuseError {}
