//! 3dskinlayers for Horizon (behavioral clean-room port of tr7zw's 3D Skin Layers).
//!
//! Upstream renders the skin's second layer as an extruded 3D shell instead of a
//! flat billboard, with animation and per-part toggles. The render pass itself is
//! a host seam (see README); this crate carries the geometry and policy logic:
//! per-part config gating, the skin-overlay pixel volume model (which faces
//! exist, greedy rectangle merging of opaque pixel runs), and animation state.

pub mod layers;

use layers::PartKind;

/// Per-part visibility and animation config, mirroring upstream's toggle set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub enabled: bool,
    pub head: bool,
    pub body: bool,
    pub left_arm: bool,
    pub right_arm: bool,
    pub left_leg: bool,
    pub right_leg: bool,
    /// First-person: upstream hides the second layer on the player's own head
    /// by default so it does not fill the screen.
    pub first_person_visible: bool,
    /// Wobble/breathing animation of the extruded shell.
    pub animations: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            head: true,
            body: true,
            left_arm: true,
            right_arm: true,
            left_leg: true,
            right_leg: true,
            first_person_visible: false,
            animations: true,
        }
    }
}

impl Config {
    fn part_enabled(&self, kind: PartKind) -> bool {
        if !self.enabled {
            return false;
        }
        match kind {
            PartKind::Head => self.head,
            PartKind::Body => self.body,
            PartKind::LeftArm => self.left_arm,
            PartKind::RightArm => self.right_arm,
            PartKind::LeftLeg => self.left_leg,
            PartKind::RightLeg => self.right_leg,
        }
    }
}

/// Per-player runtime state: resolved geometry plus animation phase.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PlayerLayers {
    /// One cuboid set per rendered part.
    pub parts: Vec<(PartKind, Vec<layers::Cuboid>)>,
    /// Animation phase in ticks, advancing only when animations are on.
    pub anim_ticks: u32,
}

impl PlayerLayers {
    /// Rebuild the extruded shell for one player from their skin overlay.
    pub fn rebuild(cfg: &Config, skin: &layers::Skin) -> Self {
        let mut parts = Vec::new();
        for spec in layers::PARTS.iter() {
            if !cfg.part_enabled(spec.kind) {
                continue;
            }
            let cuboids = layers::build_part(skin, spec);
            if !cuboids.is_empty() {
                parts.push((spec.kind, cuboids));
            }
        }
        Self { parts, anim_ticks: 0 }
    }

    /// Advance animation by `ticks`; a no-op when animations are off.
    pub fn tick(&mut self, cfg: &Config, ticks: u32) {
        if cfg.animations {
            self.anim_ticks = self.anim_ticks.wrapping_add(ticks);
        }
    }

    /// First-person policy: the worn shell on your own head is hidden unless
    /// explicitly enabled, matching upstream's default.
    pub fn visible_in_first_person(&self, cfg: &Config) -> bool {
        cfg.first_person_visible
    }

    /// Total quads the renderer must draw (budget proxy for the seam).
    pub fn quad_count(&self) -> usize {
        self.parts.iter().map(|(_, c)| c.iter().map(|q| q.quads()).sum::<usize>()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use layers::{Skin, PARTS};

    #[test]
    fn default_config_matches_upstream_defaults() {
        let cfg = Config::default();
        assert!(cfg.enabled && cfg.head && cfg.body);
        assert!(!cfg.first_person_visible);
        assert!(cfg.animations);
    }

    #[test]
    fn master_switch_disables_everything() {
        let cfg = Config { enabled: false, ..Config::default() };
        let skin = Skin::opaque_all();
        let out = PlayerLayers::rebuild(&cfg, &skin);
        assert!(out.parts.is_empty());
    }

    #[test]
    fn per_part_toggle_gates_rebuild() {
        let cfg = Config { body: false, ..Config::default() };
        let skin = Skin::opaque_all();
        let out = PlayerLayers::rebuild(&cfg, &skin);
        assert!(out.parts.iter().all(|(k, _)| *k != PartKind::Body));
        assert!(out.parts.iter().any(|(k, _)| *k == PartKind::Head));
    }

    #[test]
    fn empty_overlay_yields_no_parts() {
        let skin = Skin::transparent_all();
        let out = PlayerLayers::rebuild(&Config::default(), &skin);
        assert!(out.parts.is_empty());
    }

    #[test]
    fn animation_gated_by_config() {
        let mut on = PlayerLayers::default();
        let cfg = Config::default();
        on.tick(&cfg, 7);
        assert_eq!(on.anim_ticks, 7);
        let mut off = PlayerLayers::default();
        let cfg_off = Config { animations: false, ..Config::default() };
        off.tick(&cfg_off, 7);
        assert_eq!(off.anim_ticks, 0);
    }

    #[test]
    fn first_person_default_hidden() {
        let cfg = Config::default();
        let out = PlayerLayers::default();
        assert!(!out.visible_in_first_person(&cfg));
        let cfg_v = Config { first_person_visible: true, ..Config::default() };
        assert!(out.visible_in_first_person(&cfg_v));
    }

    #[test]
    fn every_part_spec_present_once() {
        assert_eq!(PARTS.len(), 6);
        let kinds: Vec<_> = PARTS.iter().map(|p| p.kind).collect();
        for (i, a) in kinds.iter().enumerate() {
            assert!(kinds[i + 1..].iter().all(|b| b != a), "duplicate part {a:?}");
        }
    }

    #[test]
    fn full_skin_produces_geometry_for_all_parts() {
        let skin = Skin::opaque_all();
        let out = PlayerLayers::rebuild(&Config::default(), &skin);
        assert_eq!(out.parts.len(), 6);
        assert!(out.quad_count() > 0);
    }
}
