# Windows Gaussian Splat Studio

A Windows-first desktop application for turning video/image captures of environments and equipment into Gaussian Splat reconstructions, refining them with NVIDIA Harmonizer where available, and exporting production-friendly 3DGS assets for 3ds Max.

The desktop UI is native Rust using `eframe`/`egui`. GPU workloads are isolated behind pluggable backends so the Windows application can use a local Windows tool, WSL2/Docker, or a remote Linux NuRec host without changing the UI.

## Target workflow

`Video / Images -> frame extraction -> camera solve -> 3DGS reconstruction -> Harmonizer -> offline refinement -> PLY / SPZ -> 3ds Max`

## Runtime profiles

### Local 16 GB

Target: RTX 4070 Ti Super 16 GB and similar cards.

Reconstruction and Harmonizer run as separate GPU stages so their peak VRAM is not additive. This is the default profile. The intended path is:

1. FFmpeg frame extraction and blur/duplicate filtering.
2. Camera solve with COLMAP-compatible tooling.
3. VRAM-aware 3DGS/gsplat reconstruction.
4. Render pseudo-views in weakly observed regions.
5. NVIDIA Harmonizer inference on those views.
6. Distill corrected pseudo-views back into the 3DGS representation.
7. Export splat PLY and optionally SPZ.

NVIDIA's NuRec skills documentation describes 16 GB as the practical floor for Harmonizer inference. Full NuRec reconstruction has a higher requirement and therefore is a separate profile.

### Full NVIDIA NuRec

Target: Linux x86_64 host with a supported NVIDIA GPU and **more than 24 GB VRAM**; NVIDIA recommends 48 GB+.

This backend will drive the official NuRec containers and enable Harmonizer through NuRec when the host meets the hardware requirements. A future remote-host option will let the Windows GUI submit jobs to a workstation/server with a larger GPU.

## Model/download footprint

The application will download models on demand rather than bundling them in the installer.

| Component | Approx. download | Notes |
| --- | ---: | --- |
| Harmonizer non-temporal checkpoint | 1.45 GB | Fast per-image checkpoint |
| Harmonizer temporal checkpoint | 5.04 GB | Highest-quality temporal checkpoint |
| Both Harmonizer checkpoints | 6.49 GB | Official Hugging Face repository total |
| Cosmos Predict2 0.6B base weights | at least 1.2 GB + tokenizer | Harmonizer inference also requires the base model; NVIDIA's helper currently downloads the whole model repository, including three ~1.2 GB top-level weights |
| NuRec main container | ~13.31 GB compressed for 26.04.00 | Optional; Full NuRec profile only |
| NuRec tools container | ~20.42 GB compressed | Optional; only needed for NuRec data preparation workflows |

The normal RTX 4070 Ti Super install should therefore avoid the full NuRec containers and download only the local reconstruction runtime plus Harmonizer/Cosmos assets that are actually selected.

## 3ds Max output

PLY is the primary interchange format. 3ds Max 2027.2 can import 3D Gaussian Splat data as PLY, SPZ, or LCC, and Arnold/MAXtoA can render Gaussian Splat PLY directly. Keeping the reconstruction as splats preserves view-dependent appearance better than converting everything to a conventional mesh.

For mixed factory scenes, the intended workflow is to keep the large environment as one or more splat assets, scan important equipment as separate splat assets, then position them independently in 3ds Max.

## Current status

The repository now contains the first native Rust GUI shell, Windows NVIDIA/WSL/Docker capability checks, VRAM-aware runtime profiles, and a Windows CI build. Execution backends and the one-click runtime/model installer are the next implementation milestones.

## Upstream projects

- NVIDIA Harmonizer: https://github.com/NVIDIA/harmonizer
- NVIDIA NuRec docs: https://docs.nvidia.com/nurec/
- NVIDIA gsplat: https://github.com/nerfstudio-project/gsplat
