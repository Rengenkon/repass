use crate::StorageError;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::net::IpAddr;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

/// A protocol-independent IP address or domain name.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub enum Host {
    IP(IpAddr),
    Domain(String),
}

impl Host {
    pub fn validate(&self) -> Result<(), StorageError> {
        if let Self::Domain(domain) = self {
            let domain = domain.strip_suffix('.').unwrap_or(domain);
            if domain.is_empty()
                || domain.len() > 253
                || domain.parse::<IpAddr>().is_ok()
                || domain.split('.').any(|label| {
                    label.is_empty()
                        || label.len() > 63
                        || label.starts_with('-')
                        || label.ends_with('-')
                        || !label.chars().all(|c| c.is_alphanumeric() || c == '-')
                })
            {
                return Err(StorageError::InvalidState(
                    "invalid domain name; provide a host without protocol, port or path",
                ));
            }
        }
        Ok(())
    }

    pub fn matches(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::IP(a), Self::IP(b)) => a == b,
            (Self::Domain(a), Self::Domain(b)) => {
                a.trim_end_matches('.').to_lowercase() == b.trim_end_matches('.').to_lowercase()
            }
            _ => false,
        }
    }
}

impl FromStr for Host {
    type Err = StorageError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let host = value
            .parse()
            .map(Self::IP)
            .unwrap_or_else(|_| Self::Domain(value.to_owned()));
        host.validate()?;
        Ok(host)
    }
}

impl Display for Host {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IP(ip) => ip.fmt(f),
            Self::Domain(domain) => domain.fmt(f),
        }
    }
}

/// At least one nonblank key must be present. Key text is preserved verbatim.
#[derive(Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct SshKey {
    pub private_key: Option<String>,
    pub public_key: Option<String>,
}

impl SshKey {
    pub fn new(
        private_key: Option<String>,
        public_key: Option<String>,
    ) -> Result<Self, StorageError> {
        let key = Self {
            private_key,
            public_key,
        };
        key.validate()?;
        Ok(key)
    }

    pub fn validate(&self) -> Result<(), StorageError> {
        if self.private_key.is_none() && self.public_key.is_none() {
            return Err(StorageError::InvalidState(
                "SSH requires a private and/or public key",
            ));
        }
        if self
            .private_key
            .iter()
            .chain(self.public_key.iter())
            .any(|key| key.trim().is_empty())
        {
            return Err(StorageError::InvalidState("SSH key fields cannot be blank"));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub enum TotpAlgorithm {
    Sha1,
    Sha256,
    Sha512,
}

/// Configuration only; this type does not calculate one-time codes.
#[derive(Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Totp {
    /// RFC 4648 Base32, uppercase. Constructors strip canonical padding;
    /// validation also accepts padded values loaded from disk.
    pub secret: String,
    pub algorithm: TotpAlgorithm,
    pub digits: u8,
    pub period: u32,
}

impl Totp {
    pub fn new(
        secret: String,
        algorithm: TotpAlgorithm,
        digits: u8,
        period: u32,
    ) -> Result<Self, StorageError> {
        let mut totp = Self {
            secret,
            algorithm,
            digits,
            period,
        };
        totp.validate()?;
        totp.secret
            .truncate(totp.secret.trim_end_matches('=').len());
        Ok(totp)
    }

    pub fn validate(&self) -> Result<(), StorageError> {
        let unpadded = self.secret.trim_end_matches('=');
        let remainder = unpadded.len() % 8;
        let padding = self.secret.len() - unpadded.len();
        let expected_padding = match remainder {
            0 => 0,
            2 => 6,
            4 => 4,
            5 => 3,
            7 => 1,
            _ => {
                return Err(StorageError::InvalidState(
                    "invalid Base32 TOTP secret length",
                ));
            }
        };
        if unpadded.is_empty()
            || !unpadded
                .bytes()
                .all(|c| c.is_ascii_uppercase() || (b'2'..=b'7').contains(&c))
            || (padding != 0 && padding != expected_padding)
        {
            return Err(StorageError::InvalidState(
                "TOTP secret must be uppercase RFC 4648 Base32",
            ));
        }
        // Unused bits in the final Base32 symbol must be zero.
        let last = unpadded.as_bytes()[unpadded.len() - 1];
        let value = if last.is_ascii_uppercase() {
            last - b'A'
        } else {
            last - b'2' + 26
        };
        let mask = match remainder {
            2 => 3,
            4 => 15,
            5 => 1,
            7 => 7,
            _ => 0,
        };
        if value & mask != 0 {
            return Err(StorageError::InvalidState(
                "noncanonical Base32 TOTP secret",
            ));
        }
        if !(6..=8).contains(&self.digits) {
            return Err(StorageError::InvalidState(
                "TOTP digits must be between 6 and 8",
            ));
        }
        if self.period == 0 {
            return Err(StorageError::InvalidState("TOTP period must be positive"));
        }
        Ok(())
    }
}

/// Secret-bearing values intentionally do not implement Debug.
#[derive(Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum Data {
    Password(String),
    SshKey(SshKey),
    Totp(Totp),
    Code(String),
}

impl Data {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Password(_) => "password",
            Self::SshKey(_) => "ssh",
            Self::Totp(_) => "totp",
            Self::Code(_) => "code",
        }
    }

    pub fn validate(&self) -> Result<(), StorageError> {
        match self {
            Self::SshKey(key) => key.validate(),
            Self::Totp(totp) => totp.validate(),
            Self::Code(code) if code.trim().is_empty() => {
                Err(StorageError::InvalidState("code cannot be blank"))
            }
            _ => Ok(()),
        }
    }
}

