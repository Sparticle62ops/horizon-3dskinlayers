//! The skin-overlay pixel volume model: which faces of each body part's second
//! layer exist, and how opaque pixel runs merge into the fewest quads.
//!
//! Behavioral clean-room work: the atlas layout (standard 64x64 skin), the
//! per-part box dimensions and the greedy rectangle merge are described from
//! behavior, with no upstream code or assets included.

/// Which body part a spec renders.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartKind {
    Head,
    Body,
    LeftArm,
    RightArm,
    LeftLeg,
    RightLeg,
}

/// A body part: its overlay region in the 64x64 atlas and its box dimensions
/// in skin pixels (w across, h tall, d deep).
#[derive(Debug, Clone, Copy)]
pub struct PartSpec {
    pub kind: PartKind,
    /// Atlas origin of the part's overlay region.
    pub u: u32,
    pub v: u32,
    /// Box dims: (w, h, d).
    pub w: u32,
    pub h: u32,
    pub d: u32,
}

/// Standard 64x64 skin overlay layout (second layer starts at 32x0).
pub const PARTS: [PartSpec; 6] = [
    PartSpec { kind: PartKind::Head, u: 32, v: 0, w: 8, h: 8, d: 8 },
    PartSpec { kind: PartKind::Body, u: 16, v: 16, w: 8, h: 12, d: 4 },
    PartSpec { kind: PartKind::RightArm, u: 40, v: 16, w: 4, h: 12, d: 4 },
    PartSpec { kind: PartKind::LeftArm, u: 32, v: 48, w: 4, h: 12, d: 4 },
    PartSpec { kind: PartKind::RightLeg, u: 0, v: 16, w: 4, h: 12, d: 4 },
    PartSpec { kind: PartKind::LeftLeg, u: 0, v: 48, w: 4, h: 12, d: 4 },
];

/// The six shell faces of one box, in atlas order: top, bottom, right, front,
/// left, back. Each entry is (face offset from the part origin, face pixel size).
fn face_rects(spec: &PartSpec) -> [(u32, u32, u32, u32); 6] {
    let (w, h, d) = (spec.w, spec.h, spec.d);
    let (u, v) = (spec.u, spec.v);
    [
        (u + d, v, w, d),         // top
        (u + d + w, v, w, d),     // bottom
        (u, v + d, d, h),         // right
        (u + d, v + d, w, h),     // front
        (u + d + w, v + d, d, h), // left
        (u + d + w + d, v + d, w, h), // back
    ]
}

/// A merged opaque rectangle on one face: position in face pixels, size, and
/// where it samples from in the atlas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Quad {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub u: u32,
    pub v: u32,
}

impl Quad {
    /// One merged rectangle draws one quad.
    pub fn quads(&self) -> usize {
        1
    }
}

/// A skin atlas: 64x64 RGBA, alpha > 0 means the overlay pixel is drawn.
pub struct Skin {
    /// (x, y, alpha) for every overlay pixel with alpha > 0.
    opaque: Vec<(u32, u32)>,
}

impl Skin {
    pub fn from_alphas(alphas: &[[u8; 64]; 64]) -> Self {
        let mut opaque = Vec::new();
        for y in 0..64u32 {
            for x in 0..64u32 {
                if alphas[y as usize][x as usize] > 0 {
                    opaque.push((x, y));
                }
            }
        }
        opaque.sort();
        Skin { opaque }
    }

    pub fn opaque_all() -> Self {
        let mut a = [[0u8; 64]; 64];
        for row in a.iter_mut() {
            for px in row.iter_mut() {
                *px = 255;
            }
        }
        Self::from_alphas(&a)
    }

    pub fn transparent_all() -> Self {
        Self::from_alphas(&[[0u8; 64]; 64])
    }

    fn opaque(&self, x: u32, y: u32) -> bool {
        self.opaque.binary_search(&(x, y)).is_ok()
    }
}

/// One body part's extruded shell: the merged quads for each face that has any
/// opaque pixels.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Cuboid {
    pub faces: Vec<Face>,
}

/// One face of the shell with its merged quads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Face {
    /// Which of the six faces (0=top .. 5=back, atlas order).
    pub index: u8,
    pub quads: Vec<Quad>,
}

impl Cuboid {
    pub fn quads(&self) -> usize {
        self.faces.iter().map(|f| f.quads.len()).sum()
    }
}

