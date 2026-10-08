---
feature_name: camera_movement
start_date: 2026-10-05
status: implemented
---

## Summary
[summary]: #summary

The top down 2D camera stays centered on the character by smoothly adjusting
when the player moves outside of a implicit region in the center of the screen.

## Guide-level explanation
[guide-level-explanation]: #guide-level-explanation

The camera has a **follow region**: an invisible circle centered on the screen.
Its radius is a fixed fraction of the shorter screen dimension, so it looks
the same on any window size.

Think of the camera as tied to the character with a rope that has slack. The
radius of the circle is the length of the rope:

* While the character is inside the circle, the rope is slack and the camera
  does not move. Walking around in a small area feels stable.
* When the character reaches the edge, the rope goes taut and the camera is
  tugged along after them, easing toward them rather than snapping.
* The camera keeps following until the rope has some slack again, then
  settles. The character ends up on the edge, not re-centered.

For example, if the character walks steadily to the right, the rope goes
taut at the right edge of the circle and the camera glides after them. When
they stop, the camera finishes its glide and rests with the character at the
circle's right edge. Walking back to the left lets the rope go slack again,
so the camera stays put until the character reaches the left edge.

## Reference-level explanation
[reference-level-explanation]: #reference-level-explanation

### Data

```rust
/// Camera-side settings: what to follow, the follow region, and smoothing.
#[derive(Component)]
struct CameraFollow {
    /// Entity to follow; any entity with a `Transform`. `None` means idle.
    target: Option<Entity>,
    /// Region radius as a fraction of the shorter visible dimension (0.0..=0.5).
    region_fraction: f32,
    /// Exponential decay rate passed to `smooth_nudge`; higher is snappier.
    decay_rate: f32,
}
```

The camera knows nothing about players. Anything with a `Transform` can be
followed, and retargeting is just assigning `target`.

### Behavior

```rust
/// Camera position the camera should ease toward: unchanged if the target is
/// inside the follow region, otherwise the point that puts the target on its edge.
fn target_position(camera: Vec2, target: Vec2, visible_size: Vec2, region_fraction: f32) -> Vec2;

/// Eases the camera's xy toward `target_position` each frame, leaving z untouched.
fn follow_target(
    time: Res<Time>,
    targets: Query<&Transform, Without<CameraFollow>>,
    camera: Single<(&mut Transform, &CameraFollow, &Projection), With<Camera2d>>,
);

/// Registers `follow_target` in `PostUpdate`, before transform propagation.
struct CameraFollowPlugin;
```

`target_position` first derives the region's radius in world units as
`radius = visible_size.min_element() * region_fraction`. It returns `camera`
when `|target - camera| <= radius`. Otherwise it returns
`target - radius * normalize(target - camera)`. `follow_target` then calls
`smooth_nudge` on the camera's xy with `decay_rate` and `time.delta_secs()`.
`smooth_nudge` is framerate independent, so large or uneven frame times don't
change the result.

### Interactions

* **Assigning the target.** The code that spawns the player sets
  `CameraFollow::target` to the player entity. `target` is an `Option` so the
  camera can be spawned before the player without ordering constraints.
* **Ordering.** `follow_target` runs in `PostUpdate`, ordered
  `.before(TransformSystems::Propagate)`, so the camera sees the target's
  final position for the frame and avoids a one-frame lag.
* **Movement schedule.** This assumes the target moves in `Update`. If it
  moves to `FixedUpdate`, the camera must follow the interpolated visual
  transform instead of the physics position, or it will jitter.
* **Window size.** `follow_target` reads the visible size from the camera's
  world-space area each frame (not from window pixels) and passes it to
  `target_position`, so resizing the window or changing the `ScalingMode` needs no
  extra handling.

### Corner cases

* **No target, or target despawned.** If `target` is `None` or the entity no
  longer exists (or has no `Transform`), the camera does not move.
* **Target is the camera.** The `Without<CameraFollow>` filter excludes it,
  so a camera cannot follow itself.
* **Target exactly at the camera position.** The target is inside the
  region, so `normalize` is never reached with a zero vector.
