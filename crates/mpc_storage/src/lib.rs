use mpc_core::{MpcCore, ProjectSnapshot, ProjectSnapshotError};
use std::fmt;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

pub const PROJECT_FILE_SUFFIX: &str = ".mpc2000xl-project.json";
pub const DEFAULT_PROJECT_FILE_PATH: &str = "local-assets/projects/last.mpc2000xl-project.json";

static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectFileReport {
    pub path: PathBuf,
    pub byte_count: usize,
    pub snapshot_version: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectFileLoad {
    pub snapshot: ProjectSnapshot,
    pub report: ProjectFileReport,
}

#[derive(Debug)]
pub enum ProjectStorageError {
    EmptyPath,
    DirectoryPath {
        path: PathBuf,
    },
    InvalidSuffix {
        path: PathBuf,
        expected_suffix: &'static str,
    },
    Io {
        operation: &'static str,
        path: PathBuf,
        source: std::io::Error,
    },
    ProjectJson {
        path: PathBuf,
        source: ProjectSnapshotError,
    },
}

impl fmt::Display for ProjectStorageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyPath => write!(formatter, "project file path is empty"),
            Self::DirectoryPath { path } => {
                write!(
                    formatter,
                    "project file path is a directory: {}",
                    path.display()
                )
            }
            Self::InvalidSuffix {
                path,
                expected_suffix,
            } => write!(
                formatter,
                "project file path must end with {expected_suffix}: {}",
                path.display()
            ),
            Self::Io {
                operation,
                path,
                source,
            } => write!(
                formatter,
                "{operation} failed for {}: {source}",
                path.display()
            ),
            Self::ProjectJson { path, source } => write!(
                formatter,
                "project JSON validation failed for {}: {source}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for ProjectStorageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::ProjectJson { source, .. } => Some(source),
            Self::EmptyPath | Self::DirectoryPath { .. } | Self::InvalidSuffix { .. } => None,
        }
    }
}

pub fn default_project_file_path() -> PathBuf {
    PathBuf::from(DEFAULT_PROJECT_FILE_PATH)
}

pub fn save_project_file(
    core: &MpcCore,
    path: impl AsRef<Path>,
) -> Result<ProjectFileReport, ProjectStorageError> {
    let path = path.as_ref();
    validate_project_file_path(path)?;

    let snapshot_version = core.export_project_snapshot().version;
    let json = core
        .to_project_json()
        .map_err(|source| project_json_error(path, source))?;
    let byte_count = json.len();

    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .map_err(|source| io_error("create project file parent directories", parent, source))?;
    }

    let temp_path = sibling_temp_path(path);
    let write_result = write_temp_then_rename(&temp_path, path, json.as_bytes());
    if write_result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    write_result?;

    Ok(ProjectFileReport {
        path: canonicalize_project_path(path)?,
        byte_count,
        snapshot_version,
    })
}

pub fn load_project_file(path: impl AsRef<Path>) -> Result<ProjectSnapshot, ProjectStorageError> {
    Ok(load_project_file_with_report(path)?.snapshot)
}

pub fn load_project_file_with_report(
    path: impl AsRef<Path>,
) -> Result<ProjectFileLoad, ProjectStorageError> {
    let path = path.as_ref();
    validate_project_file_path(path)?;

    let json =
        fs::read_to_string(path).map_err(|source| io_error("read project file", path, source))?;
    let byte_count = json.len();
    let snapshot =
        MpcCore::from_project_json(&json).map_err(|source| project_json_error(path, source))?;
    let snapshot_version = snapshot.version;

    Ok(ProjectFileLoad {
        snapshot,
        report: ProjectFileReport {
            path: canonicalize_project_path(path)?,
            byte_count,
            snapshot_version,
        },
    })
}

fn validate_project_file_path(path: &Path) -> Result<(), ProjectStorageError> {
    if path.as_os_str().is_empty() {
        return Err(ProjectStorageError::EmptyPath);
    }
    if path.is_dir() {
        return Err(ProjectStorageError::DirectoryPath {
            path: path.to_path_buf(),
        });
    }

    let Some(file_name) = path.file_name().and_then(|file_name| file_name.to_str()) else {
        return Err(ProjectStorageError::InvalidSuffix {
            path: path.to_path_buf(),
            expected_suffix: PROJECT_FILE_SUFFIX,
        });
    };

    if !file_name.ends_with(PROJECT_FILE_SUFFIX) {
        return Err(ProjectStorageError::InvalidSuffix {
            path: path.to_path_buf(),
            expected_suffix: PROJECT_FILE_SUFFIX,
        });
    }

    Ok(())
}