/// Greedy rectangle merge over one face's opaque pixels: maximal horizontal
/// runs, then runs with the same start and length merge vertically. This is
/// the standard pixel-shell packing behavior: adjacent opaque pixels become
/// one quad, so a full face is a single quad instead of per-pixel geometry.
fn merge_face(skin: &Skin, u: u32, v: u32, w: u32, h: u32) -> Vec<Quad> {
    // Horizontal runs per row.
    let mut runs: Vec<Vec<(u32, u32)>> = Vec::with_capacity(h as usize);
    for row in 0..h {
        let y = v + row;
        let mut line: Vec<(u32, u32)> = Vec::new();
        let mut col = 0u32;
        while col < w {
            if skin.opaque(u + col, y) {
                let start = col;
                while col < w && skin.opaque(u + col, y) {
                    col += 1;
                }
                line.push((start, col - start));
            } else {
                col += 1;
            }
        }
        runs.push(line);
    }
    // Merge vertically: runs that repeat with the same start/length across
    // consecutive rows become one rectangle spanning the rows.
    let mut quads: Vec<Quad> = Vec::new();
    let mut row = 0usize;
    while row < runs.len() {
        for &(start, len) in &runs[row] {
            // A run identical to one in the row above was already merged into
            // the quad started there; pushing it again would double-draw.
            if row > 0 && runs[row - 1].contains(&(start, len)) {
                continue;
            }
            let mut span = 1usize;
            while row + span < runs.len()
                && runs[row + span].contains(&(start, len))
            {
                span += 1;
            }
            quads.push(Quad { x: start, y: row as u32, w: len, h: span as u32, u: u + start, v: v + row as u32 });
        }
        row += 1;
    }
    quads
}

/// Build the shell for one part: every face with at least one opaque overlay
/// pixel, quads merged greedily. A part with a fully transparent overlay has
/// no geometry at all.
pub fn build_part(skin: &Skin, spec: &PartSpec) -> Vec<Cuboid> {
    let mut faces = Vec::new();
    for (index, (fu, fv, fw, fh)) in face_rects(spec).into_iter().enumerate() {
        let quads = merge_face(skin, fu, fv, fw, fh);
        if !quads.is_empty() {
            faces.push(Face { index: index as u8, quads });
        }
    }
    if faces.is_empty() {
        Vec::new()
    } else {
        vec![Cuboid { faces }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skin_region_opaque(u: u32, v: u32, w: u32, h: u32) -> Skin {
        let mut a = [[0u8; 64]; 64];
        for y in v..v + h {
            for x in u..u + w {
                a[y as usize][x as usize] = 255;
            }
        }
        Skin::from_alphas(&a)
    }

    #[test]
    fn full_face_is_one_quad() {
        let skin = skin_region_opaque(40, 8, 8, 8); // head front face
        let spec = &PARTS[0];
        let cub = build_part(&skin, spec);
        assert_eq!(cub.len(), 1);
        let front = cub[0].faces.iter().find(|f| f.index == 3).unwrap();
        assert_eq!(front.quads.len(), 1);
        let q = front.quads[0];
        assert_eq!((q.x, q.y, q.w, q.h), (0, 0, 8, 8));
        assert_eq!((q.u, q.v), (40, 8));
    }

    #[test]
    fn horizontal_runs_merge() {
        let mut a = [[0u8; 64]; 64];
        for x in 36..40 {
            a[8][x as usize] = 255;
            a[9][x as usize] = 255;
        }
        let skin = Skin::from_alphas(&a);
        let quads = merge_face(&skin, 36, 8, 8, 8);
        assert_eq!(quads.len(), 1);
        assert_eq!((quads[0].w, quads[0].h), (4, 2));
    }

    #[test]
    fn separate_rows_of_different_widths_do_not_merge() {
        let mut a = [[0u8; 64]; 64];
        for x in 36..40 {
            a[8][x as usize] = 255;
        }
        for x in 36..38 {
            a[9][x as usize] = 255;
        }
        let skin = Skin::from_alphas(&a);
        let quads = merge_face(&skin, 36, 8, 8, 8);
        assert_eq!(quads.len(), 2);
    }

    #[test]
    fn empty_face_is_absent() {
        let skin = skin_region_opaque(40, 8, 8, 8); // only head front
        let spec = &PARTS[0];
        let cub = build_part(&skin, spec);
        assert_eq!(cub.len(), 1);
        assert_eq!(cub[0].faces.len(), 1);
        assert_eq!(cub[0].faces[0].index, 3);
    }

    #[test]
    fn transparent_part_has_no_geometry() {
        let skin = Skin::transparent_all();
        for spec in PARTS.iter() {
            assert!(build_part(&skin, spec).is_empty(), "{:?}", spec.kind);
        }
    }

    #[test]
    fn opaque_all_merges_to_one_quad_per_face() {
        let skin = Skin::opaque_all();
        let cub = build_part(&skin, &PARTS[1]); // body 8x12x4
        assert_eq!(cub.len(), 1);
        assert_eq!(cub[0].faces.len(), 6);
        for f in &cub[0].faces {
            assert_eq!(f.quads.len(), 1, "face {} not fully merged", f.index);
        }
    }

    #[test]
    fn face_rects_match_atlas_layout() {
        // Head overlay at (32,0), box 8x8x8: front face lands at (40,8).
        let rects = face_rects(&PARTS[0]);
        assert_eq!(rects[3], (40, 8, 8, 8));
        // Body overlay at (16,16), box 8x12x4: front at (20,20) 8x12.
        let rects = face_rects(&PARTS[1]);
        assert_eq!(rects[3], (20, 20, 8, 12));
    }

    #[test]
    fn quad_count_counts_merged_quads() {
        let skin = Skin::opaque_all();
        let out = super::super::PlayerLayers::rebuild(
            &super::super::Config::default(),
            &skin,
        );
        // 6 parts, each 6 faces, one merged quad each = 36 quads.
        assert_eq!(out.quad_count(), 36);
    }
}
