use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use thiserror::Error;

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
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn save_path(&self, path: impl AsRef<Path>) -> Result<(), ProjectError> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| ProjectError::CreateDirectory {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let output = self.to_json_string_pretty()?;
        fs::write(path, output).map_err(|source| ProjectError::Write {
            path: path.to_path_buf(),
            source,
        })
    }

    fn ensure_supported_schema(&self) -> Result<(), ProjectError> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(ProjectError::UnsupportedSchemaVersion(self.schema_version));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioAsset {
    pub id: String,
    pub path: String,
    pub duration: f64,
    pub sample_rate: u32,
    pub channels: u32,
    pub fingerprint: String,
    pub import_status: String,
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
    pub parameters_hash: String,
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
    pub validation_status: String,
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

    use serde_json::json;

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

    fn track_count(project: &ProjectDocument, track_type: TrackType) -> usize {
        project
            .tracks
            .iter()
            .filter(|track| track.track_type == track_type)
            .count()
    }
}