* **Soft edge.** Because decay is exponential, a moving target trails the
  circle edge by about `speed / decay_rate`: it may sit slightly outside
  the region while moving, and settles exactly on the edge once it stops.
* **Retargeting.** Changing `target` mid-follow needs no reset. The goal is
  recomputed each frame from the new target, so the camera eases toward it.

### The guide example

Walking right: the target leaves the circle, `target_position` returns a point
left of the character by `radius`, and the camera eases toward it each frame
(the rope going taut). On stopping, the goal stops moving, and the camera
converges on it, leaving the character on the right edge. Walking back left
puts the target inside the region, so `target_position` returns the camera's own
position and nothing moves (the rope is slack) until the left edge is hit.

## Prior art
[prior-art]: #prior-art

The camera movement in this [indie game](./docs/concept-art/creature-keeper.mp4) is almost exactly the camera movement style I'm suggesting.

## Tests

### Unit tests

`target_position` is a pure function, so it is tested directly with no Bevy
`App`:

* Target inside the region: returns the camera position unchanged.
* Target outside the region: returns a point exactly `radius` from the
  target, on the line between the camera and the target.
* Target exactly on the edge: returns the camera position unchanged.
* Target exactly at the camera position: returns the camera position, with
  no NaN from normalizing a zero vector.
* Different `visible_size` values: the radius scales with the shorter
  dimension (wide and tall windows give the same result for the same short
  side).

### Test level

Seeing the camera work needs a level larger than the window. The test level
is a throwaway greybox, not level infrastructure:

* **Content.** A background and a few landmarks, spread over an area at
  least about three windows wide and tall. The current level spawns only the
  player and music, so with nothing drawn the camera's movement would be
  invisible.
* **No screen wrap.** The player must not have `ScreenWrap`, or they teleport
  at the window edge before the camera can follow.
* **Camera target.** The player spawn code sets `CameraFollow::target` to
  the player entity.
* **Bounds.** The level provides a rectangle for the camera clamp. For the
  test level it is a hardcoded value.

### Conclusions

Authored levels, tilemaps and a real source for level bounds are out of scope
for this RFC and should get their own RFC.

## Unresolved questions
[unresolved-questions]: #unresolved-questions

### To resolve through this RFC

* **Level bounds clamp.** The camera is clamped to the level bounds so the
  void is never visible. Where do the bounds live: a `LevelBounds` component
  on the level entity that the follow system reads, or a field on
  `CameraFollow`? The clamp must also account for the visible size, keeping
  the camera center at least half the visible size inside the bounds. This
  needs a section in the reference-level explanation once decided.
* **Pixel snapping.** The game is pixel art, and a camera moving by
  fractional amounts makes sprite pixels shimmer. Two options:
  1. **Round the camera's `Transform` directly.** After `smooth_nudge`,
     `follow_target` rounds `translation.xy` in place. This adds no new
     state. However, the nudge now starts from the rounded value each frame,
     so per-frame movements under half a pixel round away and the easing
     becomes steppy, stalling within half a pixel of the goal.
  2. **Keep an unrounded position and round on write.** The true camera
     position lives in its own field or component and is the value that gets
     nudged. Each frame it is rounded and written to `Transform`. The easing
     stays smooth, at the cost of extra state, and other systems reading the
     camera's `Transform` see the rounded value.

  Either option must pick the snap grid: one screen pixel, or one art pixel
  (depends on the `ScalingMode`).

### To resolve through implementation

* **Visible-size lookup.** How `follow_target` reads the camera's visible
  world-space size from `Projection` in Bevy 0.19.
* **Tuning values.** The defaults for `region_fraction` and `decay_rate`,
  chosen by playtesting against the creature-keeper video.

### Out of scope

* **Hard-cut retargeting.** The camera eases to a new target. Cutscenes may
  want an instant cut.
* **Fixed timestep.** Movement runs in `Update`. If it moves to
  `FixedUpdate`, the camera must follow the interpolated visual transform to
  avoid jitter.
* **Authored levels.** Tilemaps and a real source for level bounds (see
  Tests).
