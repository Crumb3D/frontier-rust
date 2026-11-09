//! Hierarchical reference frames and origin rebasing.

use glam::{DMat3, DVec3};
use std::collections::HashMap;

/// How a body's world position is computed (replaces Python callables).
#[derive(Clone, Debug)]
pub enum WorldPos {
    Fixed(DVec3),
    /// Kepler orbit around the system star (barycentre at origin).
    Orbit {
        elements: crate::kepler::OrbitalElements,
    },
    /// Circular orbit around another body's current world position.
    CircleAround {
        parent_id: String,
        radius_km: f64,
        period_days: f64,
        phase0: f64,
    },
    /// Surface site: parent + spun lat/lon offset.
    Surface {
        parent_id: String,
        offset: DVec3,
        spin_period_days: f64,
    },
}

impl WorldPos {
    pub fn at(&self, days: f64, bodies: &HashMap<String, BodyRef>) -> DVec3 {
        match self {
            WorldPos::Fixed(v) => *v,
            WorldPos::Orbit { elements } => crate::kepler::position_at(elements, days),
            WorldPos::CircleAround {
                parent_id,
                radius_km,
                period_days,
                phase0,
            } => {
                let parent = bodies
                    .get(parent_id)
                    .map(|b| b.world_pos_at(days, bodies))
                    .unwrap_or(DVec3::ZERO);
                let ang = phase0 + 2.0 * std::f64::consts::PI * days / period_days.max(1e-6);
                parent + DVec3::new(ang.cos() * radius_km, 0.0, ang.sin() * radius_km)
            }
            WorldPos::Surface {
                parent_id,
                offset,
                spin_period_days,
            } => {
                let parent = bodies
                    .get(parent_id)
                    .map(|b| b.world_pos_at(days, bodies))
                    .unwrap_or(DVec3::ZERO);
                parent + spin_about_y(days, *spin_period_days) * *offset
            }
        }
    }
}

pub fn spin_about_y(days: f64, period_days: f64) -> DMat3 {
    if period_days <= 1e-6 {
        return DMat3::IDENTITY;
    }
    let a = 2.0 * std::f64::consts::PI * days / period_days;
    let (c, s) = (a.cos(), a.sin());
    DMat3::from_cols(
        DVec3::new(c, 0.0, -s),
        DVec3::new(0.0, 1.0, 0.0),
        DVec3::new(s, 0.0, c),
    )
}

/// A body that can be a coordinate parent.
#[derive(Clone, Debug)]
pub struct BodyRef {
    pub id: String,
    pub name: String,
    pub radius_km: f64,
    pub world_pos: WorldPos,
    pub parent_id: Option<String>,
    pub is_station: bool,
    pub is_starport: bool,
    pub atmosphere_km: f64,
    pub spin_period_days: f64,
    pub spin_phase: f64,
}

impl BodyRef {
    pub fn world_pos_at(&self, days: f64, bodies: &HashMap<String, BodyRef>) -> DVec3 {
        self.world_pos.at(days, bodies)
    }
}

/// Ship/camera state relative to a parent body.
#[derive(Clone, Debug)]
pub struct FrameState {
    pub parent_id: String,
    pub position: DVec3,
    pub velocity: DVec3,
    pub orientation: DMat3,
    pub angular_velocity: DVec3,
}

impl FrameState {
    pub fn new(parent_id: impl Into<String>) -> Self {
        Self {
            parent_id: parent_id.into(),
            position: DVec3::ZERO,
            velocity: DVec3::ZERO,
            orientation: DMat3::IDENTITY,
            angular_velocity: DVec3::ZERO,
        }
    }
}

/// Tracks bodies and rebases ship frames when leaving a parent sphere.
#[derive(Clone, Debug, Default)]
pub struct FrameSystem {
    pub bodies: HashMap<String, BodyRef>,
}

