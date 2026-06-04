use autolight_core::transforms::{TransformRegistry, TransformSpec};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransformSpecRow {
    pub transform_id: String,
    pub version: String,
    pub name: String,
    pub estimated_cost: String,
    pub output_schema: String,
}

pub fn builtin_transform_spec_rows() -> Vec<TransformSpecRow> {
    transform_spec_rows(&TransformRegistry::with_builtin_transforms())
}

pub fn transform_spec_rows(registry: &TransformRegistry) -> Vec<TransformSpecRow> {
    registry.specs().into_iter().map(spec_row).collect()
}

pub fn transform_specs_json(registry: &TransformRegistry) -> Result<String, serde_json::Error> {
    serde_json::to_string(&transform_spec_rows(registry))
}

fn spec_row(spec: &TransformSpec) -> TransformSpecRow {
    TransformSpecRow {
        transform_id: spec.id.clone(),
        version: spec.version.clone(),
        name: spec.name.clone(),
        estimated_cost: spec.estimated_cost.clone(),
        output_schema: spec.output_schema.clone(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::{builtin_transform_spec_rows, transform_specs_json};
    use autolight_core::transforms::TransformRegistry;

    #[test]
    fn transform_model_exposes_builtin_spec_roles_as_json() {
        let registry = TransformRegistry::with_builtin_transforms();
        let payload = transform_specs_json(&registry).unwrap();
        let rows: Value = serde_json::from_str(&payload).unwrap();
        let first = &rows[0];

        assert_eq!(
            builtin_transform_spec_rows()
                .iter()
                .map(|row| row.transform_id.as_str())
                .collect::<Vec<_>>(),
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
        assert!(first.get("transformId").is_some());
        assert!(first.get("version").is_some());
        assert!(first.get("name").is_some());
        assert!(first.get("estimatedCost").is_some());
        assert!(first.get("outputSchema").is_some());
    }
}