fn write_temp_then_rename(
    temp_path: &Path,
    final_path: &Path,
    bytes: &[u8],
) -> Result<(), ProjectStorageError> {
    {
        let mut file = File::create(temp_path)
            .map_err(|source| io_error("create temporary project file", temp_path, source))?;
        file.write_all(bytes)
            .map_err(|source| io_error("write temporary project file", temp_path, source))?;
        file.sync_all()
            .map_err(|source| io_error("sync temporary project file", temp_path, source))?;
    }

    fs::rename(temp_path, final_path)
        .map_err(|source| io_error("rename temporary project file", final_path, source))
}

fn sibling_temp_path(path: &Path) -> PathBuf {
    let counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let file_name = path
        .file_name()
        .and_then(|file_name| file_name.to_str())
        .unwrap_or("project");
    path.with_file_name(format!(".{file_name}.tmp-{}-{counter}", std::process::id()))
}

fn canonicalize_project_path(path: &Path) -> Result<PathBuf, ProjectStorageError> {
    fs::canonicalize(path).map_err(|source| io_error("resolve project file path", path, source))
}

fn io_error(operation: &'static str, path: &Path, source: std::io::Error) -> ProjectStorageError {
    ProjectStorageError::Io {
        operation,
        path: path.to_path_buf(),
        source,
    }
}