impl FrameSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, body: BodyRef) {
        self.bodies.insert(body.id.clone(), body);
    }

    pub fn world_position(&self, body_id: &str, days: f64) -> DVec3 {
        self.bodies
            .get(body_id)
            .map(|b| b.world_pos_at(days, &self.bodies))
            .unwrap_or(DVec3::ZERO)
    }

    pub fn ship_world_pos(&self, state: &FrameState, days: f64) -> DVec3 {
        self.world_position(&state.parent_id, days) + state.position
    }

    pub fn ship_world_vel(&self, state: &FrameState, days: f64) -> DVec3 {
        let p0 = self.world_position(&state.parent_id, days);
        let p1 = self.world_position(&state.parent_id, days + 1e-6);
        let parent_vel = (p1 - p0) / (1e-6 * 86400.0);
        parent_vel + state.velocity
    }

    pub fn rebase(&self, state: &FrameState, days: f64, new_parent_id: &str) -> FrameState {
        if new_parent_id == state.parent_id {
            return state.clone();
        }
        let world_pos = self.ship_world_pos(state, days);
        let world_vel = self.ship_world_vel(state, days);
        let new_parent_pos = self.world_position(new_parent_id, days);
        let p0 = new_parent_pos;
        let p1 = self.world_position(new_parent_id, days + 1e-6);
        let parent_vel = (p1 - p0) / (1e-6 * 86400.0);
        FrameState {
            parent_id: new_parent_id.to_string(),
            position: world_pos - new_parent_pos,
            velocity: world_vel - parent_vel,
            orientation: state.orientation,
            angular_velocity: state.angular_velocity,
        }
    }

    pub fn auto_rebase(&self, state: &FrameState, days: f64) -> FrameState {
        let world = self.ship_world_pos(state, days);
        let mut best_id = state.parent_id.clone();
        let mut best_score = f64::INFINITY;
        for (bid, body) in &self.bodies {
            let dist = (world - body.world_pos_at(days, &self.bodies)).length();
            let sphere = if body.is_station {
                (body.radius_km * 20.0).max(50.0)
            } else if body.is_starport {
                body.radius_km + body.atmosphere_km.max(100.0)
            } else {
                body.radius_km * 15.0
            };
            if dist < sphere {
                let mut score = dist / body.radius_km.max(1.0);
                if body.is_station {
                    score *= 0.1;
                }
                if score < best_score {
                    best_score = score;
                    best_id = bid.clone();
                }
            }
        }
        if best_id != state.parent_id {
            self.rebase(state, days, &best_id)
        } else {
            state.clone()
        }
    }

    pub fn altitude(&self, state: &FrameState, _days: f64) -> f64 {
        let body = &self.bodies[&state.parent_id];
        state.position.length() - body.radius_km
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rebase_preserves_world() {
        let mut frames = FrameSystem::new();
        frames.add(BodyRef {
            id: "star".into(),
            name: "Star".into(),
            radius_km: 1e5,
            world_pos: WorldPos::Fixed(DVec3::ZERO),
            parent_id: None,
            is_station: false,
            is_starport: false,
            atmosphere_km: 0.0,
            spin_period_days: 0.0,
            spin_phase: 0.0,
        });
        frames.add(BodyRef {
            id: "planet".into(),
            name: "P".into(),
            radius_km: 1000.0,
            world_pos: WorldPos::Fixed(DVec3::new(10000.0, 0.0, 0.0)),
            parent_id: Some("star".into()),
            is_station: false,
            is_starport: false,
            atmosphere_km: 0.0,
            spin_period_days: 0.0,
            spin_phase: 0.0,
        });
        let state = FrameState {
            parent_id: "star".into(),
            position: DVec3::new(10005.0, 1.0, 2.0),
            velocity: DVec3::new(0.1, 0.0, 0.0),
            orientation: DMat3::IDENTITY,
            angular_velocity: DVec3::ZERO,
        };
        let w0 = frames.ship_world_pos(&state, 0.0);
        let v0 = frames.ship_world_vel(&state, 0.0);
        let state2 = frames.rebase(&state, 0.0, "planet");
        let w1 = frames.ship_world_pos(&state2, 0.0);
        let v1 = frames.ship_world_vel(&state2, 0.0);
        assert!((w0 - w1).length() < 1e-6);
        assert!((v0 - v1).length() < 1e-6);
        assert_eq!(state2.parent_id, "planet");
    }
}
