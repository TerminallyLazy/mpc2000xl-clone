use anyhow::{Context, Result, bail};
use mpc_core::{HardwareEvent, Mode, MpcCore};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fixture {
    pub id: String,
    pub name: String,
    pub source_refs: Vec<String>,
    pub events: Vec<HardwareEvent>,
    pub expect: ExpectedState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpectedState {
    pub mode: Mode,
    pub lcd_title: String,
    pub playing: bool,
    pub recording: bool,
    pub event_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixtureReport {
    pub id: String,
    pub name: String,
    pub passed: bool,
    pub details: Vec<String>,
}

pub fn load_fixture(path: impl AsRef<Path>) -> Result<Fixture> {
    let path = path.as_ref();
    let json = fs::read_to_string(path)
        .with_context(|| format!("failed to read fixture {}", path.display()))?;
    serde_json::from_str(&json)
        .with_context(|| format!("failed to parse fixture {}", path.display()))
}

pub fn run_fixture(fixture: &Fixture) -> FixtureReport {
    let mut core = MpcCore::new();

    for event in &fixture.events {
        core.dispatch(event.clone());
    }

    let state = core.state();
    let mut details = Vec::new();

    if state.mode != fixture.expect.mode {
        details.push(format!(
            "mode mismatch: expected {:?}, got {:?}",
            fixture.expect.mode, state.mode
        ));
    }

    if state.lcd.title != fixture.expect.lcd_title {
        details.push(format!(
            "lcd title mismatch: expected {}, got {}",
            fixture.expect.lcd_title, state.lcd.title
        ));
    }

    if state.playing != fixture.expect.playing {
        details.push(format!(
            "playing mismatch: expected {}, got {}",
            fixture.expect.playing, state.playing
        ));
    }

    if state.recording != fixture.expect.recording {
        details.push(format!(
            "recording mismatch: expected {}, got {}",
            fixture.expect.recording, state.recording
        ));
    }

    if state.event_count != fixture.expect.event_count {
        details.push(format!(
            "event_count mismatch: expected {}, got {}",
            fixture.expect.event_count, state.event_count
        ));
    }

    FixtureReport {
        id: fixture.id.clone(),
        name: fixture.name.clone(),
        passed: details.is_empty(),
        details,
    }
}

pub fn run_fixture_path(path: impl AsRef<Path>) -> Result<FixtureReport> {
    let fixture = load_fixture(path)?;
    if fixture.source_refs.is_empty() {
        bail!("fixture {} has no source references", fixture.id);
    }
    Ok(run_fixture(&fixture))
}
