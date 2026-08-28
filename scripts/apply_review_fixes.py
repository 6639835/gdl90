from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def write(path: str, content: str) -> None:
    (ROOT / path).write_text(content.rstrip() + "\n", encoding="utf-8")


def replace_once(path: str, old: str, new: str) -> None:
    content = read(path)
    count = content.count(old)
    if count != 1:
        raise RuntimeError(f"{path}: expected one match, found {count}")
    write(path, content.replace(old, new, 1))


replace_once(
    "src/foreflight.rs",
    '''pub struct ForeFlightCadenceProfile {
    pub ahrs_rate_hz: u8,
    pub ahrs_interval: Duration,
    pub connectivity_interval: Duration,
    pub discovery_interval: Duration,
}

pub fn cadence_profile() -> ForeFlightCadenceProfile {''',
    '''pub struct ForeFlightCadenceProfile {
    pub ahrs_rate_hz: u8,
    pub ahrs_interval: Duration,
    pub connectivity_interval: Duration,
    pub discovery_interval: Duration,
}

impl ForeFlightCadenceProfile {
    pub fn validate(self) -> Result<Self> {
        if self.ahrs_rate_hz == 0
            || self.ahrs_interval.is_zero()
            || self.connectivity_interval.is_zero()
            || self.discovery_interval.is_zero()
        {
            return Err(Gdl90Error::InvalidField {
                field: "ForeFlight cadence profile",
                details: "rate and all intervals must be greater than zero".to_string(),
            });
        }
        Ok(self)
    }
}

pub fn cadence_profile() -> ForeFlightCadenceProfile {''',
)

replace_once(
    "src/foreflight.rs",
    '''impl ForeFlightCadenceScheduler {
    pub fn new(start: Duration) -> Self {
        Self::with_profile(start, cadence_profile())
    }

    pub fn with_profile(start: Duration, profile: ForeFlightCadenceProfile) -> Self {
        Self {
            profile,
            next_ahrs: start,
            next_connectivity: start,
            next_discovery: start,
        }
    }''',
    '''impl ForeFlightCadenceScheduler {
    pub fn new(start: Duration) -> Self {
        Self {
            profile: cadence_profile(),
            next_ahrs: start,
            next_connectivity: start,
            next_discovery: start,
        }
    }

    pub fn with_profile(start: Duration, profile: ForeFlightCadenceProfile) -> Result<Self> {
        Ok(Self {
            profile: profile.validate()?,
            next_ahrs: start,
            next_connectivity: start,
            next_discovery: start,
        })
    }''',
)

replace_once(
    "src/foreflight.rs",
    '''    #[test]
    fn foreflight_message_set_allows_supported_non_connectivity_messages() {''',
    '''    #[test]
    fn cadence_profile_rejects_zero_values() {
        let valid = cadence_profile();
        let invalid = [
            ForeFlightCadenceProfile {
                ahrs_rate_hz: 0,
                ..valid
            },
            ForeFlightCadenceProfile {
                ahrs_interval: Duration::ZERO,
                ..valid
            },
            ForeFlightCadenceProfile {
                connectivity_interval: Duration::ZERO,
                ..valid
            },
            ForeFlightCadenceProfile {
                discovery_interval: Duration::ZERO,
                ..valid
            },
        ];

        for profile in invalid {
            assert!(matches!(
                ForeFlightCadenceScheduler::with_profile(Duration::ZERO, profile),
                Err(Gdl90Error::InvalidField {
                    field: "ForeFlight cadence profile",
                    ..
                })
            ));
        }
    }

    #[test]
    fn foreflight_message_set_allows_supported_non_connectivity_messages() {''',
)

replace_once(
    "src/control.rs",
    '''pub struct ControlCadenceProfile {
    pub mode_interval: Duration,
    pub call_sign_interval: Duration,
    pub vfr_code_interval: Duration,
}

pub fn cadence_profile() -> ControlCadenceProfile {''',
    '''pub struct ControlCadenceProfile {
    pub mode_interval: Duration,
    pub call_sign_interval: Duration,
    pub vfr_code_interval: Duration,
}

impl ControlCadenceProfile {
    pub fn validate(self) -> Result<Self> {
        if self.mode_interval.is_zero()
            || self.call_sign_interval.is_zero()
            || self.vfr_code_interval.is_zero()
        {
            return Err(Gdl90Error::InvalidField {
                field: "control cadence profile",
                details: "all intervals must be greater than zero".to_string(),
            });
        }
        Ok(self)
    }
}

pub fn cadence_profile() -> ControlCadenceProfile {''',
)

replace_once(
    "src/control.rs",
    '''impl ControlCadenceScheduler {
    pub fn new(start: Duration) -> Self {
        Self::with_profile(start, cadence_profile())
    }

    pub fn with_profile(start: Duration, profile: ControlCadenceProfile) -> Self {
        Self {
            profile,
            next_mode: start,
            next_call_sign: start,
            next_vfr_code: start,
            call_sign_changed: false,
        }
    }''',
    '''impl ControlCadenceScheduler {
    pub fn new(start: Duration) -> Self {
        Self {
            profile: cadence_profile(),
            next_mode: start,
            next_call_sign: start,
            next_vfr_code: start,
            call_sign_changed: false,
        }
    }

    pub fn with_profile(start: Duration, profile: ControlCadenceProfile) -> Result<Self> {
        Ok(Self {
            profile: profile.validate()?,
            next_mode: start,
            next_call_sign: start,
            next_vfr_code: start,
            call_sign_changed: false,
        })
    }''',
)

replace_once(
    "src/control.rs",
    '''        assert!(scheduler.poll(Duration::from_secs(62)).call_sign);
    }
}''',
    '''        assert!(scheduler.poll(Duration::from_secs(62)).call_sign);
    }

    #[test]
    fn control_cadence_profile_rejects_zero_intervals() {
        let valid = cadence_profile();
        let invalid = [
            ControlCadenceProfile {
                mode_interval: Duration::ZERO,
                ..valid
            },
            ControlCadenceProfile {
                call_sign_interval: Duration::ZERO,
                ..valid
            },
            ControlCadenceProfile {
                vfr_code_interval: Duration::ZERO,
                ..valid
            },
        ];

        for profile in invalid {
            assert!(matches!(
                ControlCadenceScheduler::with_profile(Duration::ZERO, profile),
                Err(Gdl90Error::InvalidField {
                    field: "control cadence profile",
                    ..
                })
            ));
        }
    }
}''',
)

replace_once(
    "README.md",
    '''## Quick start

```rust''',
    '''## Quick start

The declared minimum supported Rust version is **1.88** and is checked in CI.

```rust''',
)
