#![allow(unused)]

use bevy::math::Vec3Swizzles;
use bevy::prelude::*;
use bevy::sprite::collide_aabb::collide;
use player::PlayerPlugin;
use enemy::EnemyPlugin;
use bevy::window::PrimaryWindow;
use std::collections::HashSet;

use crate::components::{
	Movable, Velocity, FromPlayer, 
  FromEnemy, Laser, Player, SpriteSize,
  Enemy, Explosion, ExplosionTimer, ExplosionToSpawn
};

mod components;
mod player;
mod enemy;

const PLAYER_SPRITE: &str = "player_a_01.png";
const PLAYER_SIZE: (f32, f32) = (144., 75.);
const PLAYER_LASER_SPRITE: &str = "laser_a_01.png";
const PLAYER_LASER_SIZE: (f32, f32) = (9., 54.);
const SPRITE_SCALE: f32 = 0.5;
const TIME_STEP: f32 = 1. / 60.;
const BASE_SPEED: f32 = 500.;
const ENEMY_SPRITE: &str = "enemy_a_01.png";
const ENEMY_SIZE: (f32, f32) = (144., 75.);
const ENEMY_LASER_SPRITE:  &str = "laser_b_01.png";
const ENEMY_LASER_SIZE: (f32, f32) = (17., 55.); 
const EXPLOSION_SHEET: &str ="explo_a_sheet.png";
const EXPLOSION_LEN: usize = 16;
const ENEMY_MAX: u32 = 2;
const PLAYER_RESPAWN_DELAY: f64 = 2.;
const FORMATION_MEMBERS_MAX: u32 = 2;

#[derive(Resource)]
pub struct WinSize {
  pub w: f32,
  pub h: f32
}

#[derive(Resource)]
struct GameTextures {
  player: Handle<Image>,
  player_laser: Handle<Image>,
  enemy: Handle<Image>,
  enemy_laser: Handle<Image>,
  explosion: Handle<TextureAtlas>,
}

#[derive(Resource)]
struct EnemyCount(u32);

#[derive(Resource)]
struct PlayerState {
  on: bool, // alive
  last_shot: f64,
} 

impl Default for PlayerState {
  fn default() -> Self {
    Self {
      on: false,
      last_shot: -1.,
    }
  }
}

impl PlayerState {
  pub fn shot(&mut self, time: f64) {
    self.on = false;
    self.last_shot = time;
  }
  
  pub fn spawned(&mut self) {
    self.on = true;
    self.last_shot = -1.;
  }
}

fn main() {
  let window_position = IVec2::new(2780 as i32, 0 as i32);

  App::new()
    .insert_resource(ClearColor(Color::rgb(0.04, 0.04, 0.04)))
    .add_plugins(DefaultPlugins.set(WindowPlugin {
      primary_window: Some(Window {
          title: "Rust Invaders".to_string(),
          resolution: (598.0, 676.0).into(),
          position: WindowPosition::new(window_position),
          ..default()
      }),
      ..default()
    }))
    .add_plugins(PlayerPlugin)
    .add_plugins(EnemyPlugin)
    .add_systems(Startup, setup_systems)
    .add_systems(Update, movable_system)
    .add_systems(Update, player_laser_hit_enemy_system)
    .add_systems(Update, enemy_laser_hit_player_system)
    .add_systems(Update, explosion_to_spawn_system)
    .add_systems(Update, explosion_animation_system)
    .run();
}

fn setup_systems(
  mut commands: Commands, 
  asset_server: Res<AssetServer>,
  mut texture_atlases: ResMut<Assets<TextureAtlas>>,
	query: Query<&Window, With<PrimaryWindow>>,
) {
  // camera
  commands.spawn(Camera2dBundle::default());

  // capture window size
  let Ok(primary) = query.get_single() else {
		return;
	};
	let (win_w, win_h) = (primary.width(), primary.height());

  // add window size
  let win_size = WinSize { w: win_w, h: win_h };
  commands.insert_resource(win_size);

  // create explosion texture
  let texture_handle = asset_server.load(EXPLOSION_SHEET);
  let texture_atlas = TextureAtlas::from_grid(texture_handle, Vec2::new(64., 64.), 4, 4, None, None);
  let explosion = texture_atlases.add(texture_atlas);

  // add game textures resource
  let game_textures = GameTextures {
    player: asset_server.load(PLAYER_SPRITE),
    player_laser: asset_server.load(PLAYER_LASER_SPRITE),
    enemy: asset_server.load(ENEMY_SPRITE),
    enemy_laser: asset_server.load(ENEMY_LASER_SPRITE),
    explosion,
  };
  commands.insert_resource(game_textures);
  commands.insert_resource(EnemyCount(0));
}

