#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelPack {
    LocalRecommended,
    NonTemporalJit,
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
    pub const ALL: [Self; 4] = [
        Self::LocalRecommended,
        Self::NonTemporalJit,
        Self::FullHarmonizer,
        Self::FullNuRec,
    ];

    pub fn info(self) -> ModelPackInfo {
        match self {
            Self::LocalRecommended => ModelPackInfo {
                title: "Local / 16 GB — public Harmonizer pipeline",
                // 5.04 GB diffusion_harmonizer.pkl + 1.2 GB Cosmos model.pt,
                // excluding tokenizer files. The same checkpoint can run with --nontemporal.
                payload_download_gb: 6.24,
                free_disk_gb: 120,
                description: "Recommended for RTX 4070 Ti Super. Uses NVIDIA's current public Harmonizer inference path. Starts in non-temporal mode on 16 GB, then enables temporal conditioning only after a local VRAM smoke test passes.",
            },
            Self::NonTemporalJit => ModelPackInfo {
                title: "Optional 1.45 GB non-temporal JIT checkpoint",
                payload_download_gb: 1.45,
                free_disk_gb: 120,
                description: "Small NVIDIA JIT checkpoint intended for fast per-image enhancement/NuRec integration. Kept as an optional acceleration path rather than the default standalone refinement backend.",
            },
            Self::FullHarmonizer => ModelPackInfo {
                title: "Full official Harmonizer checkpoint helper",
                // Harmonizer repo is 6.49 GB; NVIDIA's helper also downloads the Cosmos base repo.
                payload_download_gb: 10.1,
                free_disk_gb: 120,
                description: "Lower-bound estimate when following NVIDIA's full helper: 6.49 GB Harmonizer repository plus at least 3.6 GB of top-level Cosmos weights, before tokenizer files and container layers.",
            },
            Self::FullNuRec => ModelPackInfo {
                title: "Full NuRec containers (optional / remote GPU)",
                payload_download_gb: 33.73,
                free_disk_gb: 160,
                description: "Container payload estimate: ~13.31 GB NuRec main image + ~20.42 GB NuRec tools image, before unpacked layers and Harmonizer model cache. Not intended for the local 16 GB GPU.",
            },
        }
    }
}
