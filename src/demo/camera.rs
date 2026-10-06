//! Camera that eases toward a target once it leaves a circular follow region.
//! See `docs/rfcs/0001-camera-movement.md`.

use bevy::prelude::*;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        PostUpdate,
        follow_target.before(TransformSystems::Propagate),
    );
}

/// Camera-side settings: what to follow, the follow region, and smoothing.
#[derive(Component, Reflect)]
#[reflect(Component)]
#[require(CameraPosition)]
pub struct CameraFollow {
    /// Entity to follow; any entity with a `Transform`. `None` means idle.
    pub target: Option<Entity>,
    /// Region radius as a fraction of the shorter visible dimension (0.0..=0.5).
    pub region_fraction: f32,
    /// Exponential decay rate passed to `smooth_nudge`; higher is snappier.
    pub decay_rate: f32,
}

impl Default for CameraFollow {
    fn default() -> Self {
        Self {
            target: None,
            region_fraction: 0.2,
            decay_rate: 8.0,
        }
    }
}

/// The camera's true, unrounded position; `Transform` holds the pixel-snapped copy.
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
struct CameraPosition(Vec2);

/// World-space rectangle the camera's view must stay inside; lives on the level entity.
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct LevelBounds(pub Rect);

/// Eases the camera toward `target_position` each frame, clamps it to the
/// level bounds, and writes a pixel-snapped copy to its `Transform`.
fn follow_target(
    time: Res<Time>,
    targets: Query<&Transform, Without<CameraFollow>>,
    bounds: Query<&LevelBounds>,
    camera: Single<
        (
            &mut Transform,
            &mut CameraPosition,
            &CameraFollow,
            &Projection,
        ),
        With<Camera2d>,
    >,
) {
    let (mut transform, mut position, follow, projection) = camera.into_inner();
    let Projection::Orthographic(projection) = projection else {
        return;
    };
    let Some(target) = follow.target.and_then(|entity| targets.get(entity).ok()) else {
        return;
    };

    let visible_size = projection.area.size();
    let goal = target_position(
        position.0,
        target.translation.xy(),
        visible_size,
        follow.region_fraction,
    );
    position
        .0
        .smooth_nudge(&goal, follow.decay_rate, time.delta_secs());

    if let Ok(bounds) = bounds.single() {
        position.0 = clamp_to_bounds(position.0, visible_size, bounds.0);
    }

    let snapped = position.0.round();
    transform.translation.x = snapped.x;
    transform.translation.y = snapped.y;
}

/// Camera position the camera should ease toward: unchanged if the target is
/// inside the follow region, otherwise the point that puts the target on its edge.
fn target_position(camera: Vec2, target: Vec2, visible_size: Vec2, region_fraction: f32) -> Vec2 {
    let radius = visible_size.min_element() * region_fraction;
    let offset = target - camera;
    let distance = offset.length();
    if distance <= radius {
        camera
    } else {
        target - offset / distance * radius
    }
}

/// Camera position moved the least needed to keep its view inside `bounds`,
/// centered on any axis where the view is larger than the bounds.
fn clamp_to_bounds(position: Vec2, visible_size: Vec2, bounds: Rect) -> Vec2 {
    let half = visible_size / 2.0;
    let min = bounds.min + half;
    let max = bounds.max - half;
    Vec2::new(
        clamp_axis(position.x, min.x, max.x),
        clamp_axis(position.y, min.y, max.y),
    )
}

/// `value` clamped to `min..=max`, or the midpoint when `min > max`.
fn clamp_axis(value: f32, min: f32, max: f32) -> f32 {
    if min > max {
        (min + max) / 2.0
    } else {
        value.clamp(min, max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VIEW: Vec2 = Vec2::new(1280.0, 720.0);
    /// Radius is 720 * 0.25 = 180.
    const FRACTION: f32 = 0.25;

    fn assert_near(actual: Vec2, expected: Vec2) {
        assert!(
            (actual - expected).length() < 1e-3,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn target_inside_region_leaves_camera_unchanged() {
        let camera = Vec2::new(10.0, 20.0);
        let goal = target_position(camera, camera + Vec2::new(100.0, 0.0), VIEW, FRACTION);
        assert_near(goal, camera);
    }

    #[test]
    fn target_outside_region_ends_up_on_the_edge() {
        let goal = target_position(Vec2::ZERO, Vec2::new(500.0, 0.0), VIEW, FRACTION);
        assert_near(goal, Vec2::new(320.0, 0.0));

        let target = Vec2::new(300.0, 400.0);
        let goal = target_position(Vec2::ZERO, target, VIEW, FRACTION);
        assert!(((target - goal).length() - 180.0).abs() < 1e-3);
        // The goal stays on the line between the camera and the target.
        assert!(goal.perp_dot(target).abs() < 1e-2);
    }

    #[test]
    fn target_exactly_on_the_edge_leaves_camera_unchanged() {
        let goal = target_position(Vec2::ZERO, Vec2::new(180.0, 0.0), VIEW, FRACTION);
        assert_near(goal, Vec2::ZERO);
    }

    #[test]
    fn target_at_camera_position_is_not_nan() {
        let camera = Vec2::new(5.0, 5.0);
        let goal = target_position(camera, camera, VIEW, FRACTION);
        assert_near(goal, camera);
        assert!(goal.is_finite());
    }

    #[test]
    fn radius_uses_the_shorter_dimension() {
        let target = Vec2::new(300.0, 0.0);
        let wide = target_position(Vec2::ZERO, target, Vec2::new(1280.0, 720.0), FRACTION);
        let tall = target_position(Vec2::ZERO, target, Vec2::new(720.0, 1280.0), FRACTION);
        assert_near(wide, tall);
    }

    #[test]
    fn clamp_leaves_positions_inside_bounds_unchanged() {
        let bounds = Rect::from_center_size(Vec2::ZERO, Vec2::new(3840.0, 2160.0));
        let position = Vec2::new(100.0, -200.0);
        assert_near(clamp_to_bounds(position, VIEW, bounds), position);
    }

    #[test]
    fn clamp_keeps_the_view_inside_bounds() {
        let bounds = Rect::from_center_size(Vec2::ZERO, Vec2::new(3840.0, 2160.0));
        let clamped = clamp_to_bounds(Vec2::new(9999.0, -9999.0), VIEW, bounds);
        assert_near(clamped, Vec2::new(1920.0 - 640.0, -1080.0 + 360.0));
    }

    #[test]
    fn clamp_centers_the_view_when_bounds_are_smaller() {
        let bounds = Rect::from_center_size(Vec2::new(50.0, 0.0), Vec2::new(400.0, 400.0));
        let clamped = clamp_to_bounds(Vec2::new(900.0, 900.0), VIEW, bounds);
        assert_near(clamped, Vec2::new(50.0, 0.0));
    }
}
