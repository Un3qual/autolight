use std::collections::BTreeMap;

use thiserror::Error;

use crate::graph::{find_track, source_track_id_for_context};
use crate::project::{ProjectDocument, ResultState, Track, TrackType};

#[derive(Debug, Error)]
pub enum TransformError {
    #[error("duplicate transform id/version: {id}@{version}")]
    DuplicateVersion { id: String, version: String },
    #[error("unknown transform id: {0}")]
    UnknownTransform(String),
    #[error("multiple versions registered for transform {0}")]
    MultipleVersions(String),
    #[error("transform {id} version mismatch: requested {requested}, available {available:?}")]
    VersionMismatch {
        id: String,
        requested: String,
        available: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransformSpec {
    pub id: String,
    pub version: String,
    pub name: String,
    pub input_schema: String,
    pub output_schema: String,
    pub estimated_cost: String,
}

impl TransformSpec {
    pub fn new(
        id: impl Into<String>,
        version: impl Into<String>,
        name: impl Into<String>,
        input_schema: impl Into<String>,
        output_schema: impl Into<String>,
        estimated_cost: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            version: version.into(),
            name: name.into(),
            input_schema: input_schema.into(),
            output_schema: output_schema.into(),
            estimated_cost: estimated_cost.into(),
        }
    }

    pub fn is_audio_input(&self) -> bool {
        self.input_schema == "audio.v1"
    }

    pub fn is_compatible_parent(&self, project: &ProjectDocument, parent_track_id: &str) -> bool {
        match self.input_schema.as_str() {
            "audio.v1" => parent_has_audio_context(project, parent_track_id),
            "audio-or-markers.v1" => find_track(project, parent_track_id)
                .is_some_and(|track| track.result_state == ResultState::Complete),
            _ => find_track(project, parent_track_id).is_some(),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct TransformRegistry {
    transforms: BTreeMap<String, BTreeMap<String, TransformSpec>>,
}

impl TransformRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_builtin_transforms() -> Self {
        let mut registry = Self::new();
        for spec in builtin_transform_specs() {
            registry
                .register(spec)
                .expect("builtin transform ids are unique");
        }
        registry
    }

    pub fn register(&mut self, spec: TransformSpec) -> Result<(), TransformError> {
        let versions = self.transforms.entry(spec.id.clone()).or_default();
        if versions.contains_key(&spec.version) {
            return Err(TransformError::DuplicateVersion {
                id: spec.id,
                version: spec.version,
            });
        }
        versions.insert(spec.version.clone(), spec);
        Ok(())
    }

    pub fn get(
        &self,
        transform_id: &str,
        version: Option<&str>,
    ) -> Result<&TransformSpec, TransformError> {
        let versions = self
            .transforms
            .get(transform_id)
            .ok_or_else(|| TransformError::UnknownTransform(transform_id.to_string()))?;

        match version {
            Some(version) => versions
                .get(version)
                .ok_or_else(|| TransformError::VersionMismatch {
                    id: transform_id.to_string(),
                    requested: version.to_string(),
                    available: versions.keys().cloned().collect(),
                }),
            None if versions.len() == 1 => Ok(versions.values().next().unwrap()),
            None => Err(TransformError::MultipleVersions(transform_id.to_string())),
        }
    }

    pub fn ids(&self) -> Vec<String> {
        self.transforms.keys().cloned().collect()
    }

    pub fn specs(&self) -> Vec<&TransformSpec> {
        self.transforms
            .values()
            .flat_map(|versions| versions.values())
            .collect()
    }

    pub fn compatible_specs<'a>(
        &'a self,
        project: &ProjectDocument,
        parent_track_id: &str,
    ) -> Vec<&'a TransformSpec> {
        self.specs()
            .into_iter()
            .filter(|spec| spec.is_compatible_parent(project, parent_track_id))
            .collect()
    }
}

pub fn builtin_transform_specs() -> Vec<TransformSpec> {
    vec![
        TransformSpec::new(
            "markers.fixed_interval",
            "1",
            "Fixed Interval Markers",
            "audio-or-markers.v1",
            "markers.v1",
            "light",
        ),
        TransformSpec::new(
            "stems.vocals_stand_in",
            "1",
            "Vocals Stem Stand-In",
            "audio.v1",
            "artifact.stem.v1",
            "heavy",
        ),
        TransformSpec::new(
            "audio.drums_stand_in",
            "1",
            "Drums Stem Stand-In",
            "audio.v1",
            "artifact.audio.v1",
            "medium",
        ),
        TransformSpec::new(
            "timing.onsets",
            "1",
            "Onsets",
            "audio.v1",
            "markers.v1",
            "medium",
        ),
        TransformSpec::new(
            "timing.beats",
            "1",
            "Beats",
            "audio.v1",
            "markers.v1",
            "medium",
        ),
        TransformSpec::new(
            "waveform.summary",
            "1",
            "Waveform Summary",
            "audio.v1",
            "artifact.waveform.v1",
            "medium",
        ),
        TransformSpec::new(
            "music.beat_grid",
            "1",
            "Beat Grid",
            "audio.v1",
            "artifact.beat-grid.v1",
            "medium",
        ),
        TransformSpec::new(
            "music.energy_profile",
            "1",
            "Energy Profile",
            "audio.v1",
            "artifact.energy.v1",
            "medium",
        ),
        TransformSpec::new(
            "music.harmonic_color",
            "1",
            "Harmonic Color",
            "audio.v1",
            "artifact.harmonic-color.v1",
            "medium",
        ),
    ]
}

pub fn parent_has_audio_context(project: &ProjectDocument, parent_track_id: &str) -> bool {
    let Some(track) = find_track(project, parent_track_id) else {
        return false;
    };
    match track.track_type {
        TrackType::Source => source_track_has_online_asset(project, parent_track_id),
        TrackType::Generated => track_has_valid_audio_artifact(project, track),
        TrackType::Editable => {
            track_has_valid_audio_artifact(project, track)
                || source_track_id_for_context(project, parent_track_id).is_some_and(
                    |source_track_id| source_track_has_online_asset(project, &source_track_id),
                )
        }
    }
}

fn source_track_has_online_asset(project: &ProjectDocument, source_track_id: &str) -> bool {
    let Some(source_track) = find_track(project, source_track_id) else {
        return false;
    };
    if source_track.track_type != TrackType::Source
        || source_track.result_state != ResultState::Complete
    {
        return false;
    }
    let Some(asset_id) = source_track
        .provenance
        .get("asset_id")
        .and_then(|value| value.as_str())
    else {
        return false;
    };
    project.audio_assets.iter().any(|asset| {
        asset.id == asset_id && asset.import_status == "online" && !asset.path.is_empty()
    })
}

fn track_has_valid_audio_artifact(project: &ProjectDocument, track: &Track) -> bool {
    if track.result_state != ResultState::Complete {
        return false;
    }
    track.cache_refs.iter().any(|cache_ref| {
        project.cache_entries.iter().any(|entry| {
            entry.id == *cache_ref
                && entry.validation_status == "valid"
                && matches!(entry.artifact_kind.as_str(), "audio" | "stem")
        })
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        builtin_transform_specs, parent_has_audio_context, TransformRegistry, TransformSpec,
    };
    use crate::project::{
        AudioAsset, CacheEntry, JsonObject, ProjectDocument, ResultState, Track, TrackType,
    };

    #[test]
    fn transforms_builtin_registry_exposes_python_registry_specs() {
        let registry = TransformRegistry::with_builtin_transforms();

        assert_eq!(
            registry.ids(),
            [
                "audio.drums_stand_in",
                "markers.fixed_interval",
                "music.beat_grid",
                "music.energy_profile",
                "music.harmonic_color",
                "stems.vocals_stand_in",
                "timing.beats",
                "timing.onsets",
                "waveform.summary",
            ]
        );
        let fixed = registry.get("markers.fixed_interval", Some("1")).unwrap();
        assert_eq!(fixed.name, "Fixed Interval Markers");
        assert_eq!(fixed.input_schema, "audio-or-markers.v1");
        assert_eq!(fixed.output_schema, "markers.v1");
        assert_eq!(fixed.estimated_cost, "light");
        for (transform_id, output_schema) in [
            ("waveform.summary", "artifact.waveform.v1"),
            ("music.beat_grid", "artifact.beat-grid.v1"),
            ("music.energy_profile", "artifact.energy.v1"),
            ("music.harmonic_color", "artifact.harmonic-color.v1"),
        ] {
            let spec = registry.get(transform_id, Some("1")).unwrap();
            assert_eq!(spec.input_schema, "audio.v1");
            assert_eq!(spec.output_schema, output_schema);
        }

        assert_eq!(builtin_transform_specs().len(), 9);
    }

    #[test]
    fn transforms_registry_rejects_duplicates_and_reports_version_mismatch() {
        let mut registry = TransformRegistry::default();
        registry
            .register(TransformSpec::new(
                "test.versioned",
                "1",
                "Versioned 1",
                "audio.v1",
                "markers.v1",
                "light",
            ))
            .unwrap();
        let duplicate = registry
            .register(TransformSpec::new(
                "test.versioned",
                "1",
                "Versioned 1 Duplicate",
                "audio.v1",
                "markers.v1",
                "light",
            ))
            .unwrap_err();

        assert!(duplicate.to_string().contains("duplicate"));
        assert!(registry
            .get("test.versioned", Some("2"))
            .unwrap_err()
            .to_string()
            .contains("version mismatch"));
    }

    #[test]
    fn transforms_registry_requires_explicit_version_for_multi_version_specs() {
        let mut registry = TransformRegistry::default();
        registry
            .register(TransformSpec::new(
                "test.versioned",
                "1",
                "Versioned 1",
                "audio.v1",
                "markers.v1",
                "light",
            ))
            .unwrap();
        registry
            .register(TransformSpec::new(
                "test.versioned",
                "2",
                "Versioned 2",
                "audio.v1",
                "markers.v1",
                "light",
            ))
            .unwrap();

        assert!(registry
            .get("test.versioned", None)
            .unwrap_err()
            .to_string()
            .contains("multiple versions"));
        assert_eq!(registry.specs()[1].version, "2");
    }

    #[test]
    fn transforms_audio_parent_compatibility_accepts_source_or_audio_artifact_context() {
        let mut project = project_with_source();
        project.tracks.extend([
            generated_track(
                "track_markers",
                "track_source",
                "markers.v1",
                Vec::default(),
            ),
            generated_track(
                "track_stem",
                "track_source",
                "artifact.stem.v1",
                vec!["cache_stem".to_string()],
            ),
        ]);
        project
            .cache_entries
            .push(cache_entry("cache_stem", "stem"));

        assert!(parent_has_audio_context(&project, "track_source"));
        assert!(parent_has_audio_context(&project, "track_stem"));
        assert!(!parent_has_audio_context(&project, "track_markers"));

        project.audio_assets[0].import_status = "offline".to_string();
        assert!(!parent_has_audio_context(&project, "track_markers"));
        assert!(parent_has_audio_context(&project, "track_stem"));
    }

    #[test]
    fn transforms_compatible_specs_filter_audio_inputs_for_parent_state() {
        let mut project = project_with_source();
        project.tracks.push(generated_track(
            "track_stale_markers",
            "track_source",
            "markers.v1",
            Vec::default(),
        ));
        project.tracks[1].result_state = ResultState::Stale;
        let registry = TransformRegistry::with_builtin_transforms();

        let source_ids = registry
            .compatible_specs(&project, "track_source")
            .into_iter()
            .map(|spec| spec.id.as_str())
            .collect::<Vec<_>>();
        let stale_ids = registry
            .compatible_specs(&project, "track_stale_markers")
            .into_iter()
            .map(|spec| spec.id.as_str())
            .collect::<Vec<_>>();

        assert!(source_ids.contains(&"waveform.summary"));
        assert!(source_ids.contains(&"markers.fixed_interval"));
        assert!(stale_ids.is_empty());
    }

    fn project_with_source() -> ProjectDocument {
        let mut project = ProjectDocument::new("project_1", "Demo");
        project.audio_assets.push(AudioAsset {
            id: "asset_source".to_string(),
            path: "/fixtures/audio/source.wav".to_string(),
            duration: 12.0,
            sample_rate: 44_100,
            channels: 2,
            fingerprint: "fingerprint".to_string(),
            import_status: "online".to_string(),
            relink_hint: String::default(),
        });
        project.tracks.push(Track {
            id: "track_source".to_string(),
            track_type: TrackType::Source,
            name: "Source".to_string(),
            input_track_ids: Vec::default(),
            transform_id: String::default(),
            transform_params: JsonObject::default(),
            transform_version: String::default(),
            output_schema: String::default(),
            dependency_hash: String::default(),
            result_state: ResultState::Complete,
            cache_refs: Vec::default(),
            provenance: object(json!({"asset_id": "asset_source"})),
            error: String::default(),
        });
        project
    }

    fn generated_track(
        id: &str,
        parent_id: &str,
        output_schema: &str,
        cache_refs: Vec<String>,
    ) -> Track {
        Track {
            id: id.to_string(),
            track_type: TrackType::Generated,
            name: id.to_string(),
            input_track_ids: vec![parent_id.to_string()],
            transform_id: "markers.fixed_interval".to_string(),
            transform_params: JsonObject::default(),
            transform_version: "1".to_string(),
            output_schema: output_schema.to_string(),
            dependency_hash: "dep".to_string(),
            result_state: ResultState::Complete,
            cache_refs,
            provenance: JsonObject::default(),
            error: String::default(),
        }
    }

    fn cache_entry(id: &str, artifact_kind: &str) -> CacheEntry {
        CacheEntry {
            id: id.to_string(),
            dependency_hash: "dep".to_string(),
            artifact_kind: artifact_kind.to_string(),
            path: format!("{artifact_kind}/{id}.bin"),
            created_at: String::default(),
            transform_version: "1".to_string(),
            size_bytes: 0,
            payload_digest: String::default(),
            validation_status: "valid".to_string(),
        }
    }

    fn object(value: serde_json::Value) -> JsonObject {
        value.as_object().cloned().unwrap()
    }
}
