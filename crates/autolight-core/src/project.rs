use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use thiserror::Error;

use crate::graph::{validate_graph, GraphError};

pub const SCHEMA_VERSION: u32 = 1;

pub type JsonObject = Map<String, Value>;

#[derive(Debug, Error)]
pub enum ProjectError {
    #[error("failed to read project file {path}: {source}")]
    Read { path: PathBuf, source: io::Error },
    #[error("failed to write project file {path}: {source}")]
    Write { path: PathBuf, source: io::Error },
    #[error("failed to create project directory {path}: {source}")]
    CreateDirectory { path: PathBuf, source: io::Error },
    #[error("failed to parse project json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported schema version: {0}")]
    UnsupportedSchemaVersion(u32),
    #[error("invalid project graph: {0}")]
    InvalidGraph(#[from] GraphError),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectDocument {
    pub id: String,
    pub name: String,
    pub schema_version: u32,
    #[serde(default)]
    pub audio_assets: Vec<AudioAsset>,
    #[serde(default)]
    pub tracks: Vec<Track>,
    #[serde(default)]
    pub markers: Vec<Marker>,
    #[serde(default)]
    pub job_runs: Vec<JobRun>,
    #[serde(default)]
    pub cache_entries: Vec<CacheEntry>,
    #[serde(default)]
    pub ui_state: JsonObject,
}

impl ProjectDocument {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            schema_version: SCHEMA_VERSION,
            audio_assets: Vec::new(),
            tracks: Vec::new(),
            markers: Vec::new(),
            job_runs: Vec::new(),
            cache_entries: Vec::new(),
            ui_state: JsonObject::new(),
        }
    }

    pub fn from_json_str(input: &str) -> Result<Self, ProjectError> {
        let project: Self = serde_json::from_str(input)?;
        project.ensure_supported_schema()?;
        validate_graph(&project)?;
        Ok(project)
    }

    pub fn load_path(path: impl AsRef<Path>) -> Result<Self, ProjectError> {
        let path = path.as_ref();
        let input = fs::read_to_string(path).map_err(|source| ProjectError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        Self::from_json_str(&input)
    }

    pub fn to_json_string_pretty(&self) -> Result<String, ProjectError> {
        self.ensure_supported_schema()?;
        validate_graph(self)?;
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn save_path(&self, path: impl AsRef<Path>) -> Result<(), ProjectError> {
        let path = path.as_ref();
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).map_err(|source| ProjectError::CreateDirectory {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let output = self.to_json_string_pretty()?;
        let tmp_path = atomic_save_temp_path(path);
        let write_result = (|| -> Result<(), ProjectError> {
            {
                let mut file =
                    fs::File::create(&tmp_path).map_err(|source| ProjectError::Write {
                        path: tmp_path.clone(),
                        source,
                    })?;
                file.write_all(output.as_bytes())
                    .map_err(|source| ProjectError::Write {
                        path: tmp_path.clone(),
                        source,
                    })?;
                file.sync_all().map_err(|source| ProjectError::Write {
                    path: tmp_path.clone(),
                    source,
                })?;
            }
            replace_project_file(&tmp_path, path)?;
            sync_parent_directory(path)?;
            Ok(())
        })();
        if write_result.is_err() {
            let _ = fs::remove_file(&tmp_path);
        }
        write_result
    }

    fn ensure_supported_schema(&self) -> Result<(), ProjectError> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(ProjectError::UnsupportedSchemaVersion(self.schema_version));
        }
        Ok(())
    }
}

fn atomic_save_temp_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("project.autolight");
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    path.with_file_name(format!(".{file_name}.tmp-{}-{nonce}", std::process::id()))
}

fn replace_project_file(tmp_path: &Path, path: &Path) -> Result<(), ProjectError> {
    fs::rename(tmp_path, path).map_err(|source| ProjectError::Write {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(not(windows))]
fn sync_parent_directory(path: &Path) -> Result<(), ProjectError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::File::open(parent)
            .and_then(|dir| dir.sync_all())
            .map_err(|source| ProjectError::Write {
                path: parent.to_path_buf(),
                source,
            })?;
    }
    Ok(())
}

#[cfg(windows)]
fn sync_parent_directory(_path: &Path) -> Result<(), ProjectError> {
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioAsset {
    pub id: String,
    pub path: String,
    pub duration: f64,
    pub sample_rate: u32,
    pub channels: u32,
    pub fingerprint: String,
    pub import_status: ImportStatus,
    pub relink_hint: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub id: String,
    #[serde(rename = "type")]
    pub track_type: TrackType,
    pub name: String,
    #[serde(default)]
    pub input_track_ids: Vec<String>,
    pub transform_id: String,
    #[serde(default)]
    pub transform_params: JsonObject,
    pub transform_version: String,
    pub output_schema: String,
    pub dependency_hash: String,
    pub result_state: ResultState,
    #[serde(default)]
    pub cache_refs: Vec<String>,
    #[serde(default)]
    pub provenance: JsonObject,
    pub error: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Marker {
    pub id: String,
    pub track_id: String,
    pub timestamp: f64,
    pub duration: Option<f64>,
    pub label: String,
    pub category: String,
    pub confidence: Option<f64>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub source_transform: String,
    #[serde(default)]
    pub source_marker_ids: Vec<String>,
    #[serde(default)]
    pub metadata: JsonObject,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JobRun {
    pub id: String,
    pub track_id: String,
    pub transform_id: String,
    #[serde(default)]
    pub transform_version: String,
    pub parameters_hash: String,
    #[serde(default)]
    pub parameters: JsonObject,
    pub state: ResultState,
    pub progress: f64,
    pub started_at: String,
    pub completed_at: String,
    pub error: String,
    #[serde(default)]
    pub produced_cache_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CacheEntry {
    pub id: String,
    pub dependency_hash: String,
    pub artifact_kind: String,
    pub path: String,
    pub created_at: String,
    pub transform_version: String,
    pub size_bytes: u64,
    pub payload_digest: String,
    pub validation_status: CacheValidationStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportStatus {
    Online,
    Offline,
    Modified,
}

impl ImportStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Online => "online",
            Self::Offline => "offline",
            Self::Modified => "modified",
        }
    }
}

impl std::fmt::Display for ImportStatus {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheValidationStatus {
    Valid,
    Invalid,
}

impl CacheValidationStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Valid => "valid",
            Self::Invalid => "invalid",
        }
    }
}