#[derive(Clone, Copy, Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct Timestamp(u64);

impl Timestamp {
    /// Creates a timestamp from milliseconds since the Unix epoch.
    pub fn from_unix_millis(milliseconds: u64) -> Self {
        Self(milliseconds)
    }

    /// Returns milliseconds since the Unix epoch.
    pub fn as_unix_millis(&self) -> u64 {
        self.0
    }

    pub(crate) fn now() -> Result<Self, crate::StorageError> {
        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| crate::StorageError::Clock)?;
        let milliseconds =
            u64::try_from(duration.as_millis()).map_err(|_| crate::StorageError::Clock)?;
        Ok(Self(milliseconds))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ssh_requires_at_least_one_nonblank_key_and_preserves_text() {
        let private = "-----BEGIN OPENSSH PRIVATE KEY-----\r\n猫 key  \r\n-----END OPENSSH PRIVATE KEY-----\r\n";
        let public = "ssh-ed25519 AAAA user@host\n";
        for (private_key, public_key) in [
            (Some(private.into()), None),
            (None, Some(public.into())),
            (Some(private.into()), Some(public.into())),
        ] {
            let key = SshKey::new(private_key.clone(), public_key.clone()).unwrap();
            assert_eq!(key.private_key, private_key);
            assert_eq!(key.public_key, public_key);
            let bytes = postcard::to_allocvec(&key).unwrap();
            let restored: SshKey = postcard::from_bytes(&bytes).unwrap();
            assert!(restored == key);
        }
        for (private, public) in [
            (None, None),
            (Some("".into()), None),
            (None, Some(" \n".into())),
            (Some("key".into()), Some(" ".into())),
        ] {
            assert!(SshKey::new(private, public).is_err());
        }
    }

    #[test]
    fn totp_checks_base32_and_configuration_without_generating_codes() {
        for secret in [
            "MY", "MY======", "MZXQ", "MZXQ====", "MZXW6", "MZXW6===", "MZXW6YQ", "MZXW6YQ=",
            "MZXW6YTB",
        ] {
            for algorithm in [
                TotpAlgorithm::Sha1,
                TotpAlgorithm::Sha256,
                TotpAlgorithm::Sha512,
            ] {
                for digits in 6..=8 {
                    let totp = Totp::new(secret.into(), algorithm, digits, 30).unwrap();
                    assert_eq!(totp.secret, secret.trim_end_matches('='));
                    let bytes = postcard::to_allocvec(&totp).unwrap();
                    let restored: Totp = postcard::from_bytes(&bytes).unwrap();
                    assert!(restored == totp);
                }
            }
        }
        for secret in [
            "",
            "A",
            "AAA",
            "AAAAAA",
            "my",
            "MY=",
            "MY=======",
            "M=Y",
            "M1",
            "MZ",
            "MZXR",
            "MZXW7",
            "MZXW6YR",
            "MZXW6YTB=",
            "猫",
        ] {
            assert!(
                Totp::new(secret.into(), TotpAlgorithm::Sha1, 6, 30).is_err(),
                "{secret}"
            );
        }
        for (digits, period) in [(0, 30), (5, 30), (9, 30), (6, 0)] {
            assert!(Totp::new("MY".into(), TotpAlgorithm::Sha1, digits, period).is_err());
        }
    }

    #[test]
    fn host_matching_and_round_trips_cover_domains_ipv4_and_ipv6() {
        for text in ["Example.TEST.", "127.0.0.1", "2001:db8::1", "猫.example"] {
            let host: Host = text.parse().unwrap();
            let bytes = postcard::to_allocvec(&host).unwrap();
            let restored: Host = postcard::from_bytes(&bytes).unwrap();
            assert_eq!(restored, host);
        }
        assert!(
            "Example.TEST."
                .parse::<Host>()
                .unwrap()
                .matches(&"example.test".parse().unwrap())
        );
        assert!(
            "2001:db8::1"
                .parse::<Host>()
                .unwrap()
                .matches(&"2001:0db8:0:0:0:0:0:1".parse().unwrap())
        );
        for text in [
            "",
            ".",
            "https://example.test",
            "example.test:22",
            "example.test/path",
            "a..b",
            "-host",
            "host-",
            "example.test..",
            "a b",
        ] {
            assert!(text.parse::<Host>().is_err(), "{text}");
        }
        assert!(Host::Domain("127.0.0.1".into()).validate().is_err());
    }

    #[test]
    fn every_data_variant_and_timestamp_round_trips() {
        let values = [
            Data::Password("猫 secret".into()),
            Data::SshKey(
                SshKey::new(Some("private\nkey\n".into()), Some("public\n".into())).unwrap(),
            ),
            Data::Totp(Totp::new("MZXW6YTB".into(), TotpAlgorithm::Sha512, 8, 60).unwrap()),
            Data::Code("recovery-猫".into()),
        ];
        for value in values {
            let bytes = postcard::to_allocvec(&value).unwrap();
            let restored: Data = postcard::from_bytes(&bytes).unwrap();
            assert!(restored == value);
        }
        let timestamp = Timestamp::from_unix_millis(u64::MAX);
        let bytes = postcard::to_allocvec(&timestamp).unwrap();
        assert_eq!(
            postcard::from_bytes::<Timestamp>(&bytes).unwrap(),
            timestamp
        );
        assert!(Data::Code(" \n".into()).validate().is_err());
    }
}
