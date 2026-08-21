#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelPack {
    LocalRecommended,
    FullHarmonizer,
    FullNuRec,
}

#[derive(Debug, Clone, Copy)]
pub struct ModelPackInfo {
    pub title: &'static str,
    pub payload_download_gb: f32,
    pub free_disk_gb: u32,
    pub description: &'static str,
}

impl ModelPack {
    pub const ALL: [Self; 3] = [
        Self::LocalRecommended,
        Self::FullHarmonizer,
        Self::FullNuRec,
    ];

    pub fn info(self) -> ModelPackInfo {
        match self {
            Self::LocalRecommended => ModelPackInfo {
                title: "Local / 16 GB — public Harmonizer pipeline",
                // 5.04 GB diffusion_harmonizer.pkl + 1.2 GB Cosmos model.pt,
                // excluding tokenizer metadata/files. The same checkpoint supports --nontemporal.
                payload_download_gb: 6.24,
                free_disk_gb: 120,
                description: "Recommended for RTX 4070 Ti Super. Downloads the current public Harmonizer paper checkpoint plus the Cosmos Predict2 0.6B base model/tokenizer needed for inference.",
            },
            Self::FullHarmonizer => ModelPackInfo {
                title: "Full official Harmonizer checkpoint repositories",
                payload_download_gb: 10.1,
                free_disk_gb: 120,
                description: "Downloads the full NVIDIA Harmonizer and Cosmos Predict2 0.6B repositories. Use this only when you want all upstream checkpoints; the training dataset is never downloaded.",
            },
            Self::FullNuRec => ModelPackInfo {
                title: "Full NuRec containers (optional / larger GPU host)",
                payload_download_gb: 33.73,
                free_disk_gb: 160,
                description: "Approximate compressed NuRec container payload before unpacked layers and model cache. Full NuRec is hardware-gated and is not a local 16 GB backend.",
            },
        }
    }
}
