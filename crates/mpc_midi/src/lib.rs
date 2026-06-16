use midir::{
    Ignore, MidiInput as MidirInput, MidiInputConnection, MidiOutput as MidirOutput,
    MidiOutputConnection,
};
use mpc_core::{MidiOutputIntent, MidiOutputIntentKind};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fmt;
use std::sync::{Arc, Mutex};

const MIDI_MIN_CHANNEL: u8 = 1;
const MIDI_MAX_CHANNEL: u8 = 16;
const MIDI_MAX_NOTE: u8 = 127;
const MIDI_MAX_VELOCITY: u8 = 127;
pub const DEFAULT_DEVICE_MIDI_INPUT_QUEUE_EVENTS: usize = 256;
pub const MAX_DEVICE_MIDI_INPUT_QUEUE_EVENTS: usize = 4_096;
pub const MAX_DEVICE_MIDI_RECENT_IGNORED_MESSAGES: usize = 8;
const DEVICE_MIDI_BACKEND_NAME: &str = "device";
const DEVICE_MIDI_CLIENT_NAME: &str = "mpc2000xl-clone";
const DEVICE_MIDI_OUTPUT_CONNECTION_NAME: &str = "mpc2000xl-clone-output";
const DEVICE_MIDI_INPUT_CONNECTION_NAME: &str = "mpc2000xl-clone-input";
const MIDI_STATUS_KIND_MASK: u8 = 0xF0;
const MIDI_STATUS_CHANNEL_MASK: u8 = 0x0F;
const MIDI_NOTE_OFF_STATUS: u8 = 0x80;
const MIDI_NOTE_ON_STATUS: u8 = 0x90;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostMidiMode {
    Disabled,
    Capture,
    Device,
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MidiMessageKind {
    #[default]
    NoteOn,
    NoteOff,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MidiMessage {
    #[serde(default)]
    pub kind: MidiMessageKind,
    pub channel: u8,
    pub note: u8,
    pub velocity: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MidiPortDescriptor {
    pub index: usize,
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MidiInputEvent {
    NoteOn { channel: u8, note: u8, velocity: u8 },
    NoteOff { channel: u8, note: u8, velocity: u8 },
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
pub struct PendingOutboundMidiNote {
    pub intent: MidiOutputIntent,
    pub note_off_due_millis: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutboundMidiNoteScheduler {
    pending_notes: Vec<PendingOutboundMidiNote>,
}

impl OutboundMidiNoteScheduler {
    pub fn pending_count(&self) -> usize {
        self.pending_notes.len()
    }

    pub fn has_pending(&self) -> bool {
        !self.pending_notes.is_empty()
    }

    pub fn register_note_on(
        &mut self,
        intent: &MidiOutputIntent,
        now_millis: u64,
        duration_millis: u64,
    ) {
        if intent.kind != MidiOutputIntentKind::NoteOn {
            return;
        }
        let due = now_millis.saturating_add(duration_millis);
        self.pending_notes
            .retain(|pending| !same_outbound_note(&pending.intent, intent));
        self.pending_notes.push(PendingOutboundMidiNote {
            intent: intent.clone(),
            note_off_due_millis: due,
        });
    }

    pub fn drain_due_note_offs(&mut self, now_millis: u64) -> Vec<MidiOutputIntent> {
        let mut due = Vec::new();
        let mut pending = Vec::with_capacity(self.pending_notes.len());
        for note in self.pending_notes.drain(..) {
            if note.note_off_due_millis <= now_millis {
                due.push(note_off_intent(&note.intent));
            } else {
                pending.push(note);
            }
        }
        self.pending_notes = pending;
        due
    }

    pub fn release_matching_note(
        &mut self,
        intent: &MidiOutputIntent,
        _now_millis: u64,
    ) -> Vec<MidiOutputIntent> {
        let mut released = Vec::new();
        let mut pending = Vec::with_capacity(self.pending_notes.len());
        for note in self.pending_notes.drain(..) {
            if same_outbound_note(&note.intent, intent) {
                released.push(note_off_intent(&note.intent));
            } else {
                pending.push(note);
            }
        }
        self.pending_notes = pending;
        released
    }

    pub fn clear(&mut self) {
        self.pending_notes.clear();
    }
}

fn same_outbound_note(left: &MidiOutputIntent, right: &MidiOutputIntent) -> bool {
    left.channel == right.channel
        && left.note == right.note
        && left.bank == right.bank
        && left.pad_number == right.pad_number
        && left.source_sample_id == right.source_sample_id
        && left.selected_track == right.selected_track
}

fn note_off_intent(intent: &MidiOutputIntent) -> MidiOutputIntent {
    let mut note_off = intent.clone();
    note_off.kind = MidiOutputIntentKind::NoteOff;
    note_off.velocity = 0;
    note_off
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
    fn mode(&self) -> HostMidiMode;
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

    fn mode(&self) -> HostMidiMode {
        HostMidiMode::Capture
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceMidiOutputStatus {
    pub backend_name: String,
    pub output_port: MidiPortDescriptor,
    pub total_sent_message_count: u64,
    pub last_sent_message: Option<MidiMessage>,
    pub last_sent_bytes: Option<[u8; 3]>,
}

pub struct DeviceMidiOutputBackend {
    output_port: MidiPortDescriptor,
    connection: MidiOutputConnection,
    total_sent_message_count: u64,
    last_sent_message: Option<MidiMessage>,
    last_sent_bytes: Option<[u8; 3]>,
}

impl DeviceMidiOutputBackend {
    pub fn connect_output_port_id(port_id: &str) -> Result<Self, HostMidiError> {
        let output = MidirOutput::new(DEVICE_MIDI_CLIENT_NAME)
            .map_err(|error| device_midi_error(format!("output init failed: {error}")))?;
        let Some((index, port)) = output
            .ports()
            .into_iter()
            .enumerate()
            .find(|(_, port)| port.id() == port_id)
        else {
            return Err(device_midi_error(format!(
                "output port id {port_id:?} is not available"
            )));
        };
        let name = output
            .port_name(&port)
            .unwrap_or_else(|error| format!("unknown MIDI output ({error})"));
        let descriptor = MidiPortDescriptor {
            index,
            id: port.id(),
            name,
        };
        let connection = output
            .connect(&port, DEVICE_MIDI_OUTPUT_CONNECTION_NAME)
            .map_err(|error| device_midi_error(format!("output connect failed: {error}")))?;

        Ok(Self {
            output_port: descriptor,
            connection,
            total_sent_message_count: 0,
            last_sent_message: None,
            last_sent_bytes: None,
        })
    }

    pub fn status(&self) -> DeviceMidiOutputStatus {
        DeviceMidiOutputStatus {
            backend_name: DEVICE_MIDI_BACKEND_NAME.to_string(),
            output_port: self.output_port.clone(),
            total_sent_message_count: self.total_sent_message_count,
            last_sent_message: self.last_sent_message.clone(),
            last_sent_bytes: self.last_sent_bytes,
        }
    }
}

impl HostMidiBackend for DeviceMidiOutputBackend {
    fn name(&self) -> &'static str {
        DEVICE_MIDI_BACKEND_NAME
    }

    fn mode(&self) -> HostMidiMode {
        HostMidiMode::Device
    }

    fn send(&mut self, message: MidiMessage) -> Result<HostMidiBackendReceipt, HostMidiError> {
        let bytes = encode_midi_message(&message)?;
        self.connection
            .send(&bytes)
            .map_err(|error| device_midi_error(format!("output send failed: {error}")))?;
        self.total_sent_message_count = self.total_sent_message_count.saturating_add(1);
        self.last_sent_message = Some(message);
        self.last_sent_bytes = Some(bytes);
        Ok(HostMidiBackendReceipt::queued(
            self.total_sent_message_count,
        ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceMidiInputConfig {
    pub max_queued_events: usize,
}

impl Default for DeviceMidiInputConfig {
    fn default() -> Self {
        Self {
            max_queued_events: DEFAULT_DEVICE_MIDI_INPUT_QUEUE_EVENTS,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceMidiInputStatus {
    pub input_port: MidiPortDescriptor,
    pub queued_event_count: usize,
    pub max_queued_event_count: usize,
    pub total_received_message_count: u64,
    pub total_decoded_event_count: u64,
    pub total_ignored_message_count: u64,
    pub dropped_event_count: u64,
    pub recent_ignored_messages: Vec<String>,
}

pub struct DeviceMidiInputConnection {
    input_port: MidiPortDescriptor,
    shared: Arc<Mutex<DeviceMidiInputQueue>>,
    _connection: MidiInputConnection<()>,
}

impl DeviceMidiInputConnection {
    pub fn connect_input_port_id(
        port_id: &str,
        config: DeviceMidiInputConfig,
    ) -> Result<Self, HostMidiError> {
        let mut input = MidirInput::new(DEVICE_MIDI_CLIENT_NAME)
            .map_err(|error| device_midi_error(format!("input init failed: {error}")))?;
        input.ignore(Ignore::None);
        let Some((index, port)) = input
            .ports()
            .into_iter()
            .enumerate()
            .find(|(_, port)| port.id() == port_id)
        else {
            return Err(device_midi_error(format!(
                "input port id {port_id:?} is not available"
            )));
        };
        let name = input
            .port_name(&port)
            .unwrap_or_else(|error| format!("unknown MIDI input ({error})"));
        let descriptor = MidiPortDescriptor {
            index,
            id: port.id(),
            name,
        };
        let shared = Arc::new(Mutex::new(DeviceMidiInputQueue::new(
            config.max_queued_events,
        )));
        let callback_shared = Arc::clone(&shared);
        let connection = input
            .connect(
                &port,
                DEVICE_MIDI_INPUT_CONNECTION_NAME,
                move |_timestamp, bytes, _| {
                    if let Ok(mut queue) = callback_shared.lock() {
                        queue.push_raw_message(bytes);
                    }
                },
                (),
            )
            .map_err(|error| device_midi_error(format!("input connect failed: {error}")))?;

        Ok(Self {
            input_port: descriptor,
            shared,
            _connection: connection,
        })
    }

    pub fn drain_events(&mut self) -> Result<Vec<MidiInputEvent>, HostMidiError> {
        let mut queue = self
            .shared
            .lock()
            .map_err(|_| device_midi_error("input event queue lock poisoned"))?;
        Ok(queue.drain_events())
    }

    pub fn status(&self) -> DeviceMidiInputStatus {
        match self.shared.lock() {
            Ok(queue) => queue.status(self.input_port.clone()),
            Err(_) => DeviceMidiInputStatus {
                input_port: self.input_port.clone(),
                queued_event_count: 0,
                max_queued_event_count: 0,
                total_received_message_count: 0,
                total_decoded_event_count: 0,
                total_ignored_message_count: 0,
                dropped_event_count: 0,
                recent_ignored_messages: vec!["input event queue lock poisoned".to_string()],
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DeviceMidiInputQueue {
    max_queued_events: usize,
    events: VecDeque<MidiInputEvent>,
    total_received_message_count: u64,
    total_decoded_event_count: u64,
    total_ignored_message_count: u64,
    dropped_event_count: u64,
    recent_ignored_messages: VecDeque<String>,
}

impl DeviceMidiInputQueue {
    fn new(max_queued_events: usize) -> Self {
        let max_queued_events = max_queued_events
            .max(1)
            .min(MAX_DEVICE_MIDI_INPUT_QUEUE_EVENTS);
        Self {
            max_queued_events,
            events: VecDeque::with_capacity(
                max_queued_events.min(DEFAULT_DEVICE_MIDI_INPUT_QUEUE_EVENTS),
            ),
            total_received_message_count: 0,
            total_decoded_event_count: 0,
            total_ignored_message_count: 0,
            dropped_event_count: 0,
            recent_ignored_messages: VecDeque::with_capacity(
                MAX_DEVICE_MIDI_RECENT_IGNORED_MESSAGES,
            ),
        }
    }

    fn push_raw_message(&mut self, bytes: &[u8]) {
        self.total_received_message_count = self.total_received_message_count.saturating_add(1);
        match decode_midi_input_event(bytes) {
            Ok(Some(event)) if self.events.len() < self.max_queued_events => {
                self.total_decoded_event_count = self.total_decoded_event_count.saturating_add(1);
                self.events.push_back(event);
            }
            Ok(Some(_)) => {
                self.dropped_event_count = self.dropped_event_count.saturating_add(1);
                self.record_ignored_message("input event queue full");
            }
            Ok(None) => {
                self.record_ignored_message(format!(
                    "unsupported MIDI message {}",
                    midi_bytes_text(bytes)
                ));
            }
            Err(message) => {
                self.record_ignored_message(message);
            }
        }
    }

    fn drain_events(&mut self) -> Vec<MidiInputEvent> {
        self.events.drain(..).collect()
    }

    fn record_ignored_message(&mut self, message: impl Into<String>) {
        self.total_ignored_message_count = self.total_ignored_message_count.saturating_add(1);
        if self.recent_ignored_messages.len() == MAX_DEVICE_MIDI_RECENT_IGNORED_MESSAGES {
            self.recent_ignored_messages.pop_front();
        }
        self.recent_ignored_messages.push_back(message.into());
    }

    fn status(&self, input_port: MidiPortDescriptor) -> DeviceMidiInputStatus {
        DeviceMidiInputStatus {
            input_port,
            queued_event_count: self.events.len(),
            max_queued_event_count: self.max_queued_events,
            total_received_message_count: self.total_received_message_count,
            total_decoded_event_count: self.total_decoded_event_count,
            total_ignored_message_count: self.total_ignored_message_count,
            dropped_event_count: self.dropped_event_count,
            recent_ignored_messages: self.recent_ignored_messages.iter().cloned().collect(),
        }
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
                self.backend.mode()
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
    let kind = match intent.kind {
        MidiOutputIntentKind::NoteOn => MidiMessageKind::NoteOn,
        MidiOutputIntentKind::NoteOff => MidiMessageKind::NoteOff,
    };
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
    let min_velocity = match kind {
        MidiMessageKind::NoteOn => 1,
        MidiMessageKind::NoteOff => 0,
    };
    validate_range(
        "velocity",
        intent.velocity,
        min_velocity,
        MIDI_MAX_VELOCITY,
        if kind == MidiMessageKind::NoteOn {
            "must be in range 1..=127"
        } else {
            "must be in range 0..=127"
        },
    )?;

    Ok(MidiMessage {
        kind,
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

pub fn list_device_midi_input_ports() -> Result<Vec<MidiPortDescriptor>, HostMidiError> {
    let input = MidirInput::new(DEVICE_MIDI_CLIENT_NAME)
        .map_err(|error| device_midi_error(format!("input init failed: {error}")))?;
    input
        .ports()
        .into_iter()
        .enumerate()
        .map(|(index, port)| {
            let name = input
                .port_name(&port)
                .map_err(|error| device_midi_error(format!("input port name failed: {error}")))?;
            Ok(MidiPortDescriptor {
                index,
                id: port.id(),
                name,
            })
        })
        .collect()
}

pub fn list_device_midi_output_ports() -> Result<Vec<MidiPortDescriptor>, HostMidiError> {
    let output = MidirOutput::new(DEVICE_MIDI_CLIENT_NAME)
        .map_err(|error| device_midi_error(format!("output init failed: {error}")))?;
    output
        .ports()
        .into_iter()
        .enumerate()
        .map(|(index, port)| {
            let name = output
                .port_name(&port)
                .map_err(|error| device_midi_error(format!("output port name failed: {error}")))?;
            Ok(MidiPortDescriptor {
                index,
                id: port.id(),
                name,
            })
        })
        .collect()
}

pub fn encode_midi_message(message: &MidiMessage) -> Result<[u8; 3], HostMidiError> {
    validate_range(
        "channel",
        message.channel,
        MIDI_MIN_CHANNEL,
        MIDI_MAX_CHANNEL,
        "must be in range 1..=16",
    )?;
    validate_range(
        "note",
        message.note,
        0,
        MIDI_MAX_NOTE,
        "must be in range 0..=127",
    )?;
    let min_velocity = match message.kind {
        MidiMessageKind::NoteOn => 1,
        MidiMessageKind::NoteOff => 0,
    };
    validate_range(
        "velocity",
        message.velocity,
        min_velocity,
        MIDI_MAX_VELOCITY,
        if message.kind == MidiMessageKind::NoteOn {
            "must be in range 1..=127"
        } else {
            "must be in range 0..=127"
        },
    )?;

    let status = match message.kind {
        MidiMessageKind::NoteOn => MIDI_NOTE_ON_STATUS,
        MidiMessageKind::NoteOff => MIDI_NOTE_OFF_STATUS,
    };
    Ok([
        status | (message.channel - 1),
        message.note,
        message.velocity,
    ])
}

pub fn encode_note_on_message(message: &MidiMessage) -> Result<[u8; 3], HostMidiError> {
    let mut message = message.clone();
    message.kind = MidiMessageKind::NoteOn;
    encode_midi_message(&message)
}

pub fn decode_midi_input_event(bytes: &[u8]) -> Result<Option<MidiInputEvent>, String> {
    let Some(status) = bytes.first().copied() else {
        return Err("empty MIDI message".to_string());
    };
    let status_kind = status & MIDI_STATUS_KIND_MASK;
    if status_kind != MIDI_NOTE_ON_STATUS && status_kind != MIDI_NOTE_OFF_STATUS {
        return Ok(None);
    }
    if bytes.len() < 3 {
        return Err(format!(
            "short MIDI note message {}",
            midi_bytes_text(bytes)
        ));
    }

    let channel = (status & MIDI_STATUS_CHANNEL_MASK) + 1;
    let note = bytes[1];
    let velocity = bytes[2];
    if note > MIDI_MAX_NOTE || velocity > MIDI_MAX_VELOCITY {
        return Err(format!(
            "out-of-range MIDI note message {}",
            midi_bytes_text(bytes)
        ));
    }

    if status_kind == MIDI_NOTE_OFF_STATUS || velocity == 0 {
        Ok(Some(MidiInputEvent::NoteOff {
            channel,
            note,
            velocity,
        }))
    } else {
        Ok(Some(MidiInputEvent::NoteOn {
            channel,
            note,
            velocity,
        }))
    }
}

fn midi_bytes_text(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return "[]".to_string();
    }
    let bytes = bytes
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(" ");
    format!("[{bytes}]")
}

fn device_midi_error(message: impl Into<String>) -> HostMidiError {
    HostMidiError::Backend {
        message: message.into(),
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
                kind: MidiMessageKind::NoteOn,
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
                kind: MidiMessageKind::NoteOn,
                channel: 1,
                note: 36,
                velocity: 80,
            })
            .expect("first send should queue");
        backend
            .send(MidiMessage {
                kind: MidiMessageKind::NoteOn,
                channel: 1,
                note: 37,
                velocity: 81,
            })
            .expect("second send should queue");
        backend
            .send(MidiMessage {
                kind: MidiMessageKind::NoteOn,
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
                    kind: MidiMessageKind::NoteOn,
                    channel: 1,
                    note: 37,
                    velocity: 81
                },
                MidiMessage {
                    kind: MidiMessageKind::NoteOn,
                    channel: 1,
                    note: 38,
                    velocity: 82
                }
            ]
        );
    }

    #[test]
    fn note_on_message_encodes_to_midi_bytes() {
        let bytes = encode_note_on_message(&MidiMessage {
            kind: MidiMessageKind::NoteOn,
            channel: 2,
            note: 55,
            velocity: 84,
        })
        .expect("valid message should encode");

        assert_eq!(bytes, [0x91, 55, 84]);
    }

    #[test]
    fn note_off_message_encodes_to_midi_bytes() {
        let bytes = encode_midi_message(&MidiMessage {
            kind: MidiMessageKind::NoteOff,
            channel: 2,
            note: 55,
            velocity: 0,
        })
        .expect("valid note-off should encode");

        assert_eq!(bytes, [0x81, 55, 0]);
    }

    #[test]
    fn outbound_note_scheduler_expires_note_on_into_note_off_intent() {
        let mut scheduler = OutboundMidiNoteScheduler::default();
        let note_on = intent();

        scheduler.register_note_on(&note_on, 100, 250);
        assert_eq!(scheduler.pending_count(), 1);
        assert!(scheduler.drain_due_note_offs(349).is_empty());

        let due = scheduler.drain_due_note_offs(350);
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].kind, mpc_core::MidiOutputIntentKind::NoteOff);
        assert_eq!(due[0].channel, note_on.channel);
        assert_eq!(due[0].note, note_on.note);
        assert_eq!(due[0].velocity, 0);
        assert_eq!(scheduler.pending_count(), 0);
    }

    #[test]
    fn outbound_note_scheduler_releases_matching_note_early() {
        let mut scheduler = OutboundMidiNoteScheduler::default();
        let note_on = intent();

        scheduler.register_note_on(&note_on, 100, 250);
        let released = scheduler.release_matching_note(&note_on, 150);

        assert_eq!(released.len(), 1);
        assert_eq!(released[0].kind, mpc_core::MidiOutputIntentKind::NoteOff);
        assert_eq!(released[0].note, note_on.note);
        assert_eq!(scheduler.pending_count(), 0);
        assert!(scheduler.drain_due_note_offs(400).is_empty());
    }

    #[test]
    fn note_on_message_rejects_invalid_channel_without_bytes() {
        let error = encode_note_on_message(&MidiMessage {
            kind: MidiMessageKind::NoteOn,
            channel: 17,
            note: 55,
            velocity: 84,
        })
        .expect_err("invalid channel should fail");

        assert!(matches!(
            error,
            HostMidiError::InvalidIntent { field, .. } if field == "channel"
        ));
    }

    #[test]
    fn midi_input_decoder_accepts_note_on_and_note_off() {
        assert_eq!(
            decode_midi_input_event(&[0x90, 36, 100]).expect("note on should decode"),
            Some(MidiInputEvent::NoteOn {
                channel: 1,
                note: 36,
                velocity: 100
            })
        );
        assert_eq!(
            decode_midi_input_event(&[0x8F, 48, 64]).expect("note off should decode"),
            Some(MidiInputEvent::NoteOff {
                channel: 16,
                note: 48,
                velocity: 64
            })
        );
        assert_eq!(
            decode_midi_input_event(&[0x92, 37, 0]).expect("zero velocity note on should decode"),
            Some(MidiInputEvent::NoteOff {
                channel: 3,
                note: 37,
                velocity: 0
            })
        );
    }

    #[test]
    fn midi_input_decoder_ignores_unsupported_messages_and_rejects_short_notes() {
        assert_eq!(
            decode_midi_input_event(&[0xB0, 1, 64]).expect("cc should be ignored"),
            None
        );
        assert!(decode_midi_input_event(&[0x90, 36]).is_err());
        assert!(decode_midi_input_event(&[]).is_err());
    }

    #[test]
    fn device_midi_input_queue_clamps_drains_and_counts_ignored_messages() {
        let mut queue = DeviceMidiInputQueue::new(usize::MAX);
        assert_eq!(
            queue.status(port()).max_queued_event_count,
            MAX_DEVICE_MIDI_INPUT_QUEUE_EVENTS
        );

        queue.push_raw_message(&[0x90, 36, 100]);
        queue.push_raw_message(&[0x80, 36, 64]);
        queue.push_raw_message(&[0xB0, 1, 64]);
        queue.push_raw_message(&[0x90, 36]);

        let status = queue.status(port());
        assert_eq!(status.queued_event_count, 2);
        assert_eq!(status.total_received_message_count, 4);
        assert_eq!(status.total_decoded_event_count, 2);
        assert_eq!(status.total_ignored_message_count, 2);
        assert_eq!(status.dropped_event_count, 0);
        assert_eq!(
            queue.drain_events(),
            vec![
                MidiInputEvent::NoteOn {
                    channel: 1,
                    note: 36,
                    velocity: 100
                },
                MidiInputEvent::NoteOff {
                    channel: 1,
                    note: 36,
                    velocity: 64
                }
            ]
        );
        assert!(queue.drain_events().is_empty());
    }

    #[test]
    fn device_midi_input_queue_drops_when_full_without_partial_growth() {
        let mut queue = DeviceMidiInputQueue::new(1);

        queue.push_raw_message(&[0x90, 36, 100]);
        queue.push_raw_message(&[0x90, 37, 100]);

        let status = queue.status(port());
        assert_eq!(status.queued_event_count, 1);
        assert_eq!(status.total_decoded_event_count, 1);
        assert_eq!(status.total_ignored_message_count, 1);
        assert_eq!(status.dropped_event_count, 1);
        assert_eq!(
            status.recent_ignored_messages,
            vec!["input event queue full"]
        );
    }

    fn intent() -> MidiOutputIntent {
        MidiOutputIntent {
            kind: mpc_core::MidiOutputIntentKind::NoteOn,
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
            window_length_frames: 48_000,
        }
    }

    fn port() -> MidiPortDescriptor {
        MidiPortDescriptor {
            index: 0,
            id: "test-port".to_string(),
            name: "Test Port".to_string(),
        }
    }
}
