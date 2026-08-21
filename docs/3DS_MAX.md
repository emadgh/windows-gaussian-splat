# 3ds Max Interchange

Gaussian Splat Studio targets 3ds Max 2027.2 or newer, where Autodesk added native 3D Gaussian Splat support for PLY, SPZ and LCC assets. Arnold/MAXtoA in that release can render native Gaussian Splat data as well.

## Recommended asset strategy

Keep scans modular:

```text
Factory.max
  Factory_Hall_refined.ply
  Durst_01_refined.ply
  Durst_02_refined.ply
  Polishing_Line_01_refined.ply
  Compressor_Room_refined.ply
  Forklift_01_refined.ply
  CAD / polygon geometry / lights
```

A lower-density environment capture establishes the visual context. Important equipment gets an independent close capture with better local detail and can then be transformed separately in Max.

## Import

Use the native 3ds Max 2027.2 3DGS import path and select `refined.ply` (recommended) or `refined.spz` (smaller). Autodesk's native Points/Gaussian Splat object can coexist with conventional mesh, spline and point objects.

`raw.ply` is retained for comparison/debugging. Use `refined.ply` for the final asset unless a refinement report says no views were accepted, in which case raw and refined are intentionally identical for that round.

## Orientation

The reconstruction PLY is exported without an arbitrary automatic axis rotation because preserving one coordinate frame between COLMAP, pseudo cameras and refined splats is more important during processing.

After import:

1. Inspect the asset orientation.
2. Set the Gaussian Splat **Up Axis** appropriately or rotate the object at the scene level.
3. Keep that orientation choice consistent for all assets captured with the same workflow.
4. Store final transforms in the Max scene rather than destructively modifying the source scan unless a downstream pipeline requires baked transforms.

The `project.json` file stores unit/up-axis intent as metadata; PLY itself does not carry the full project transform convention.

## Metric scale

A monocular video reconstruction has arbitrary global scale. Two supported workflows are:

### Scale in Gaussian Splat Studio

Determine a scale multiplier from a known dimension and enter it before export. The exporter multiplies Gaussian centers by that factor and adds `log(factor)` to each log-scale so the Gaussian ellipsoids scale correctly with their positions.

### Scale in 3ds Max

Import the asset, measure a known machine/floor dimension, calculate:

```text
desired physical length / imported measured length = scale factor
```

Then apply the same uniform factor to the whole Gaussian Splat object. This is often the most convenient approach when aligning separate equipment scans to a factory hall scan or CAD reference.

## PLY vs SPZ

Use **PLY** as the master/interchange file:

- standard 3DGS attributes;
- directly produced by gsplat;
- easiest to inspect and troubleshoot;
- directly supported by 3ds Max/Arnold Gaussian Splat workflows.

Use **SPZ** as a compact delivery/cache copy. Gaussian Splat Studio converts PLY with the official Niantic SPZ Python bindings and preserves the PLY as the authoritative asset even if optional SPZ conversion fails.

## Arnold rendering

Arnold supports Gaussian Splat rendering and can combine stored radiance/emission with relighting controls. For captured appearance, start with the stored/emission look. Increase diffuse contribution only when you intentionally want scene lights to relight the scan.

If a distant or sparse splat shows small holes, Arnold's Gaussian Splat **Min Pixel Width** can increase minimum on-screen splat size. Use it carefully: larger values improve coverage but can soften fine mechanical detail.

## Aligning separate equipment scans

For each machine scan:

1. establish a known physical dimension and scale the asset;
2. choose a repeatable anchor such as a machine foot, floor corner, column grid intersection, or known centerline;
3. move/rotate the equipment scan into the hall scan;
4. hide or remove the lower-quality representation of that equipment from the environment scan if the overlap causes visual doubling;
5. keep the equipment object independent so it can be replaced by a better future scan without rebuilding the hall.

For precise engineering layout, use CAD/survey measurements as the geometric authority and Gaussian Splats as the visual authority. A monocular GS capture is excellent for appearance and context but should not silently replace dimensional survey data.
