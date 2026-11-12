//! See `docs/rfcs/0002.md` for details.

use bevy::prelude::*;
use rand::prelude::*;
use std::time::Duration;

use crate::{
    AppSystems, PausableSystems,
    audio::sound_effect,
    game::{movement::MovementController, player::PlayerAssets},
};

pub(super) fn plugin(app: &mut App) {
    // Animate and play sound effects based on controls.
    app.add_systems(
        Update,
        (
            update_animation_timer.in_set(AppSystems::TickTimers),
            (
                update_animation,
                update_animation_atlas,
                trigger_step_sound_effect,
            )
                .chain()
                .in_set(AppSystems::Update),
        )
            .in_set(PausableSystems),
    );
}

fn update_animation_timer(time: Res<Time>, mut query: Query<&mut PlayerAnimationState>) {
    for mut animation in &mut query {
        animation.advance(time.delta());
    }
}

/// Pick which animation to play subject to how the player moving.
fn update_animation(
    mut player_query: Query<(&MovementController, &mut Sprite, &mut PlayerAnimationState)>,
) {
    for (controller, mut sprite, mut animation_state) in &mut player_query {
        let dx = controller.intent.x;
        if dx != 0.0 {
            sprite.flip_x = dx < 0.0;
        }

        let animation = if controller.intent == Vec2::ZERO {
            PlayerAnimations::Idling
        } else {
            PlayerAnimations::Walking
        };
        animation_state.change_animation(animation);
    }
}

/// If the frame has changed, update the atlas index so the new frame is drawn to the screen.
fn update_animation_atlas(mut query: Query<(&PlayerAnimationState, &mut Sprite)>) {
    for (animation, mut sprite) in &mut query {
        let Some(atlas) = sprite.texture_atlas.as_mut() else {
            continue;
        };
        if animation.changed() {
            atlas.index = animation.get_atlas_index();
        }
    }
}

/// If the player is moving, play a step sound effect synchronized with the animation.
fn trigger_step_sound_effect(
    mut commands: Commands,
    player_assets: If<Res<PlayerAssets>>,
    mut step_query: Query<&PlayerAnimationState>,
) {
    for animation_state in &mut step_query {
        if animation_state.animation == PlayerAnimations::Walking
            && animation_state.changed()
            && (animation_state.frame == 0 || animation_state.frame == 5)
        {
            let rng = &mut rand::rng();
            let random_step = player_assets.steps.choose(rng).unwrap().clone();
            commands.spawn(sound_effect(random_step));
        }
    }
}

/// Component that tracks player's animation state.
/// It is tightly bound to the texture atlas we use.
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct PlayerAnimationState {
    /// Tracks the amount of time a frame has been on screen.
    timer: Timer,
    /// Which frame of the animation to draw to the screen.
    frame: usize,
    /// The animation that is playing.
    animation: PlayerAnimations,
}

#[derive(Reflect, PartialEq)]
pub enum PlayerAnimations {
    Idling,
    Walking,
}

impl PlayerAnimationState {
    /// Number of columns in the sprite sheet (must match `from_grid` in player.rs).
    const COLUMNS: usize = 8;
    const IDEL_ROW: usize = 0;
    const WALKING_ROW: usize = 1;

    const IDLE_FRAMES: usize = 1;
    /// The duration of each idle frame.
    const IDLE_INTERVAL: Duration = Duration::from_millis(0);

    const WALKING_FRAMES: usize = 8;
    const WALKING_INTERVAL: Duration = Duration::from_millis(83);

    fn idling() -> Self {
        Self {
            timer: Timer::new(Self::IDLE_INTERVAL, TimerMode::Once),
            frame: 0,
            animation: PlayerAnimations::Idling,
        }
    }

    fn walking() -> Self {
        Self {
            timer: Timer::new(Self::WALKING_INTERVAL, TimerMode::Repeating),
            frame: 0,
            animation: PlayerAnimations::Walking,
        }
    }

    pub fn new() -> Self {
        Self::idling()
    }

    /// Add the time elapsed since the last update, then move to the next frame if the timer finished.
    pub fn advance(&mut self, delta: Duration) {
        self.timer.tick(delta);
        if !self.timer.just_finished() {
            return;
        }
        self.frame = (self.frame + 1)
            % match self.animation {
                PlayerAnimations::Idling => Self::IDLE_FRAMES,
                PlayerAnimations::Walking => Self::WALKING_FRAMES,
            };
    }

    /// Update animation state if it changes.
    pub fn change_animation(&mut self, animation: PlayerAnimations) {
        if self.animation != animation {
            match animation {
                PlayerAnimations::Idling => *self = Self::idling(),
                PlayerAnimations::Walking => *self = Self::walking(),
            }
        }
    }

    /// Whether animation changed this tick.
    pub fn changed(&self) -> bool {
        self.timer.just_finished()
    }

    /// To get the atlas index, we add the animation’s offset, the number of frames that come before it on the sheet, to the current frame.
    pub fn get_atlas_index(&self) -> usize {
        match self.animation {
            PlayerAnimations::Idling => Self::IDEL_ROW * Self::COLUMNS,
            PlayerAnimations::Walking => Self::WALKING_ROW * Self::COLUMNS + self.frame,
        }
    }
}
