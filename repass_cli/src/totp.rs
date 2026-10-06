use crate::Result;
use repass_storage::{Totp, TotpAlgorithm};
use std::time::{SystemTime, UNIX_EPOCH};
use totp_rs::{Algorithm, Builder, Secret};

/// RFC 6238 with T0 = 0. Time is Unix seconds, independent of the local timezone.
pub(crate) fn generate_at(config: &Totp, unix_seconds: u64) -> Result<String> {
    config.validate()?;
    let secret = Secret::try_from_base32(config.secret.trim_end_matches('='))
        .map_err(|_| "invalid Base32 TOTP secret")?;
    let algorithm = match config.algorithm {
        TotpAlgorithm::Sha1 => Algorithm::SHA1,
        TotpAlgorithm::Sha256 => Algorithm::SHA256,
        TotpAlgorithm::Sha512 => Algorithm::SHA512,
    };
    // Domain validation guarantees nonempty secrets, 6–8 digits and a positive
    // period. Do not impose Builder::build's additional minimum key length:
    // existing configurations (including common 16-symbol secrets) allow less.
    let generator = Builder::new()
        .with_algorithm(algorithm)
        .with_digits(config.digits)
        .with_step_duration(u64::from(config.period))
        .with_secret(secret)
        .build_noncompliant();
    Ok(generator.generate(unix_seconds).to_string())
}

pub(crate) fn generate_current(config: &Totp) -> Result<String> {
    generate_at(config, unix_seconds(SystemTime::now())?)
}

fn unix_seconds(time: SystemTime) -> Result<u64> {
    Ok(time
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "system clock is before the Unix epoch")?
        .as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn config(key: &[u8], algorithm: TotpAlgorithm, digits: u8, period: u32) -> Totp {
        Totp::new(Secret::from(key).to_base32(), algorithm, digits, period).unwrap()
    }

    #[test]
    fn rfc6238_appendix_b_vectors_cover_all_algorithms() {
        let configs = [
            config(b"12345678901234567890", TotpAlgorithm::Sha1, 8, 30),
            config(
                b"12345678901234567890123456789012",
                TotpAlgorithm::Sha256,
                8,
                30,
            ),
            config(
                b"1234567890123456789012345678901234567890123456789012345678901234",
                TotpAlgorithm::Sha512,
                8,
                30,
            ),
        ];
        for (seconds, expected) in [
            (59, ["94287082", "46119246", "90693936"]),
            (1_111_111_109, ["07081804", "68084774", "25091201"]),
            (1_111_111_111, ["14050471", "67062674", "99943326"]),
            (1_234_567_890, ["89005924", "91819424", "93441116"]),
            (2_000_000_000, ["69279037", "90698825", "38618901"]),
            (20_000_000_000, ["65353130", "77737706", "47863826"]),
        ] {
            for (config, expected) in configs.iter().zip(expected) {
                assert_eq!(generate_at(config, seconds).unwrap(), expected);
            }
        }
    }

    #[test]
    fn digits_leading_zeros_and_period_boundaries() {
        for (digits, expected) in [(6, "081804"), (7, "7081804"), (8, "07081804")] {
            let config = config(b"12345678901234567890", TotpAlgorithm::Sha1, digits, 30);
            assert_eq!(generate_at(&config, 1_111_111_109).unwrap(), expected);
        }
        let config = config(b"12345678901234567890", TotpAlgorithm::Sha1, 6, 17);
        for (seconds, expected) in [
            (0, "755224"),
            (16, "755224"),
            (17, "287082"),
            (33, "287082"),
            (34, "359152"),
        ] {
            assert_eq!(generate_at(&config, seconds).unwrap(), expected);
        }
        assert_eq!(generate_at(&config, u64::MAX).unwrap().len(), 6);
    }

    #[test]
    fn padding_short_secrets_and_invalid_configuration() {
        let unpadded = Totp::new("MY".into(), TotpAlgorithm::Sha1, 6, 30).unwrap();
        // Public fields and persisted values can still contain canonical padding.
        let padded = Totp {
            secret: "MY======".into(),
            ..unpadded.clone()
        };
        assert_eq!(
            generate_at(&padded, 59).unwrap(),
            generate_at(&unpadded, 59).unwrap()
        );
        for config in [
            Totp {
                secret: "".into(),
                ..unpadded.clone()
            },
            Totp {
                secret: "MZ".into(),
                ..unpadded.clone()
            },
            Totp {
                secret: "my".into(),
                ..unpadded.clone()
            },
            Totp {
                digits: 10,
                ..unpadded.clone()
            },
            Totp {
                period: 0,
                ..unpadded
            },
        ] {
            assert!(generate_at(&config, 59).is_err());
        }
    }

    #[test]
    fn clock_errors_are_reported_without_panics() {
        assert_eq!(
            unix_seconds(UNIX_EPOCH + Duration::from_millis(59_999)).unwrap(),
            59
        );
        assert!(unix_seconds(UNIX_EPOCH - Duration::from_secs(1)).is_err());
    }
}
