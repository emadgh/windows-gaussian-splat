# Runtime Architecture

## Goal

Windows GUI orchestrates local reconstruction and NVIDIA refinement without coupling the UI to a single backend.

## Local RTX 4070 Ti Super profile

Pipeline:

1. Validate NVIDIA driver and CUDA visibility.
2. Extract frames with FFmpeg.
3. Remove blurred and duplicate frames.
4. Solve cameras using COLMAP-compatible tooling.
5. Train 3DGS/3DGUT with VRAM-aware settings.
6. Render weak-observation pseudo views.
7. Run NVIDIA Harmonizer.
8. Validate generated views using geometry and photometric consistency.
9. Distill accepted views back into the splat representation.
10. Export PLY/SPZ and metadata.

## Industrial safety rules

- Never generate distant unseen geometry by default.
- Keep raw and refined assets separately.
- Preserve camera poses and capture metadata.
- Store scale calibration information.

## Asset layout

```
asset/
  raw.ply
  refined.ply
  refined.spz
  project.json
  cameras/
  previews/
  refinement-report.json
```

## Future NuRec backend

The Windows UI can submit jobs to a larger Linux host when VRAM requirements exceed local hardware.