# Capture Guide — Factory Environments and Equipment

Good reconstruction still starts with good coverage. Harmonizer can repair weakly constrained rendered regions, but it should not be treated as a substitute for photographing surfaces that were never observed.

## Phone / camera settings

- Prefer the main 1× camera. Avoid automatic switching between main, ultrawide and telephoto lenses during one capture.
- Record 4K when storage permits. 30 fps is sufficient; Gaussian Splat Studio extracts a much smaller set of useful frames.
- Lock focus, exposure and white balance when the camera app supports it. Exposure/focus pumping creates inconsistent training images.
- Disable cinematic/artificial depth effects, beauty filters, HDR effects that strongly change frame-to-frame tone, and aggressive digital stabilization/cropping when possible.
- Move slowly. Rolling-shutter skew and motion blur are much harder to solve than simply having more frames.
- Clean the phone lens before each scan.

## Factory hall / production line

Use multiple overlapping passes rather than one fast walkthrough.

1. Start with a wide loop around the area at approximately chest height.
2. Add a second pass from a different height where practical.
3. Keep roughly 70–80% visual overlap while moving.
4. Turn corners slowly and continue looking toward already observed geometry for several seconds after a turn.
5. Capture both sides of long production lines. Do not rely on a single corridor pass.
6. Add closer passes around important structures that otherwise occupy only a small part of the wide capture.
7. Finish by reconnecting to an area seen near the beginning. This gives the camera solver stronger loop geometry even though the default video matcher does not require generative loop closure.

Avoid long sequences pointed at blank walls, glossy floors, smoke/steam, rapidly moving conveyors, people walking through most of the image, or repeating identical tile stacks with no unique surrounding features.

## Individual machine / equipment scan

For a machine that will become a separate 3ds Max asset:

1. Make one complete orbit at mid height.
2. Make a higher orbit aimed down toward top surfaces.
3. Make a lower/closer orbit for the base, feet, cables and lower panels.
4. Add deliberate close passes over controls, heads, rollers and other parts that need to survive a close render.
5. Pause your walking direction changes; do not whip-pan around corners of the machine.
6. Capture the rear and service side even if they are not visible in the final planned camera. Missing sides constrain later placement in Max.

For long machines, use overlapping sections rather than standing far away simply to fit the whole machine in frame.

## Scale reference

Monocular COLMAP reconstruction has no inherent metric scale. Put a known measurement into the scan when physical scale matters:

- Measure a rigid distance on the machine or floor, such as exactly 1000 mm between two identifiable corners/markers.
- Keep both reference points visible in several frames.
- Record that physical distance with the project notes.
- After reconstruction, determine the same distance in the splat/3ds Max scene and set the scale multiplier accordingly.

Do not use a flexible tape measure waving in the scene as the only reconstruction feature; fixed machine/floor landmarks are better.

## Reflective, polished and repetitive equipment

Mirror-like stainless steel, polished tile, wet glaze and glass violate the static-view appearance assumptions of photogrammetry/3DGS. Improve the capture by:

- using broad, stable factory illumination rather than a moving flashlight;
- avoiding a person/camera reflection dominating the surface;
- adding nearby textured context to help camera solving;
- moving more slowly and taking additional viewing angles;
- capturing labels, fasteners, seams and frame edges that provide stable features.

Harmonizer gating is deliberately conservative on these surfaces because a generative repair can look plausible while being mechanically wrong.

## Capture quality warning signs

Re-shoot when possible if the video contains:

- repeated autofocus hunting;
- strong motion blur during most turns;
- exposure changes of several stops;
- very low light/noisy frames;
- large unobserved backsides of the object;
- large groups of moving people/vehicles occluding the machine;
- lens changes during the same clip;
- mostly featureless or reflective views with no stable context.

A slower two-minute capture with strong overlap is usually more useful than a fast one-minute capture with twice as many blurred frames.
