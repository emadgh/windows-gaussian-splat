#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelPack {
    LocalRecommended,
    TemporalQuality,
    FullHarmonizer,
    FullNuRec,
}

#[derive(Debug, Clone, Copy)]
pub struct ModelPackInfo {
    pub title: &'static str,
    pub model_download_gb: f32,
    pub free_disk_gb: u32,
    pub description: &'static str,
}

impl ModelPack {
    pub const ALL: [Self; 4] = [
        Self::LocalRecommended,
        Self::TemporalQuality,
        Self::FullHarmonizer,
        Self::FullNuRec,
    ];

    pub fn info(self) -> ModelPackInfo {
        match self {
            Self::LocalRecommended => ModelPackInfo {
                title: "Local / 16 GB — non-temporal Harmonizer",
                // 1.45 GB Harmonizer JIT + 1.2 GB Cosmos DiT, excluding tokenizer metadata.
                model_download_gb: 2.65,
                free_disk_gb: 120,
                description: "Recommended first install for RTX 4070 Ti Super. Downloads only the non-temporal Harmonizer checkpoint and required Cosmos base weights where selective download is supported.",
            },
            Self::TemporalQuality => ModelPackInfo {
                title: "Temporal Harmonizer — highest quality",
                // 5.04 GB temporal checkpoint + 1.2 GB Cosmos DiT, excluding tokenizer metadata.
                model_download_gb: 6.24,
                free_disk_gb: 120,
                description: "Adds the 5.04 GB temporal checkpoint. The installer benchmarks it on the local GPU and can fall back to non-temporal mode if memory is insufficient.",
            },
            Self::FullHarmonizer => ModelPackInfo {
                title: "Full official Harmonizer model repository",
                // Harmonizer repo is 6.49 GB; NVIDIA's helper also downloads the Cosmos base repo.
                model_download_gb: 10.1,
                free_disk_gb: 120,
                description: "Approximate lower bound when following NVIDIA's full checkpoint helper: 6.49 GB Harmonizer repo plus at least 3.6 GB of top-level Cosmos weights, before tokenizer files and container layers.",
            },
            Self::FullNuRec => ModelPackInfo {
                title: "Full NuRec runtime (optional / remote GPU)",
                model_download_gb: 33.73,
                free_disk_gb: 160,
                description: "Optional container payload estimate: ~13.31 GB NuRec main image + ~20.42 GB NuRec tools image, before unpacked layers and Harmonizer model cache. Not intended for the local 16 GB GPU.",
            },
        }
    }
}
