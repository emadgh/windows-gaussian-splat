use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MatcherMode {
    Auto,
    Exhaustive,
    Sequential,
}

impl MatcherMode {
    pub const ALL: [Self; 3] = [Self::Auto, Self::Exhaustive, Self::Sequential];

    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::Exhaustive => "Exhaustive",
            Self::Sequential => "Sequential",
        }
    }

    pub fn cli_value(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Exhaustive => "exhaustive",
            Self::Sequential => "sequential",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QualityPreset {
    Safe,
    High,
}

impl QualityPreset {
    pub const ALL: [Self; 2] = [Self::Safe, Self::High];

    pub fn label(self) -> &'static str {
        match self {
            Self::Safe => "Safe / 1M Gaussians",
            Self::High => "High / 2M Gaussians",
        }
    }

    pub fn cap_max(self) -> u32 {
        match self {
            Self::Safe => 1_000_000,
            Self::High => 2_000_000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureSettings {
    pub frame_fps: f32,
    pub blur_threshold: f64,
    pub duplicate_threshold: f32,
    pub max_frames: u32,
    pub matcher: MatcherMode,
}

impl Default for CaptureSettings {
    fn default() -> Self {
        Self {
            frame_fps: 3.0,
            blur_threshold: 70.0,
            duplicate_threshold: 0.985,
            max_frames: 1200,
            matcher: MatcherMode::Auto,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingSettings {
    pub quality: QualityPreset,
    pub max_steps: u32,
}

impl Default for TrainingSettings {
    fn default() -> Self {
        Self {
            quality: QualityPreset::Safe,
            max_steps: 30_000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefinementSettings {
    pub enabled: bool,
    pub rounds: u8,
    pub temporal: bool,
    pub resolution: u32,
    pub distill_steps: u32,
    pub max_mean_change: f32,
    pub min_edge_correlation: f32,
}

impl Default for RefinementSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            rounds: 1,
            temporal: false,
            resolution: 1024,
            distill_steps: 750,
            max_mean_change: 0.22,
            min_edge_correlation: 0.35,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportSettings {
    pub export_spz: bool,
    pub unit: String,
    pub up_axis: String,
    pub scale_multiplier: f64,
}

impl Default for ExportSettings {
    fn default() -> Self {
        Self {
            export_spz: true,
            unit: "millimeter".to_owned(),
            up_axis: "Z".to_owned(),
            scale_multiplier: 1.0,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectSettings {
    pub capture: CaptureSettings,
    pub training: TrainingSettings,
    pub refinement: RefinementSettings,
    pub export: ExportSettings,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_json() {
        let settings = ProjectSettings::default();
        let json = serde_json::to_string(&settings).unwrap();
        let decoded: ProjectSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.training.max_steps, 30_000);
        assert_eq!(decoded.training.quality.cap_max(), 1_000_000);
        assert!(decoded.refinement.enabled);
    }
}
