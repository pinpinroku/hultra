use std::{collections::HashSet, fmt, str::FromStr};

use crate::utils;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checksums(HashSet<Checksum>);

impl Checksums {
    /// Checks if given hash is already calculated.
    pub fn contains(&self, hash: &u64) -> bool {
        self.0.contains(&Checksum(*hash))
    }
}

impl FromIterator<Checksum> for Checksums {
    fn from_iter<T: IntoIterator<Item = Checksum>>(iter: T) -> Self {
        Checksums(iter.into_iter().collect::<HashSet<Checksum>>())
    }
}

impl fmt::Display for Checksums {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, checksum) in self.0.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", checksum)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Checksum(pub u64);

#[derive(Debug, thiserror::Error)]
#[error("Hash mismatch: computed: {computed}, expected: {expected}")]
pub struct ChecksumVerificationError {
    computed: String,
    expected: Checksums,
}

impl Checksums {
    /// Verifies given checksums are equal.
    pub fn verify(&self, digest: &u64) -> Result<(), ChecksumVerificationError> {
        if self.0.contains(&Checksum(*digest)) {
            Ok(())
        } else {
            Err(ChecksumVerificationError {
                computed: format!("0x{:016x}", digest),
                expected: self.clone(),
            })
        }
    }
}

impl fmt::Display for Checksum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{:016x}", self.0)
    }
}

#[derive(Debug, thiserror::Error)]
#[error("invalid checksum: could not parse the '{input}' with digits in base 16")]
pub struct ParseError {
    pub(crate) input: String,
    #[source]
    pub(crate) source: std::num::ParseIntError,
}

impl FromStr for Checksum {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let digest = utils::from_str_digest(s).map_err(|err| ParseError {
            input: s.to_string(),
            source: err,
        })?;
        Ok(Self(digest))
    }
}

#[cfg(test)]
mod tests_checksum_verification {
    use super::*;

    fn setup_checksums(values: Vec<u64>) -> Checksums {
        Checksums(values.into_iter().map(Checksum).collect())
    }

    #[test]
    fn test_verify_success() {
        let checksums = setup_checksums(vec![0x123, 0xABC]);

        assert!(checksums.verify(&0x123).is_ok());
        assert!(checksums.verify(&0xABC).is_ok());
    }

    #[test]
    fn test_verify_mismatch() {
        let checksums = setup_checksums(vec![0x111]);
        let computed_val = 0x222;

        let result = checksums.verify(&computed_val);

        assert!(result.is_err());

        if let Err(e) = result {
            assert_eq!(e.computed, "0x0000000000000222");
            assert!(e.expected.0.contains(&Checksum(0x111)));

            let err_msg = e.to_string();
            assert!(err_msg.contains("computed: 0x0000000000000222"));
            assert!(err_msg.contains("expected: 0x0000000000000111"));
        }
    }

    #[test]
    fn test_verify_empty() {
        let checksums = setup_checksums(vec![]);
        assert!(checksums.verify(&0x123).is_err());
    }
}
