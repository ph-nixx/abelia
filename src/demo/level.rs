//! Spawn the main level.

use bevy::{
    ecs::spawn::{SpawnIter, SpawnRelated},
    prelude::*,
};

use crate::{
    asset_tracking::LoadResource,
    audio::music,
    demo::{
        camera::LevelBounds,
        player::{PlayerAssets, player},
    },
    screens::Screen,
};

pub(super) fn plugin(app: &mut App) {
    app.load_resource::<LevelAssets>();
}

/// Size of the test level in world units, about three default windows wide and tall.
/// The level is centered on the origin, so the camera clamp bounds are a rect of this size.
pub const LEVEL_SIZE: Vec2 = Vec2::new(3840.0, 2160.0);

/// Distance between neighboring landmarks in world units.
const LANDMARK_SPACING: f32 = 400.0;

/// Side length of a landmark square in world units.
const LANDMARK_SIZE: f32 = 64.0;

/// A flat-colored rectangle covering the whole level, drawn behind everything.
fn background() -> impl Bundle {
    (
        Name::new("Background"),
        Sprite::from_color(Color::srgb(0.12, 0.14, 0.18), LEVEL_SIZE),
        Transform::from_xyz(0.0, 0.0, -1.0),
    )
}

/// A small colored square at `position` for the camera to move against.
fn landmark(position: Vec2, color: Color) -> impl Bundle {
    (
        Name::new("Landmark"),
        Sprite::from_color(color, Vec2::splat(LANDMARK_SIZE)),
        Transform::from_translation(position.extend(-0.5)),
    )
}

/// A grid of landmarks, spaced by `LANDMARK_SPACING`, covering the level.
fn landmark_grid() -> impl Bundle {
    let columns = (LEVEL_SIZE.x / LANDMARK_SPACING) as i32;
    let rows = (LEVEL_SIZE.y / LANDMARK_SPACING) as i32;
    let grid_size = Vec2::new(columns as f32, rows as f32) * LANDMARK_SPACING;

    let landmarks = (0..columns)
        .flat_map(move |x| (0..rows).map(move |y| (x, y)))
        .map(move |(x, y)| {
            let cell = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
            let position = cell * LANDMARK_SPACING - grid_size / 2.0;
            let hue = ((x * rows + y) as f32 * 37.0) % 360.0;
            landmark(position, Color::hsl(hue, 0.6, 0.55))
        });

    (
        Name::new("Landmarks"),
        Transform::default(),
        Visibility::default(),
        Children::spawn(SpawnIter(landmarks)),
    )
}

#[derive(Resource, Asset, Clone, Reflect)]
#[reflect(Resource)]
pub struct LevelAssets {
    #[dependency]
    music: Handle<AudioSource>,
}

impl FromWorld for LevelAssets {
    fn from_world(world: &mut World) -> Self {
        let assets = world.resource::<AssetServer>();
        Self {
            music: assets.load("audio/music/Fluffing A Duck.ogg"),
        }
    }
}

/// A system that spawns the main level.
pub fn spawn_level(
    mut commands: Commands,
    level_assets: Res<LevelAssets>,
    player_assets: Res<PlayerAssets>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    commands.spawn((
        Name::new("Level"),
        Transform::default(),
        Visibility::default(),
        LevelBounds(Rect::from_center_size(Vec2::ZERO, LEVEL_SIZE)),
        DespawnOnExit(Screen::Gameplay),
        children![
            player(&player_assets, &mut texture_atlas_layouts),
            (
                Name::new("Gameplay Music"),
                music(level_assets.music.clone())
            ),
            background(),
            landmark_grid(),
        ],
    ));
}