fn project_json_error(path: &Path, source: ProjectSnapshotError) -> ProjectStorageError {
    ProjectStorageError::ProjectJson {
        path: path.to_path_buf(),
        source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mpc_core::{HardwareEvent, PROJECT_SNAPSHOT_VERSION, PadBank, PanelControl, ProgramPad};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn save_load_project_file_round_trips_after_editing_and_recording() {
        let root = temp_root("round_trip");
        let path = root.join("nested").join("session.mpc2000xl-project.json");
        let core = edited_recorded_core();

        let report = save_project_file(&core, &path).expect("project file should save");
        assert_eq!(
            report.path,
            fs::canonicalize(&path).expect("path should exist")
        );
        assert_eq!(report.snapshot_version, PROJECT_SNAPSHOT_VERSION);
        assert!(report.byte_count > 0);

        let loaded = load_project_file_with_report(&path).expect("project file should load");
        assert_eq!(loaded.report.byte_count, report.byte_count);
        assert_eq!(loaded.report.snapshot_version, PROJECT_SNAPSHOT_VERSION);
        assert_eq!(loaded.snapshot, core.export_project_snapshot());

        let mut restored = MpcCore::new();
        restored
            .restore_project_snapshot(loaded.snapshot)
            .expect("loaded snapshot should restore");
        assert_eq!(restored.state().mode, mpc_core::Mode::Program);
        assert_eq!(restored.state().sequence_index, 3);
        assert_eq!(restored.state().selected_track, 5);
        assert_eq!(restored.state().bar_count, 4);
        assert_eq!(
            restored.state().selected_program_pad,
            ProgramPad {
                bank: PadBank::A,
                pad_number: 5,
            }
        );
        assert_eq!(restored.state().recorded_events.len(), 1);
        assert!(!restored.state().playing);
        assert!(!restored.state().recording);

        remove_temp_root(root);
    }

    #[test]
    fn wrong_suffix_is_rejected_before_writing() {
        let root = temp_root("wrong_suffix");
        let path = root.join("nested").join("session.json");

        let error =
            save_project_file(&MpcCore::new(), &path).expect_err("wrong suffix should be rejected");

        assert!(matches!(
            error,
            ProjectStorageError::InvalidSuffix {
                expected_suffix: PROJECT_FILE_SUFFIX,
                ..
            }
        ));
        assert!(
            !root.exists(),
            "wrong suffix must be rejected before creating parent directories"
        );
    }

    #[test]
    fn loading_malformed_json_returns_structured_error() {
        let root = temp_root("malformed_json");
        let path = root.join("bad.mpc2000xl-project.json");
        fs::create_dir_all(&root).expect("temp root should be created");
        fs::write(&path, "{not valid json").expect("malformed fixture should write");

        let error = load_project_file(&path).expect_err("malformed JSON should be rejected");

        assert!(matches!(
            error,
            ProjectStorageError::ProjectJson {
                source: ProjectSnapshotError::JsonDecode { .. },
                ..
            }
        ));

        remove_temp_root(root);
    }

    #[test]
    fn loading_invalid_project_json_returns_structured_error() {
        let root = temp_root("invalid_json");
        let path = root.join("invalid.mpc2000xl-project.json");
        fs::create_dir_all(&root).expect("temp root should be created");
        let mut value: serde_json::Value = serde_json::from_str(
            &MpcCore::new()
                .to_project_json()
                .expect("default snapshot should encode"),
        )
        .expect("default snapshot should parse");
        value["rights_boundary"] = serde_json::json!("contains_audio_bytes");
        fs::write(&path, serde_json::to_string_pretty(&value).unwrap())
            .expect("invalid project fixture should write");

        let error = load_project_file(&path).expect_err("invalid project JSON should be rejected");

        assert!(matches!(
            error,
            ProjectStorageError::ProjectJson {
                source: ProjectSnapshotError::InvalidRightsBoundary { .. },
                ..
            }
        ));

        remove_temp_root(root);
    }

    #[test]
    fn parent_directories_are_created_on_save() {
        let root = temp_root("parents");
        let path = root
            .join("local-assets")
            .join("projects")
            .join("last.mpc2000xl-project.json");
        assert!(!path.parent().expect("path should have parent").exists());

        save_project_file(&MpcCore::new(), &path).expect("save should create parents");

        assert!(path.exists());
        assert!(path.parent().expect("path should have parent").is_dir());

        remove_temp_root(root);
    }

    #[test]
    fn saved_json_is_metadata_only() {
        let root = temp_root("metadata_only");
        let path = root.join("metadata.mpc2000xl-project.json");

        save_project_file(&edited_recorded_core(), &path).expect("project file should save");
        let json = fs::read_to_string(&path).expect("saved file should be readable");

        assert!(json.contains("\"rights_boundary\": \"metadata_only_no_audio_bytes\""));
        assert!(!json.contains("\"audio_bytes\""));
        assert!(!json.contains("\"sample_file_contents\""));

        remove_temp_root(root);
    }

    #[test]
    fn directory_paths_are_rejected_for_load_and_save() {
        let root = temp_root("directory_path");
        let path = root.join("as-dir.mpc2000xl-project.json");
        fs::create_dir_all(&path).expect("directory fixture should be created");

        let save_error =
            save_project_file(&MpcCore::new(), &path).expect_err("directory save should fail");
        let load_error = load_project_file(&path).expect_err("directory load should fail");

        assert!(matches!(
            save_error,
            ProjectStorageError::DirectoryPath { .. }
        ));
        assert!(matches!(
            load_error,
            ProjectStorageError::DirectoryPath { .. }
        ));

        remove_temp_root(root);
    }

    #[test]
    fn empty_paths_are_rejected_for_load_and_save() {
        let save_error = save_project_file(&MpcCore::new(), Path::new(""))
            .expect_err("empty save path should fail");
        let load_error = load_project_file(Path::new("")).expect_err("empty load path should fail");

        assert!(matches!(save_error, ProjectStorageError::EmptyPath));
        assert!(matches!(load_error, ProjectStorageError::EmptyPath));
    }

    fn edited_recorded_core() -> MpcCore {
        let mut core = MpcCore::new();

        core.dispatch(HardwareEvent::TurnDataWheel { delta: 5 });
        core.dispatch(HardwareEvent::Press {
            control: PanelControl::CursorLeft,
        });
        core.dispatch(HardwareEvent::TurnDataWheel { delta: 4 });
        core.dispatch(HardwareEvent::Press {
            control: PanelControl::CursorLeft,
        });
        core.dispatch(HardwareEvent::TurnDataWheel { delta: 2 });
        core.dispatch(HardwareEvent::Press {
            control: PanelControl::CursorLeft,
        });
        core.dispatch(HardwareEvent::TurnDataWheel { delta: 3 });
        core.dispatch(HardwareEvent::Press {
            control: PanelControl::Program,
        });
        core.dispatch(HardwareEvent::StrikePad {
            bank: PadBank::A,
            pad: 5,
            velocity: 90,
        });
        core.dispatch(HardwareEvent::Press {
            control: PanelControl::Rec,
        });
        core.dispatch(HardwareEvent::Press {
            control: PanelControl::Play,
        });
        core.dispatch(HardwareEvent::Tick { micros: 500_000 });
        core.dispatch(HardwareEvent::StrikePad {
            bank: PadBank::A,
            pad: 5,
            velocity: 87,
        });

        core
    }

    fn temp_root(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        let counter = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "mpc_storage_{label}_{}_{}_{counter}",
            std::process::id(),
            nanos
        ))
    }

    fn remove_temp_root(path: PathBuf) {
        let _ = fs::remove_dir_all(path);
    }
}