impl std::fmt::Display for CacheValidationStatus {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrackType {
    Source,
    Generated,
    Editable,
}

impl TrackType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Generated => "generated",
            Self::Editable => "editable",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResultState {
    Pending,
    Running,
    Complete,
    Stale,
    Failed,
    Cancelled,
    Blocked,
}

impl ResultState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Complete => "complete",
            Self::Stale => "stale",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Blocked => "blocked",
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use serde_json::{json, Value};

    use super::{ProjectDocument, TrackType, SCHEMA_VERSION};

    fn fixture_path(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/projects")
            .join(name)
    }

    fn round_trip_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "autolight-core-{name}-{}-{}.autolight",
            std::process::id(),
            std::thread::current().name().unwrap_or("project")
        ))
    }

    #[test]
    fn project_basic_graph_fixture_loads_schema_and_track_counts() {
        let project = ProjectDocument::load_path(fixture_path("basic_graph.autolight")).unwrap();

        assert_eq!(project.schema_version, SCHEMA_VERSION);
        assert_eq!(track_count(&project, TrackType::Source), 1);
        assert_eq!(track_count(&project, TrackType::Generated), 1);
        assert_eq!(track_count(&project, TrackType::Editable), 1);
    }

    #[test]
    fn project_tree_analysis_fixture_loads_nested_analysis_counts() {
        let project = ProjectDocument::load_path(fixture_path("tree_analysis.autolight")).unwrap();

        assert_eq!(project.schema_version, SCHEMA_VERSION);
        assert_eq!(track_count(&project, TrackType::Source), 1);
        assert_eq!(track_count(&project, TrackType::Generated), 5);
        assert_eq!(track_count(&project, TrackType::Editable), 1);
        assert_eq!(project.cache_entries.len(), 3);
        assert_eq!(project.job_runs.len(), 2);
    }

    #[test]
    fn project_round_trip_preserves_stable_semantic_fields_and_extensible_maps() {
        let project = ProjectDocument::load_path(fixture_path("tree_analysis.autolight")).unwrap();
        let path = round_trip_path("tree-analysis");

        project.save_path(&path).unwrap();
        let reloaded = ProjectDocument::load_path(&path).unwrap();
        fs::remove_file(&path).unwrap();

        assert_eq!(reloaded, project);
        assert_eq!(
            reloaded.ui_state["timeline"]["selected_track_id"],
            json!("track_drums")
        );
        assert_eq!(
            reloaded.tracks[1].transform_params["separation"]["model"],
            json!("demo-stems")
        );
        assert_eq!(
            reloaded.tracks[2].provenance["visible_energy"]["bins"],
            json!([0.2, 0.8, 0.4])
        );
        assert_eq!(
            reloaded.markers[3].metadata["editor"]["snap"],
            json!("beat-grid")
        );
    }

    #[test]
    fn project_load_rejects_unsupported_schema_version() {
        let mut raw = fs::read_to_string(fixture_path("basic_graph.autolight")).unwrap();
        raw = raw.replacen("\"schema_version\": 1", "\"schema_version\": 999", 1);

        let err = ProjectDocument::from_json_str(&raw).unwrap_err();

        assert!(err.to_string().contains("unsupported schema version: 999"));
    }

    #[test]
    fn project_load_rejects_invalid_graph() {
        let raw = fs::read_to_string(fixture_path("basic_graph.autolight")).unwrap();
        let mut project: Value = serde_json::from_str(&raw).unwrap();
        project["tracks"][1]["input_track_ids"] = json!(["missing_track"]);
        let raw = serde_json::to_string(&project).unwrap();

        let err = ProjectDocument::from_json_str(&raw).unwrap_err();

        assert!(err
            .to_string()
            .contains("invalid project graph: missing input track: missing_track"));
    }

    #[test]
    fn project_save_failure_does_not_replace_existing_file() {
        let mut project =
            ProjectDocument::load_path(fixture_path("basic_graph.autolight")).unwrap();
        let path = round_trip_path("invalid-save");
        project.save_path(&path).unwrap();
        let original = fs::read_to_string(&path).unwrap();
        project.tracks[1].input_track_ids = vec!["missing_track".to_string()];

        let err = project.save_path(&path).unwrap_err();

        assert!(err
            .to_string()
            .contains("missing input track: missing_track"));
        assert_eq!(fs::read_to_string(&path).unwrap(), original);
        fs::remove_file(&path).unwrap();
    }

    #[test]
    fn project_save_path_replaces_existing_file_contents() {
        let project = ProjectDocument::load_path(fixture_path("basic_graph.autolight")).unwrap();
        let path = round_trip_path("replace-existing");
        fs::write(&path, "old contents").unwrap();

        project.save_path(&path).unwrap();
        let reloaded = ProjectDocument::load_path(&path).unwrap();

        assert_eq!(reloaded, project);
        fs::remove_file(&path).unwrap();
    }

    fn track_count(project: &ProjectDocument, track_type: TrackType) -> usize {
        project
            .tracks
            .iter()
            .filter(|track| track.track_type == track_type)
            .count()
    }
}
