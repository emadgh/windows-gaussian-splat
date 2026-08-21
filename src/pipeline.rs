use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuntimeProfile {
    Local16Gb,
    FullNuRec,
}

impl RuntimeProfile {
    pub fn label(self) -> &'static str {
        match self {
            Self::Local16Gb => "Local 16 GB (recommended for RTX 4070 Ti Super)",
            Self::FullNuRec => "Full NVIDIA NuRec (24+ GB VRAM host)",
        }
    }
}

#[derive(Debug, Clone)]
pub struct PipelineStep {
    pub name: &'static str,
    pub detail: &'static str,
}

pub fn plan(profile: RuntimeProfile) -> Vec<PipelineStep> {
    let mut steps = vec![
        PipelineStep {
            name: "Extract frames",
            detail: "FFmpeg extracts sharp, spaced frames and preserves source metadata.",
        },
        PipelineStep {
            name: "Solve cameras",
            detail: "COLMAP/cuSFM-style camera calibration and pose estimation.",
        },
        PipelineStep {
            name: "Reconstruct 3DGS",
            detail: "Train a Gaussian Splat representation with a VRAM-aware preset.",
        },
    ];

    match profile {
        RuntimeProfile::Local16Gb => {
            steps.push(PipelineStep {
                name: "Harmonizer pass",
                detail: "Run NVIDIA Harmonizer as a separate 16 GB-friendly inference stage.",
            });
            steps.push(PipelineStep {
                name: "Offline refinement",
                detail: "Generate corrected pseudo-views and distill them back into the splat when supported by the selected reconstruction backend.",
            });
        }
        RuntimeProfile::FullNuRec => {
            steps.push(PipelineStep {
                name: "NuRec refinement",
                detail: "Run the NVIDIA NuRec reconstruction/refinement container with Harmonizer enabled.",
            });
        }
    }

    steps.push(PipelineStep {
        name: "Export",
        detail: "Write Gaussian Splat PLY as the primary 3ds Max interchange asset; SPZ export is planned as an additional compact format.",
    });

    steps
}
