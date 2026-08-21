# Windows Gaussian Splat Studio

Windows-first desktop workflow for turning ordinary video captures of environments and equipment into Gaussian Splat assets, refining weakly reconstructed views with NVIDIA Harmonizer, and exporting PLY/SPZ assets for 3ds Max.

The desktop application is native Rust (`eframe`/`egui`). CUDA reconstruction runs in a dedicated Ubuntu 24.04 WSL2 runtime so the Windows GUI stays small while NVIDIA/PyTorch tooling runs in its supported Linux environment.

## Primary workflow

```text
MP4 / MOV
  -> FFmpeg frame extraction
  -> blur + duplicate filtering
  -> COLMAP calibration / camera solve
  -> gsplat MCMC + 3DGUT reconstruction
  -> raw.ply
  -> conservative interpolated pseudo views
  -> NVIDIA Harmonizer
  -> geometry / edge consistency gates
  -> offline gradient distillation into the splat
  -> refined.ply + optional refined.spz
  -> 3ds Max 2027.2+
```

The raw reconstruction is always preserved. Harmonizer is not allowed to freely invent remote viewpoints: pseudo cameras are midpoint interpolations between adjacent cameras that COLMAP actually registered, large camera gaps are rejected, and strongly altered or geometrically inconsistent Harmonizer outputs are not used for distillation.

## Target hardware

The default `Local 16 GB` profile is designed around RTX 4070 Ti Super-class hardware:

- NVIDIA RTX GPU with 16 GB VRAM recommended for the local Harmonizer workflow.
- 64 GB system RAM recommended for large factory captures.
- Windows 11 with current NVIDIA Windows driver and WSL2 support.
- At least 120 GB free disk is recommended for CUDA/toolchain, Docker layers, model cache, intermediate images, training checkpoints, and outputs.

Full NVIDIA NuRec remains a separate hardware-gated backend because current NuRec reconstruction requires more than 24 GB VRAM and NVIDIA recommends substantially more for the full stack. It is not forced onto a 16 GB GPU.

## Easy installation

Build or run the Windows app, then use the buttons in this order:

1. **Install / Repair Runtime** — enables/uses WSL2, installs Ubuntu 24.04, CUDA 12.8 toolkit, COLMAP, FFmpeg, Docker Engine, NVIDIA Container Toolkit, PyTorch, current gsplat/3DGUT, NVIDIA Harmonizer, and Niantic SPZ bindings. Docker Desktop is not required.
2. **Hugging Face Login** — opens an interactive WSL terminal running `hf auth login`. The app never stores a Hugging Face token in the repository or project JSON.
3. Accept the NVIDIA Cosmos Predict2 model terms in the associated Hugging Face account if required.
4. **Download Model Pack** — the recommended local pack downloads the public `diffusion_harmonizer.pkl` checkpoint and the Cosmos Predict2 0.6B `model.pt` + tokenizer needed by current Harmonizer inference.
5. **Refresh checks** — verifies WSL runtime, CUDA visibility and model files.
6. Select a video and a parent project folder, configure quality/refinement, then press **Start Reconstruction**.

The installer is resumable. Re-running **Install / Repair Runtime** skips stamped completed stages and repairs/verifies the environment.

## Model footprint

Models are downloaded on demand and are never committed to this repository.

| Pack | Approx. model download | Use |
| --- | ---: | --- |
| Local recommended | ~6.24 GB + tokenizer/metadata | Default RTX 4070 Ti Super path; `diffusion_harmonizer.pkl` + Cosmos 0.6B base model |
| Full Harmonizer repos | ~10.1 GB+ | Optional; downloads all files from the two official model repositories, but never the training dataset |
| Full NuRec containers | ~33.7 GB compressed before cache/layers | Optional larger-GPU host only |

The approximately 120 GB disk recommendation is **not** the model download size. Most of that allowance is runtime/container/build/cache/intermediate space.

## Reconstruction defaults

The safe preset intentionally prioritizes stability over maximum splat count:

- 3 FPS frame extraction.
- Laplacian blur filtering and consecutive-frame duplicate rejection.
- COLMAP `OPENCV` single-camera calibration.
- Auto matcher: exhaustive for up to 350 retained frames, sequential for larger captures.
- gsplat MCMC strategy with 3DGUT enabled (`--with-ut --with-eval3d`).
- World normalization disabled so COLMAP and splat coordinates remain in the same reconstruction frame for refinement.
- 30,000 training steps and a 1,000,000 Gaussian cap by default.
- One Harmonizer/distillation round by default; 1–3 rounds available.
- Non-temporal Harmonizer by default on 16 GB; temporal mode can be selected and automatically falls back to non-temporal if the temporal run fails.

## Offline refinement

Each refinement round does the following:

1. Loads the current PLY and the original registered COLMAP cameras.
2. Chooses neighboring captured cameras and rejects unusually large camera gaps.
3. Creates midpoint camera poses only within that observed camera path.
4. Renders those pseudo views from the current splat using gsplat 3DGUT.
5. Runs current NVIDIA Harmonizer inference in its Docker environment.
6. Rejects generated views when mean RGB change is too large or edge correlation to the rendered source is too low.
7. Optimizes the **actual** Gaussian parameters against accepted Harmonizer targets. Means, scales and opacities are strongly anchored; the number of Gaussians is fixed and no densification happens during refinement.
8. Writes a per-round JSON report including accepted/rejected views and before/after pseudo-target PSNR.

If no generated view passes the gates, the input PLY is copied unchanged. This is intentional for industrial scans where plausible-but-wrong geometry is worse than a visible hole.

## Output layout

Every click on **Start Reconstruction** creates a unique project folder:

```text
<chosen folder>/machine-1700000000/
  project.json
  data/
    images_raw/
    images/
    database.db
    sparse/0/
  training_raw/
    ckpts/
    ply/
  raw.ply
  refinement/
    round_01/
      cameras.json
      raw/
      raw_harmonized/
      refined.ply
      report.json
  refined.ply
  refined.spz          # when optional SPZ export succeeds
```

`project.json` records source capture, settings, scale multiplier, unit/up-axis metadata, final assets and refinement report paths.

## 3ds Max workflow

`refined.ply` is the primary interchange asset for 3ds Max 2027.2+ / Arnold Gaussian Splat workflows. `refined.spz` is an optional compressed copy produced with the official Niantic SPZ library.

For a factory project, scan the hall/line as one asset and important machines as separate assets. Keep each equipment scan independent, then place/scale the assets in 3ds Max. Because monocular COLMAP reconstruction has arbitrary metric scale, set a measured scale multiplier before final export or calibrate the imported asset against a known dimension in Max.

See [docs/CAPTURE_GUIDE.md](docs/CAPTURE_GUIDE.md) and [docs/3DS_MAX.md](docs/3DS_MAX.md).

## Development

```powershell
cargo fmt --all --check
cargo check --all-targets
cargo test --all-targets
cargo run --release
```

CI validates the Rust application on Windows and statically compiles/parses the Python, Bash and PowerShell runtime scripts. GPU reconstruction/Harmonizer execution requires a real NVIDIA CUDA workstation and therefore cannot be exercised by standard GitHub-hosted CI.

## Upstream projects

- NVIDIA Harmonizer: https://github.com/NVIDIA/harmonizer
- NVIDIA NuRec: https://docs.nvidia.com/nurec/
- gsplat / 3DGUT: https://github.com/nerfstudio-project/gsplat
- COLMAP: https://github.com/colmap/colmap
- Niantic SPZ: https://github.com/nianticlabs/spz
