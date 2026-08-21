use crate::config::ProjectSettings;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetOutputs {
    pub raw_ply: Option<String>,
    pub refined_ply: Option<String>,
    pub refined_spz: Option<String>,
    pub refinement_report: Option<String>,
}

impl Default for AssetOutputs {
    fn default() -> Self {
        Self {
            raw_ply: None,
            refined_ply: None,
            refined_spz: None,
            refinement_report: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectManifest {
    pub schema_version: u32,
    pub project_id: String,
    pub created_unix: u64,
    pub source_video: String,
    pub output_dir: String,
    pub settings: ProjectSettings,
    pub outputs: AssetOutputs,
    pub status: String,
}

impl ProjectManifest {
    pub fn new(input_video: &Path, output_dir: &Path, settings: ProjectSettings) -> Self {
        let created_unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let stem = input_video
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("scan");

        Self {
            schema_version: 1,
            project_id: format!("{stem}-{created_unix}"),
            created_unix,
            source_video: input_video.display().to_string(),
            output_dir: output_dir.display().to_string(),
            settings,
            outputs: AssetOutputs::default(),
            status: "created".to_owned(),
        }
    }

    pub fn manifest_path(&self) -> PathBuf {
        Path::new(&self.output_dir).join("project.json")
    }

    pub fn save(&self) -> io::Result<()> {
        fs::create_dir_all(&self.output_dir)?;
        let bytes = serde_json::to_vec_pretty(self).map_err(io::Error::other)?;
        fs::write(self.manifest_path(), bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_uses_source_name_in_id() {
        let manifest = ProjectManifest::new(
            Path::new("C:/captures/durst-01.mp4"),
            Path::new("C:/out/durst-01"),
            ProjectSettings::default(),
        );
        assert!(manifest.project_id.starts_with("durst-01-"));
        assert_eq!(manifest.schema_version, 1);
    }
}