fn movable_system(
  mut commands: Commands,
  win_size: Res<WinSize>,
  mut query: Query<(Entity, &Velocity, &mut Transform, &Movable)>
) {
  for (entity, velocity, mut transform, movable) in query.iter_mut() {
    let translation = &mut transform.translation;
    translation.x += velocity.x * TIME_STEP * BASE_SPEED;
    translation.y += velocity.y * TIME_STEP * BASE_SPEED;

    if movable.auto_despawn {
      //despawn when out of screen

      const MARGIN: f32 = 200.;
      if translation.y > win_size.h / 2. + MARGIN 
        || translation.y < -win_size.h / 2. - MARGIN 
        || translation.x > win_size.w / 2. + MARGIN
        || translation.x < -win_size.w / 2. - MARGIN
        {
          commands.entity(entity).despawn();
        }
    }
  }
}

fn player_laser_hit_enemy_system(
  mut commands: Commands,
  mut enemy_count: ResMut<EnemyCount>,
  laser_query: Query<(Entity, &Transform, &SpriteSize), (With<Laser>, With<FromPlayer>)>,
  enemy_query: Query<(Entity, &Transform, &SpriteSize), With<Enemy>>
) {
  // iterate trought lasers:
  let mut despawned_entities: HashSet<Entity> = HashSet::new();

  for (laser_entity, laser_tf, laser_size) in laser_query.iter() {
    if despawned_entities.contains(&laser_entity) {
      continue;
    }
    
    let laser_scale = Vec2::from(laser_tf.scale.xy());

    // enemies
    for (enemy_entity, enemy_tf, enemy_size) in enemy_query.iter() {
      if despawned_entities.contains(&enemy_entity) ||
        despawned_entities.contains(&laser_entity) {
          continue;
      }

      let enemy_scale = Vec2::from(enemy_tf.scale.xy());

      //determin if collision
      let collision = collide(
        laser_tf.translation,
        laser_size.0 * laser_scale,
        enemy_tf.translation,
        enemy_size.0 * enemy_scale,
      );

      // perf collision
      if collision.is_some() {
        println!("its a shot!");

        //remove laser
        commands.entity(laser_entity).despawn();
        despawned_entities.insert(laser_entity);
        enemy_count.0 -= 1;
        // remove enemy
        commands.entity(enemy_entity).despawn();
        despawned_entities.insert(enemy_entity);

        commands.spawn(ExplosionToSpawn(enemy_tf.translation));
      }
    }

  }
}

fn enemy_laser_hit_player_system(
  mut commands: Commands,
  mut player_state: ResMut<PlayerState>,
  time: Res<Time>,
  laser_query: Query<(Entity, &Transform, &SpriteSize), (With<Laser>, With<FromEnemy>)>,
  player_query: Query<(Entity, &Transform, &SpriteSize), With<Player>>,
) {
  if let Ok((player_entity, player_tf, player_size)) = player_query.get_single() {
    let player_scale = Vec2::from(player_tf.scale.xy());

    for (laser_entity, laser_tf, laser_size) in laser_query.iter() {
      let laser_scale = Vec2::from(laser_tf.scale.xy());

      // determine if collision exists
      let collision = collide(
        laser_tf.translation,
        laser_size.0 * laser_scale,
        player_tf.translation,
        player_size.0 * player_scale,
      );

      // perform the collision
      if let Some(_) = collision {
        // remove the player
        commands.entity(player_entity).despawn();
        player_state.shot(time.elapsed_seconds_f64());

        // remove the laser
        commands.entity(laser_entity).despawn();

        // spawn explosion
        commands.spawn(ExplosionToSpawn(player_tf.translation));

        break;
      }
    }
  }
}
  
fn explosion_to_spawn_system(
  mut commands: Commands,
  game_textures: Res<GameTextures>,
  query: Query<(Entity, &ExplosionToSpawn)>,
) {
  for (explosion_spawn_entity, explosion_to_spawn) in query.iter() {
    // spawn the explosion sprite
    commands.spawn(SpriteSheetBundle {
      texture_atlas: game_textures.explosion.clone(),
      transform: Transform {
        translation: explosion_to_spawn.0,
        ..Default::default()
      },
      ..Default::default()
    })
    .insert(Explosion)
    .insert(ExplosionTimer::default());

    // despawn explosion
    commands.entity(explosion_spawn_entity).despawn();
    
  }
}

fn explosion_animation_system(
  mut commands: Commands,
  time: Res<Time>,
  mut query: Query<(Entity, &mut ExplosionTimer, &mut TextureAtlasSprite), With<Explosion>>
) {
  for (entity, mut timer, mut sprite) in query.iter_mut() {
    timer.0.tick(time.delta());

    if timer.0.finished() {
      sprite.index += 1; //move to next sprite cell
      if sprite.index >= EXPLOSION_LEN {
        commands.entity(entity).despawn();
      }
    }
  }

}