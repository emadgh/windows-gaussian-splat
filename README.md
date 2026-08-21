# Windows Gaussian Splat Studio

A Windows-first desktop application for turning video/image captures of environments and equipment into Gaussian Splat reconstructions, refining them with NVIDIA Harmonizer where available, and exporting production-friendly 3DGS assets for 3ds Max.

This repository is being bootstrapped around a native Rust GUI and pluggable reconstruction backends.

## Target workflow

`Video / Images -> frame extraction -> camera solve -> 3DGS reconstruction -> Harmonizer refinement -> PLY / SPZ export -> 3ds Max`

The application will expose two execution profiles:

- **Local 16 GB profile** — intended for GeForce RTX 4070 Ti Super-class GPUs. Reconstruction and Harmonizer inference are run as separate stages to stay inside VRAM limits.
- **Full NuRec profile** — integrates NVIDIA NuRec when the runtime is hosted on hardware meeting NVIDIA's NuRec VRAM requirements.

## Output policy

PLY is the primary interchange format because modern 3ds Max / Arnold can load Gaussian Splat PLY directly. SPZ is planned as an additional compact export.

> Status: initial project bootstrap.
