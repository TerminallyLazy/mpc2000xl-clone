use mpc_core::MidiOutputIntent;
use serde::{Deserialize, Serialize};
use std::fmt;

const MIDI_MIN_CHANNEL: u8 = 1;
const MIDI_MAX_CHANNEL: u8 = 16;
const MIDI_MAX_NOTE: u8 = 127;
const MIDI_MAX_VELOCITY: u8 = 127;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostMidiMode {
    Disabled,
    Capture,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostMidiState {
    pub mode: HostMidiMode,
    pub backend_name: String,
    pub queued_message_count: u64,
    pub ignored_message_count: u64,
    pub failed_message_count: u64,
    pub last_event: Option<HostMidiEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MidiMessage {
    pub channel: u8,
    pub note: u8,
    pub velocity: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostMidiBackendReceipt {
    pub queued: bool,
    pub message_count: u64,
}

impl HostMidiBackendReceipt {
    pub fn queued(message_count: u64) -> Self {
        Self {
            queued: true,
            message_count,
        }
    }

    pub fn not_queued(message_count: u64) -> Self {
        Self {
            queued: false,
            message_count,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostMidiOutputReceipt {
    pub intent: MidiOutputIntent,
    pub message: MidiMessage,
    pub backend_receipt: HostMidiBackendReceipt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HostMidiEvent {
    Queued {
        receipt: HostMidiOutputReceipt,
    },
    Ignored {
        reason: String,
        intent: MidiOutputIntent,
    },
    Failed {
        error: HostMidiError,
        intent: MidiOutputIntent,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostMidiOutputReport {
    pub event: HostMidiEvent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HostMidiError {
    InvalidIntent { field: String, message: String },
    Backend { message: String },
}

impl fmt::Display for HostMidiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidIntent { field, message } => {
                write!(
                    formatter,
                    "invalid MIDI output intent field {field}: {message}"
                )
            }
            Self::Backend { message } => write!(formatter, "host MIDI backend failed: {message}"),
        }
    }
}

impl std::error::Error for HostMidiError {}

pub trait HostMidiBackend {
    fn name(&self) -> &'static str;
    fn send(&mut self, message: MidiMessage) -> Result<HostMidiBackendReceipt, HostMidiError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureMidiBackend {
    retained_message_capacity: usize,
    messages: Vec<MidiMessage>,
    total_sent_count: u64,
}

impl CaptureMidiBackend {
    pub fn new(retained_message_capacity: usize) -> Self {
        Self {
            retained_message_capacity,
            messages: Vec::new(),
            total_sent_count: 0,
        }
    }

    pub fn messages(&self) -> &[MidiMessage] {
        &self.messages
    }

    pub fn total_sent_count(&self) -> u64 {
        self.total_sent_count
    }
}

impl HostMidiBackend for CaptureMidiBackend {
    fn name(&self) -> &'static str {
        "capture"
    }

    fn send(&mut self, message: MidiMessage) -> Result<HostMidiBackendReceipt, HostMidiError> {
        self.total_sent_count = self.total_sent_count.saturating_add(1);
        if self.retained_message_capacity > 0 {
            if self.messages.len() == self.retained_message_capacity {
                self.messages.remove(0);
            }
            self.messages.push(message);
        }
        Ok(HostMidiBackendReceipt::queued(self.total_sent_count))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostMidiEngine<B: HostMidiBackend> {
    enabled: bool,
    backend: B,
    queued_message_count: u64,
    ignored_message_count: u64,
    failed_message_count: u64,
    last_event: Option<HostMidiEvent>,
}

impl<B: HostMidiBackend> HostMidiEngine<B> {
    pub fn new(backend: B) -> Self {
        Self {
            enabled: false,
            backend,
            queued_message_count: 0,
            ignored_message_count: 0,
            failed_message_count: 0,
            last_event: None,
        }
    }

    pub fn enabled(backend: B) -> Self {
        let mut engine = Self::new(backend);
        engine.enabled = true;
        engine
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn backend(&self) -> &B {
        &self.backend
    }

    pub fn state(&self) -> HostMidiState {
        HostMidiState {
            mode: if self.enabled {
                HostMidiMode::Capture
            } else {
                HostMidiMode::Disabled
            },
            backend_name: self.backend.name().to_string(),
            queued_message_count: self.queued_message_count,
            ignored_message_count: self.ignored_message_count,
            failed_message_count: self.failed_message_count,
            last_event: self.last_event.clone(),
        }
    }

    pub fn send_intent(&mut self, intent: &MidiOutputIntent) -> HostMidiOutputReport {
        if !self.enabled {
            let event = HostMidiEvent::Ignored {
                reason: "host midi disabled".to_string(),
                intent: intent.clone(),
            };
            self.ignored_message_count = self.ignored_message_count.saturating_add(1);
            self.last_event = Some(event.clone());
            return HostMidiOutputReport { event };
        }

        let message = match message_from_intent(intent) {
            Ok(message) => message,
            Err(error) => {
                let event = HostMidiEvent::Failed {
                    error,
                    intent: intent.clone(),
                };
                self.failed_message_count = self.failed_message_count.saturating_add(1);
                self.last_event = Some(event.clone());
                return HostMidiOutputReport { event };
            }
        };

        match self.backend.send(message.clone()) {
            Ok(backend_receipt) if backend_receipt.queued => {
                self.queued_message_count = self.queued_message_count.saturating_add(1);
                let event = HostMidiEvent::Queued {
                    receipt: HostMidiOutputReceipt {
                        intent: intent.clone(),
                        message,
                        backend_receipt,
                    },
                };
                self.last_event = Some(event.clone());
                HostMidiOutputReport { event }
            }
            Ok(backend_receipt) => {
                let event = HostMidiEvent::Ignored {
                    reason: format!(
                        "backend {} did not queue message {}",
                        self.backend.name(),
                        backend_receipt.message_count
                    ),
                    intent: intent.clone(),
                };
                self.ignored_message_count = self.ignored_message_count.saturating_add(1);
                self.last_event = Some(event.clone());
                HostMidiOutputReport { event }
            }
            Err(error) => {
                let event = HostMidiEvent::Failed {
                    error,
                    intent: intent.clone(),
                };
                self.failed_message_count = self.failed_message_count.saturating_add(1);
                self.last_event = Some(event.clone());
                HostMidiOutputReport { event }
            }
        }
    }
}

fn message_from_intent(intent: &MidiOutputIntent) -> Result<MidiMessage, HostMidiError> {
    validate_range(
        "channel",
        intent.channel,
        MIDI_MIN_CHANNEL,
        MIDI_MAX_CHANNEL,
        "must be in range 1..=16",
    )?;
    validate_range(
        "note",
        intent.note,
        0,
        MIDI_MAX_NOTE,
        "must be in range 0..=127",
    )?;
    validate_range(
        "velocity",
        intent.velocity,
        1,
        MIDI_MAX_VELOCITY,
        "must be in range 1..=127",
    )?;

    Ok(MidiMessage {
        channel: intent.channel,
        note: intent.note,
        velocity: intent.velocity,
    })
}

fn validate_range(
    field: &str,
    value: u8,
    min: u8,
    max: u8,
    message: &str,
) -> Result<(), HostMidiError> {
    if (min..=max).contains(&value) {
        Ok(())
    } else {
        Err(HostMidiError::InvalidIntent {
            field: field.to_string(),
            message: message.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mpc_core::PadBank;

    #[test]
    fn disabled_engine_ignores_intent_without_queueing_message() {
        let mut engine = HostMidiEngine::new(CaptureMidiBackend::new(4));
        let intent = intent();

        let report = engine.send_intent(&intent);

        assert!(matches!(
            report.event,
            HostMidiEvent::Ignored { reason, .. } if reason == "host midi disabled"
        ));
        let state = engine.state();
        assert_eq!(state.mode, HostMidiMode::Disabled);
        assert_eq!(state.queued_message_count, 0);
        assert_eq!(state.ignored_message_count, 1);
        assert!(engine.backend().messages().is_empty());
    }

    #[test]
    fn enabled_engine_captures_valid_note_on_message() {
        let mut engine = HostMidiEngine::enabled(CaptureMidiBackend::new(4));
        let intent = intent();

        let report = engine.send_intent(&intent);

        assert!(matches!(
            report.event,
            HostMidiEvent::Queued { receipt }
                if receipt.message.channel == 2
                    && receipt.message.note == 55
                    && receipt.message.velocity == 84
                    && receipt.backend_receipt.queued
                    && receipt.backend_receipt.message_count == 1
        ));
        let state = engine.state();
        assert_eq!(state.mode, HostMidiMode::Capture);
        assert_eq!(state.queued_message_count, 1);
        assert_eq!(
            engine.backend().messages(),
            &[MidiMessage {
                channel: 2,
                note: 55,
                velocity: 84
            }]
        );
    }

    #[test]
    fn invalid_velocity_fails_without_queueing_message() {
        let mut engine = HostMidiEngine::enabled(CaptureMidiBackend::new(4));
        let mut intent = intent();
        intent.velocity = 0;

        let report = engine.send_intent(&intent);

        assert!(matches!(
            report.event,
            HostMidiEvent::Failed {
                error: HostMidiError::InvalidIntent { field, .. },
                ..
            } if field == "velocity"
        ));
        let state = engine.state();
        assert_eq!(state.queued_message_count, 0);
        assert_eq!(state.failed_message_count, 1);
        assert!(engine.backend().messages().is_empty());
    }

    #[test]
    fn capture_backend_retains_bounded_recent_messages() {
        let mut backend = CaptureMidiBackend::new(2);

        backend
            .send(MidiMessage {
                channel: 1,
                note: 36,
                velocity: 80,
            })
            .expect("first send should queue");
        backend
            .send(MidiMessage {
                channel: 1,
                note: 37,
                velocity: 81,
            })
            .expect("second send should queue");
        backend
            .send(MidiMessage {
                channel: 1,
                note: 38,
                velocity: 82,
            })
            .expect("third send should queue");

        assert_eq!(backend.total_sent_count(), 3);
        assert_eq!(
            backend.messages(),
            &[
                MidiMessage {
                    channel: 1,
                    note: 37,
                    velocity: 81
                },
                MidiMessage {
                    channel: 1,
                    note: 38,
                    velocity: 82
                }
            ]
        );
    }

    fn intent() -> MidiOutputIntent {
        MidiOutputIntent {
            selected_track: 2,
            program_index: 1,
            program_name: "Program01".to_string(),
            bank: PadBank::B,
            pad_number: 4,
            source_sample_id: "synthetic_b_04".to_string(),
            source_sample_name: "SYN-B04".to_string(),
            channel: 2,
            note: 55,
            velocity: 84,
        }
    }
}
