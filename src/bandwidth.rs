use crate::error::{Gdl90Error, Result};
use crate::message::Message;

#[derive(Debug, Clone, PartialEq)]
pub struct BandwidthConfig {
    pub baud_rate: u32,
    pub utilization_numerator: u32,
    pub utilization_denominator: u32,
    pub uplinks_per_second: usize,
    pub byte_budget_override: Option<usize>,
}

impl Default for BandwidthConfig {
    fn default() -> Self {
        Self {
            baud_rate: 38_400,
            utilization_numerator: 90,
            utilization_denominator: 100,
            uplinks_per_second: 4,
            byte_budget_override: None,
        }
    }
}

impl BandwidthConfig {
    pub fn validate(&self) -> Result<()> {
        if self.baud_rate == 0 {
            return Err(Gdl90Error::InvalidField {
                field: "bandwidth baud rate",
                details: "must be greater than zero".to_string(),
            });
        }
        if self.utilization_denominator == 0 {
            return Err(Gdl90Error::InvalidField {
                field: "bandwidth utilization denominator",
                details: "must be greater than zero".to_string(),
            });
        }
        if self.utilization_numerator > self.utilization_denominator {
            return Err(Gdl90Error::InvalidField {
                field: "bandwidth utilization",
                details: "numerator must not exceed denominator".to_string(),
            });
        }
        if self.uplinks_per_second > 4 {
            return Err(Gdl90Error::InvalidField {
                field: "primary uplinks per second",
                details: "Garmin Rev A permits a configured value in the range 0..=4".to_string(),
            });
        }
        if self.byte_budget_override == Some(0) {
            return Err(Gdl90Error::InvalidField {
                field: "bandwidth byte budget override",
                details: "must be greater than zero when present".to_string(),
            });
        }
        Ok(())
    }

    pub fn byte_budget_per_second(&self) -> Result<usize> {
        self.validate()?;
        if let Some(budget) = self.byte_budget_override {
            return Ok(budget);
        }

        let numerator = u64::from(self.baud_rate)
            .checked_mul(u64::from(self.utilization_numerator))
            .ok_or(Gdl90Error::InvalidField {
                field: "bandwidth byte budget",
                details: "calculation overflowed".to_string(),
            })?;
        let denominator = 10u64
            .checked_mul(u64::from(self.utilization_denominator))
            .ok_or(Gdl90Error::InvalidField {
                field: "bandwidth byte budget",
                details: "calculation overflowed".to_string(),
            })?;
        usize::try_from(numerator / denominator).map_err(|_| Gdl90Error::InvalidField {
            field: "bandwidth byte budget",
            details: "calculated budget does not fit in usize".to_string(),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TrafficCandidate {
    pub range_nm: f64,
    pub message: Message,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UplinkCandidate {
    pub station_range_nm: f64,
    pub time_slot: u8,
    pub message: Message,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScheduleInputs {
    pub heartbeat: Message,
    pub ownship: Message,
    pub alert_traffic: Vec<TrafficCandidate>,
    pub uplinks: Vec<UplinkCandidate>,
    pub proximate_traffic: Vec<TrafficCandidate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduledStage {
    Heartbeat,
    Ownship,
    AlertTraffic,
    PrimaryUplink,
    ProximateTraffic,
    SecondaryUplink,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScheduledMessage {
    pub stage: ScheduledStage,
    pub size_bytes: usize,
    pub message: Message,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScheduleResult {
    pub byte_budget: usize,
    pub used_bytes: usize,
    pub selected: Vec<ScheduledMessage>,
    pub dropped_alert_traffic: usize,
    pub dropped_proximate_traffic: usize,
    pub dropped_uplinks: usize,
    pub over_budget_due_to_mandatory_messages: bool,
}

#[derive(Debug, Clone)]
pub struct BandwidthManager {
    config: BandwidthConfig,
}

impl BandwidthManager {
    pub fn new(config: BandwidthConfig) -> Result<Self> {
        config.validate()?;
        Ok(Self { config })
    }

    pub fn config(&self) -> &BandwidthConfig {
        &self.config
    }

    pub fn schedule(&self, mut inputs: ScheduleInputs) -> Result<ScheduleResult> {
        let byte_budget = self.config.byte_budget_per_second()?;
        let mut selected = Vec::new();
        let mut used_bytes = 0usize;

        push_mandatory(
            ScheduledStage::Heartbeat,
            inputs.heartbeat,
            &mut selected,
            &mut used_bytes,
        )?;
        push_mandatory(
            ScheduledStage::Ownship,
            inputs.ownship,
            &mut selected,
            &mut used_bytes,
        )?;

        let over_budget_due_to_mandatory_messages = used_bytes > byte_budget;

        inputs
            .alert_traffic
            .sort_by(|left, right| left.range_nm.total_cmp(&right.range_nm));
        inputs
            .proximate_traffic
            .sort_by(|left, right| left.range_nm.total_cmp(&right.range_nm));

        let mut eligible_uplinks = Vec::new();
        let mut invalid_uplink_count = 0usize;
        for candidate in inputs.uplinks {
            let application_data_valid = match &candidate.message {
                Message::UplinkData(message) => {
                    message.payload.decoded_header()?.application_data_valid
                }
                _ => {
                    return Err(Gdl90Error::InvalidField {
                        field: "uplink candidate message",
                        details: "must contain a GDL90 Uplink Data message".to_string(),
                    });
                }
            };
            if application_data_valid {
                eligible_uplinks.push(candidate);
            } else {
                invalid_uplink_count += 1;
            }
        }
        eligible_uplinks.sort_by(|left, right| {
            left.station_range_nm
                .total_cmp(&right.station_range_nm)
                .then(left.time_slot.cmp(&right.time_slot))
        });

        let primary_limit = self.config.uplinks_per_second.min(eligible_uplinks.len());
        let mut primary_uplinks = eligible_uplinks.drain(..primary_limit).collect::<Vec<_>>();
        let mut secondary_uplinks = eligible_uplinks;

        let dropped_alert_traffic = schedule_traffic_group(
            ScheduledStage::AlertTraffic,
            inputs.alert_traffic,
            byte_budget,
            &mut used_bytes,
            &mut selected,
        )?;
        let dropped_primary_uplinks = schedule_uplink_group(
            ScheduledStage::PrimaryUplink,
            primary_uplinks.as_mut_slice(),
            byte_budget,
            &mut used_bytes,
            &mut selected,
        )?;
        let dropped_proximate_traffic = schedule_traffic_group(
            ScheduledStage::ProximateTraffic,
            inputs.proximate_traffic,
            byte_budget,
            &mut used_bytes,
            &mut selected,
        )?;
        let dropped_secondary_uplinks = schedule_uplink_group(
            ScheduledStage::SecondaryUplink,
            secondary_uplinks.as_mut_slice(),
            byte_budget,
            &mut used_bytes,
            &mut selected,
        )?;

        Ok(ScheduleResult {
            byte_budget,
            used_bytes,
            selected,
            dropped_alert_traffic,
            dropped_proximate_traffic,
            dropped_uplinks: invalid_uplink_count
                + dropped_primary_uplinks
                + dropped_secondary_uplinks,
            over_budget_due_to_mandatory_messages,
        })
    }
}

fn push_mandatory(
    stage: ScheduledStage,
    message: Message,
    selected: &mut Vec<ScheduledMessage>,
    used_bytes: &mut usize,
) -> Result<()> {
    let size_bytes = message.encode_frame()?.len();
    *used_bytes = used_bytes
        .checked_add(size_bytes)
        .ok_or(Gdl90Error::InvalidField {
            field: "bandwidth used bytes",
            details: "counter overflowed".to_string(),
        })?;
    selected.push(ScheduledMessage {
        stage,
        size_bytes,
        message,
    });
    Ok(())
}

fn schedule_traffic_group(
    stage: ScheduledStage,
    candidates: Vec<TrafficCandidate>,
    byte_budget: usize,
    used_bytes: &mut usize,
    selected: &mut Vec<ScheduledMessage>,
) -> Result<usize> {
    let mut dropped = 0usize;
    for candidate in candidates {
        let size_bytes = candidate.message.encode_frame()?.len();
        if let Some(total) = used_bytes.checked_add(size_bytes)
            && total <= byte_budget
        {
            *used_bytes = total;
            selected.push(ScheduledMessage {
                stage,
                size_bytes,
                message: candidate.message,
            });
        } else {
            dropped += 1;
        }
    }
    Ok(dropped)
}

fn schedule_uplink_group(
    stage: ScheduledStage,
    candidates: &mut [UplinkCandidate],
    byte_budget: usize,
    used_bytes: &mut usize,
    selected: &mut Vec<ScheduledMessage>,
) -> Result<usize> {
    let mut dropped = 0usize;
    for candidate in candidates {
        let size_bytes = candidate.message.encode_frame()?.len();
        if let Some(total) = used_bytes.checked_add(size_bytes)
            && total <= byte_budget
        {
            *used_bytes = total;
            selected.push(ScheduledMessage {
                stage,
                size_bytes,
                message: candidate.message.clone(),
            });
        } else {
            dropped += 1;
        }
    }
    Ok(dropped)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::UplinkData;
    use crate::uplink::{APPLICATION_DATA_LEN, UatUplinkHeader, UatUplinkPayload};

    fn dummy_message(id: u8, payload_len: usize) -> Message {
        Message::Unknown {
            message_id: id,
            data: vec![0u8; payload_len],
        }
    }

    fn uplink_message(application_data_valid: bool) -> Message {
        let header = UatUplinkHeader {
            position_valid: false,
            latitude_deg: 0.0,
            longitude_deg: 0.0,
            utc_coupled: false,
            application_data_valid,
            slot_id: 0,
            tisb_site_id: 0,
        }
        .encode()
        .unwrap();
        Message::UplinkData(UplinkData {
            time_of_reception: None,
            payload: UatUplinkPayload {
                header,
                application_data: [0; APPLICATION_DATA_LEN],
            },
        })
    }

    #[test]
    fn schedules_in_documented_stage_order() {
        let manager = BandwidthManager::new(BandwidthConfig {
            byte_budget_override: Some(470),
            uplinks_per_second: 1,
            ..BandwidthConfig::default()
        })
        .unwrap();

        let result = manager
            .schedule(ScheduleInputs {
                heartbeat: dummy_message(1, 0),
                ownship: dummy_message(2, 0),
                alert_traffic: vec![
                    TrafficCandidate {
                        range_nm: 5.0,
                        message: dummy_message(3, 0),
                    },
                    TrafficCandidate {
                        range_nm: 1.0,
                        message: dummy_message(4, 0),
                    },
                ],
                uplinks: vec![
                    UplinkCandidate {
                        station_range_nm: 10.0,
                        time_slot: 4,
                        message: uplink_message(true),
                    },
                    UplinkCandidate {
                        station_range_nm: 2.0,
                        time_slot: 1,
                        message: uplink_message(true),
                    },
                ],
                proximate_traffic: vec![TrafficCandidate {
                    range_nm: 3.0,
                    message: dummy_message(8, 0),
                }],
            })
            .unwrap();

        let stages_and_ids = result
            .selected
            .iter()
            .map(|scheduled| (scheduled.stage, scheduled.message.message_id()))
            .collect::<Vec<_>>();

        assert_eq!(
            stages_and_ids,
            vec![
                (ScheduledStage::Heartbeat, 1),
                (ScheduledStage::Ownship, 2),
                (ScheduledStage::AlertTraffic, 4),
                (ScheduledStage::AlertTraffic, 3),
                (ScheduledStage::PrimaryUplink, 7),
                (ScheduledStage::ProximateTraffic, 8),
            ]
        );
        assert_eq!(result.dropped_uplinks, 1);
        assert_eq!(result.dropped_alert_traffic, 0);
        assert_eq!(result.dropped_proximate_traffic, 0);
    }

    #[test]
    fn derives_application_data_validity_from_the_uplink_header() {
        let manager = BandwidthManager::new(BandwidthConfig {
            byte_budget_override: Some(500),
            uplinks_per_second: 4,
            ..BandwidthConfig::default()
        })
        .unwrap();

        let result = manager
            .schedule(ScheduleInputs {
                heartbeat: dummy_message(1, 0),
                ownship: dummy_message(2, 0),
                alert_traffic: Vec::new(),
                uplinks: vec![
                    UplinkCandidate {
                        station_range_nm: 1.0,
                        time_slot: 0,
                        message: uplink_message(false),
                    },
                    UplinkCandidate {
                        station_range_nm: 1.0,
                        time_slot: 1,
                        message: uplink_message(true),
                    },
                ],
                proximate_traffic: Vec::new(),
            })
            .unwrap();

        let ids = result
            .selected
            .iter()
            .map(|scheduled| scheduled.message.message_id())
            .collect::<Vec<_>>();
        assert_eq!(ids, vec![1, 2, 7]);
        assert_eq!(result.dropped_uplinks, 1);
    }

    #[test]
    fn rejects_invalid_configuration_instead_of_panicking() {
        let error = BandwidthManager::new(BandwidthConfig {
            utilization_denominator: 0,
            ..BandwidthConfig::default()
        })
        .unwrap_err();
        assert!(matches!(
            error,
            Gdl90Error::InvalidField {
                field: "bandwidth utilization denominator",
                ..
            }
        ));

        assert!(
            BandwidthManager::new(BandwidthConfig {
                uplinks_per_second: 5,
                ..BandwidthConfig::default()
            })
            .is_err()
        );
    }

    #[test]
    fn rejects_non_uplink_candidates() {
        let manager = BandwidthManager::new(BandwidthConfig {
            byte_budget_override: Some(500),
            ..BandwidthConfig::default()
        })
        .unwrap();
        let error = manager
            .schedule(ScheduleInputs {
                heartbeat: dummy_message(1, 0),
                ownship: dummy_message(2, 0),
                alert_traffic: Vec::new(),
                uplinks: vec![UplinkCandidate {
                    station_range_nm: 1.0,
                    time_slot: 0,
                    message: dummy_message(3, 0),
                }],
                proximate_traffic: Vec::new(),
            })
            .unwrap_err();
        assert!(matches!(
            error,
            Gdl90Error::InvalidField {
                field: "uplink candidate message",
                ..
            }
        ));
    }

    #[test]
    fn marks_over_budget_if_mandatory_messages_alone_exceed_budget() {
        let manager = BandwidthManager::new(BandwidthConfig {
            byte_budget_override: Some(8),
            ..BandwidthConfig::default()
        })
        .unwrap();

        let result = manager
            .schedule(ScheduleInputs {
                heartbeat: dummy_message(1, 0),
                ownship: dummy_message(2, 0),
                alert_traffic: Vec::new(),
                uplinks: Vec::new(),
                proximate_traffic: Vec::new(),
            })
            .unwrap();

        assert!(result.over_budget_due_to_mandatory_messages);
        assert_eq!(result.selected.len(), 2);
    }
}
