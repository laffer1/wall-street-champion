//! Procedural vector + pixel-art illustrations and icons for Wall Street Champion.
//! Gives the game colorful visual character (mascots, portraits, TV studio,
//! newspaper masthead, wire radar, skyline, and sector emblems) using pure GPU
//! vertex batches with zero external asset files.

use crate::sim::instruments::{Kind, Sector};
use crate::sim::market::Regime;
use crate::sim::news::Source;
use super::draw::Batch;

/// Render a multi-color ASCII pixel-art sprite with horizontal run-length merging.
/// Space `'.'` or `' '` is transparent.
pub fn draw_sprite(
    b: &mut Batch,
    x: f32,
    y: f32,
    px: f32,
    rows: &[&str],
    palette: &[(char, [f32; 4])],
    shadow: bool,
) {
    let lookup = |ch: char| -> Option<[f32; 4]> {
        if ch == '.' || ch == ' ' {
            return None;
        }
        palette.iter().find(|(c, _)| *c == ch).map(|(_, col)| *col)
    };

    if shadow {
        for (ry, row) in rows.iter().enumerate() {
            let chars: Vec<char> = row.chars().collect();
            let mut cx = 0;
            while cx < chars.len() {
                if lookup(chars[cx]).is_some() {
                    let start = cx;
                    while cx < chars.len() && lookup(chars[cx]).is_some() {
                        cx += 1;
                    }
                    b.rect(
                        x + start as f32 * px + px * 0.6,
                        y + ry as f32 * px + px * 0.6,
                        (cx - start) as f32 * px,
                        px,
                        [0.0, 0.0, 0.0, 0.45],
                    );
                } else {
                    cx += 1;
                }
            }
        }
    }

    for (ry, row) in rows.iter().enumerate() {
        let chars: Vec<char> = row.chars().collect();
        let mut cx = 0;
        while cx < chars.len() {
            let ch = chars[cx];
            if let Some(col) = lookup(ch) {
                let start = cx;
                while cx < chars.len() && chars[cx] == ch {
                    cx += 1;
                }
                b.rect(
                    x + start as f32 * px,
                    y + ry as f32 * px,
                    (cx - start) as f32 * px,
                    px,
                    col,
                );
            } else {
                cx += 1;
            }
        }
    }
}

/// Charging Wall Street Bull mascot (20x13 grid).
pub fn draw_bull(b: &mut Batch, x: f32, y: f32, px: f32) {
    const ROWS: &[&str] = &[
        "..............HH..HH",
        ".............H..HH..",
        "....BBBBBB..H..BBBB.",
        "..BBBBBBBBBB..BBBBBB",
        ".BddddBBBBBBBBBBBRBB",
        "BBdBBBBBBBBBBBBBBBBW",
        "BBBBBBBBBBBBBBBBGGG.",
        ".BBBBBBBBBBBBBB....S",
        "..BBB..BBB..BBB...SS",
        "..BB...BB....BB.....",
        "..BB...BB....BB.....",
        "..HH...HH....HH.....",
    ];
    let pal = [
        ('H', [0.98, 0.94, 0.78, 1.0]), // ivory horns/hooves
        ('B', [0.88, 0.62, 0.16, 1.0]), // golden bronze body
        ('d', [0.62, 0.40, 0.08, 1.0]), // dark bronze shading
        ('G', [0.98, 0.80, 0.28, 1.0]), // bright gold highlight
        ('R', [0.15, 0.95, 0.45, 1.0]), // glowing emerald eye
        ('W', [0.95, 0.75, 0.25, 1.0]), // snout ring
        ('S', [0.75, 0.95, 0.85, 0.75]), // snorting steam
    ];
    draw_sprite(b, x, y, px, ROWS, &pal, true);
}

/// Fierce Wall Street Bear mascot (20x12 grid).
pub fn draw_bear(b: &mut Batch, x: f32, y: f32, px: f32) {
    const ROWS: &[&str] = &[
        ".EE...EE............",
        "EpeE.EpeE...........",
        "RRRRRRRRR..RRRRRR...",
        "RRYRRRRRRRRRRRRRRR..",
        "RRRRRRRRRRRRddRRRRR.",
        "WWWWWRRRRRRddddRRRRR",
        ".WW..RRRRRRRRRRRRRRR",
        "......RRRRRRRRRRRRR.",
        ".....RRR..RRR...RRR.",
        ".....RR....RR....RR.",
        "....CC.....CC....CC.",
        "....................",
    ];
    let pal = [
        ('E', [0.78, 0.22, 0.20, 1.0]), // ears
        ('p', [0.95, 0.45, 0.42, 1.0]), // inner ear
        ('e', [0.55, 0.12, 0.12, 1.0]), // shade
        ('R', [0.85, 0.24, 0.22, 1.0]), // crimson fur
        ('d', [0.58, 0.14, 0.14, 1.0]), // dark fur shade
        ('Y', [1.00, 0.85, 0.20, 1.0]), // fierce yellow eye
        ('W', [0.98, 0.96, 0.92, 1.0]), // fangs
        ('C', [0.95, 0.88, 0.75, 1.0]), // claws
    ];
    draw_sprite(b, x, y, px, ROWS, &pal, true);
}

/// Golden Championship Trophy icon (12x12 grid).
pub fn draw_trophy(b: &mut Batch, x: f32, y: f32, px: f32) {
    const ROWS: &[&str] = &[
        "..GGGGGGGG..",
        ".HHHHHHHHHH.",
        "H.GGGGGGGG.H",
        "H.GWRRRRWG.H",
        ".HGggggggGH.",
        "..GGGGGGGG..",
        "...GGGGGG...",
        "....GGGG....",
        "....GGGG....",
        "...BBBBBB...",
        "..BBBBBBBB..",
        ".DDDDDDDDDD.",
    ];
    let pal = [
        ('G', [0.98, 0.78, 0.18, 1.0]), // gold
        ('H', [1.00, 0.92, 0.50, 1.0]), // bright highlight
        ('g', [0.78, 0.56, 0.10, 1.0]), // gold shadow
        ('R', [0.20, 0.90, 0.50, 1.0]), // emerald gem
        ('W', [1.00, 1.00, 0.90, 1.0]), // sparkle
        ('B', [0.55, 0.38, 0.14, 1.0]), // wood base
        ('D', [0.38, 0.25, 0.08, 1.0]), // dark base
    ];
    draw_sprite(b, x, y, px, ROWS, &pal, true);
}

/// Draw a stylized Manhattan / Wall Street skyline across a banner rect.
pub fn draw_skyline(b: &mut Batch, x: f32, y: f32, w: f32, h: f32, frame: u64) {
    // Building descriptors: (width, height_frac, has_spire)
    let buildings: &[(f32, f32, bool)] = &[
        (28.0, 0.55, false),
        (22.0, 0.78, true),
        (34.0, 0.62, false),
        (26.0, 0.88, true),
        (30.0, 0.50, false),
        (24.0, 0.72, false),
        (36.0, 0.66, true),
        (20.0, 0.48, false),
        (32.0, 0.82, true),
        (28.0, 0.58, false),
        (24.0, 0.74, false),
        (34.0, 0.90, true),
        (26.0, 0.54, false),
        (30.0, 0.68, false),
    ];
    let mut cx = x;
    let mut idx = 0usize;
    while cx < x + w {
        let (bw, hf, spire) = buildings[idx % buildings.len()];
        let bw = bw.min(x + w - cx);
        if bw < 6.0 {
            break;
        }
        let bh = (h * hf).round();
        let by = y + h - bh;
        let bcol_top = [0.09, 0.13, 0.22, 0.55];
        let bcol_bot = [0.04, 0.06, 0.11, 0.75];
        b.rect_grad_v(cx, by, bw - 2.0, bh, bcol_top, bcol_bot);
        b.rect(cx, by, bw - 2.0, 1.0, [0.22, 0.34, 0.55, 0.45]);

        if spire && bw >= 14.0 {
            let sx = cx + (bw - 2.0) * 0.5;
            b.rect(sx - 3.0, by - 5.0, 6.0, 5.0, bcol_top);
            b.rect(sx - 1.0, by - 11.0, 2.0, 6.0, [0.35, 0.50, 0.75, 0.55]);
            let blink = ((frame / 15 + idx as u64) % 2) == 0;
            let beacon = if blink { [1.0, 0.25, 0.30, 0.85] } else { [0.20, 0.85, 1.0, 0.65] };
            b.circle(sx, by - 12.0, 2.0, beacon);
        }

        // Tiny lit office windows
        let mut wy = by + 4.0;
        let mut row_i = 0usize;
        while wy + 3.0 < y + h - 2.0 {
            let mut wx = cx + 3.0;
            let mut col_i = 0usize;
            while wx + 3.0 < cx + bw - 4.0 {
                let hash = (idx * 31 + row_i * 17 + col_i * 13) % 7;
                if hash < 3 {
                    let wcol = if hash == 0 {
                        [0.98, 0.82, 0.32, 0.32]
                    } else {
                        [0.30, 0.75, 0.98, 0.24]
                    };
                    b.rect(wx, wy, 2.0, 2.0, wcol);
                }
                wx += 5.0;
                col_i += 1;
            }
            wy += 5.0;
            row_i += 1;
        }

        cx += bw;
        idx += 1;
    }
}

/// Draw a 34x34 framed character portrait for one of the 8 tipster archetypes.
pub fn draw_portrait(
    b: &mut Batch,
    x: f32,
    y: f32,
    size: f32,
    archetype: &str,
    trusted: bool,
    selected: bool,
) {
    let idx = match archetype {
        s if s.contains("bartender") => 0,
        s if s.contains("retired") => 1,
        s if s.contains("influencer") => 2,
        s if s.contains("cab") => 3,
        s if s.contains("analyst") => 4,
        s if s.contains("roommate") => 5,
        s if s.contains("gym") => 6,
        _ => 7, // crypto Discord moderator
    };

    // Archetype-specific background gradient
    let (bg_top, bg_bot) = match idx {
        0 => ([0.35, 0.18, 0.08, 1.0], [0.15, 0.07, 0.04, 1.0]), // warm amber bar
        1 => ([0.10, 0.28, 0.18, 1.0], [0.05, 0.12, 0.08, 1.0]), // floor green
        2 => ([0.42, 0.10, 0.38, 1.0], [0.14, 0.05, 0.24, 1.0]), // neon magenta/violet
        3 => ([0.38, 0.30, 0.06, 1.0], [0.14, 0.12, 0.04, 1.0]), // taxi gold
        4 => ([0.10, 0.24, 0.42, 1.0], [0.05, 0.10, 0.20, 1.0]), // corporate blue
        5 => ([0.24, 0.14, 0.38, 1.0], [0.10, 0.06, 0.18, 1.0]), // dorm purple
        6 => ([0.40, 0.12, 0.12, 1.0], [0.16, 0.05, 0.06, 1.0]), // gym crimson
        _ => ([0.06, 0.32, 0.32, 1.0], [0.03, 0.12, 0.16, 1.0]), // cyber teal
    };

    b.rect_grad_v(x, y, size, size, bg_top, bg_bot);
    b.glow(x + size * 0.5, y + size * 0.5, size * 0.45, [bg_top[0] * 1.5, bg_top[1] * 1.5, bg_top[2] * 1.5, 0.35]);

    // 12x12 character sprite inside the frame
    let px = ((size - 6.0) / 12.0).max(1.5);
    let sx = x + (size - 12.0 * px) * 0.5;
    let sy = y + size - 12.0 * px - 2.0;

    let (rows, pal): (&[&str], &[(_, [f32; 4])]) = match idx {
        // 0: Bartender (slick hair, mustache, red bowtie, cocktail shaker)
        0 => (
            &[
                "...HHHHHH...",
                "..HHHHHHHH..",
                "..HSSSSSSH..",
                "..SEESSEES..",
                "..SSSSSSSS..",
                "..SMMMMMMS..",
                "...SSSSSS...",
                ".WWWRRRRWWW.",
                "WWWWWRRWWWWW",
                "WWWWWWWWWWCC",
                "WWWWWWWWWWCC",
                "WWWWWWWWWW..",
            ],
            &[
                ('H', [0.18, 0.12, 0.08, 1.0]),
                ('S', [0.92, 0.74, 0.58, 1.0]),
                ('E', [0.12, 0.12, 0.16, 1.0]),
                ('M', [0.28, 0.16, 0.10, 1.0]),
                ('W', [0.94, 0.95, 0.98, 1.0]),
                ('R', [0.90, 0.18, 0.22, 1.0]),
                ('C', [0.95, 0.78, 0.22, 1.0]),
            ],
        ),
        // 1: Retired floor trader (silver hair, green visor, cigar + smoke)
        1 => (
            &[
                "...HHHHHH..K",
                "..HHHHHHHHK.",
                ".GGGGGGGGGG.",
                "..SEESSEES..",
                "..SSSSSSSS..",
                "..SSSSCCCCG.",
                "...SSSSSS...",
                ".JJJJWWJJJJ.",
                "JJJJJRRJJJJJ",
                "JJJJJRRJJJJJ",
                "JJJJJJJJJJJJ",
                "JJJJJJJJJJJJ",
            ],
            &[
                ('H', [0.82, 0.85, 0.90, 1.0]),
                ('G', [0.15, 0.85, 0.45, 1.0]),
                ('S', [0.88, 0.68, 0.54, 1.0]),
                ('E', [0.15, 0.15, 0.20, 1.0]),
                ('C', [0.52, 0.30, 0.12, 1.0]),
                ('K', [0.75, 0.80, 0.85, 0.8]),
                ('J', [0.18, 0.36, 0.62, 1.0]),
                ('W', [0.95, 0.95, 0.95, 1.0]),
                ('R', [0.85, 0.65, 0.15, 1.0]),
            ],
        ),
        // 2: Influencer (neon spiky hair, clout shades, gold chain)
        2 => (
            &[
                ".N.NN..NN.N.",
                "..NNNNNNNN..",
                ".NNNNNNNNNN.",
                "..CCCCCCCC..",
                "..CCWWWWCC..",
                "..SSSSSSSS..",
                "..SSWWWWSS..",
                "...SSSSSS...",
                ".PPPPGGPPPP.",
                "PPPPGGGGPPPP",
                "PPPPPPPPPPPP",
                "PPPPPPPPPPPP",
            ],
            &[
                ('N', [1.00, 0.25, 0.75, 1.0]),
                ('C', [0.15, 0.92, 1.00, 1.0]),
                ('W', [1.00, 1.00, 1.00, 1.0]),
                ('S', [0.90, 0.70, 0.55, 1.0]),
                ('P', [0.22, 0.12, 0.35, 1.0]),
                ('G', [1.00, 0.82, 0.20, 1.0]),
            ],
        ),
        // 3: Cab driver (yellow checker cap, stubble, leather jacket)
        3 => (
            &[
                "..YYYYYYYY..",
                ".YYYYYYYYYY.",
                ".YBYBYBYBYB.",
                ".DDDDDDDDDD.",
                "..SEESSEES..",
                "..SSSSSSSS..",
                "..SddddddS..",
                "...dddddd...",
                ".JJJJYYJJJJ.",
                "JJJJJYYJJJJJ",
                "JJJJJJJJJJJJ",
                "JJJJJJJJJJJJ",
            ],
            &[
                ('Y', [0.98, 0.80, 0.15, 1.0]),
                ('B', [0.12, 0.12, 0.15, 1.0]),
                ('D', [0.22, 0.22, 0.26, 1.0]),
                ('S', [0.85, 0.66, 0.50, 1.0]),
                ('d', [0.58, 0.44, 0.34, 1.0]),
                ('E', [0.12, 0.12, 0.16, 1.0]),
                ('J', [0.38, 0.24, 0.14, 1.0]),
            ],
        ),
        // 4: Junior analyst (glasses, navy suit, blue tie, coffee mug)
        4 => (
            &[
                "...HHHHHH...",
                "..HHHHHHHH..",
                "..HSSSSSSH..",
                ".GGWWSSWWGG.",
                ".GGWWSSWWGG.",
                "..SSSSSSSS..",
                "...SSSSSS...",
                ".NNNNWWNNNN.",
                "NNNNNTTNNNNM",
                "NNNNNTTNNNMM",
                "NNNNNTTNNNNM",
                "NNNNNNNNNNNN",
            ],
            &[
                ('H', [0.28, 0.18, 0.10, 1.0]),
                ('S', [0.94, 0.78, 0.64, 1.0]),
                ('G', [0.20, 0.65, 0.95, 1.0]),
                ('W', [0.96, 0.98, 1.00, 1.0]),
                ('N', [0.14, 0.20, 0.34, 1.0]),
                ('T', [0.22, 0.62, 1.00, 1.0]),
                ('M', [0.95, 0.92, 0.85, 1.0]),
            ],
        ),
        // 5: College roommate (backwards cap, hoodie, headphones)
        5 => (
            &[
                "..RRRRRRRR..",
                ".RRRRRRRRRRR",
                "..HSSSSSSH..",
                ".ASEESSEESA.",
                ".ASSSSSSSSA.",
                "..SSWWWWSS..",
                "...SSSSSS...",
                ".PPPPPPPPPP.",
                "PPPPWWWWPPPP",
                "PPPPPPPPPPPP",
                "PPPPPPPPPPPP",
                "PPPPPPPPPPPP",
            ],
            &[
                ('R', [0.92, 0.24, 0.28, 1.0]),
                ('H', [0.35, 0.22, 0.12, 1.0]),
                ('S', [0.90, 0.72, 0.56, 1.0]),
                ('E', [0.12, 0.14, 0.18, 1.0]),
                ('W', [0.98, 0.98, 0.98, 1.0]),
                ('A', [0.20, 0.85, 0.95, 1.0]),
                ('P', [0.48, 0.26, 0.78, 1.0]),
            ],
        ),
        // 6: Gym bro (red sweatband, tank top, dumbbell)
        6 => (
            &[
                "...HHHHHH...",
                ".RRRRRRRRRR.",
                "..SSSSSSSS..",
                "..SEESSEES..",
                "..SSSSSSSS..",
                "..SSWWWWSS..",
                "...SSSSSS...",
                "SSSSWWWWSSSS",
                "SSSWWWWWWWSS",
                "DDDWWWWWWDDD",
                "SSSWWWWWWWSS",
                "....WWWW....",
            ],
            &[
                ('H', [0.85, 0.68, 0.25, 1.0]),
                ('R', [0.95, 0.22, 0.22, 1.0]),
                ('S', [0.88, 0.65, 0.48, 1.0]),
                ('E', [0.12, 0.12, 0.16, 1.0]),
                ('W', [0.95, 0.82, 0.18, 1.0]),
                ('D', [0.65, 0.70, 0.78, 1.0]),
            ],
        ),
        // 7: Crypto Discord moderator (cat-ear headset, dark hood, neon visor)
        _ => (
            &[
                ".CC......CC.",
                "CCCCDDDDCCCC",
                ".DDDDDDDDDD.",
                ".DDGGGGGGDD.",
                ".DDGGGGGGDD.",
                ".DDSSSSSSDD.",
                "..DDDDDDDD..",
                ".DDDDYYDDDD.",
                "DDDDYYYYDDDD",
                "DDDDDDDDDDDD",
                "DDDDDDDDDDDD",
                "DDDDDDDDDDDD",
            ],
            &[
                ('C', [0.95, 0.30, 0.85, 1.0]),
                ('D', [0.12, 0.16, 0.24, 1.0]),
                ('G', [0.15, 1.00, 0.65, 1.0]),
                ('S', [0.75, 0.62, 0.55, 1.0]),
                ('Y', [1.00, 0.78, 0.15, 1.0]),
            ],
        ),
    };

    draw_sprite(b, sx, sy, px, rows, pal, false);

    // Border & trusted star badge
    let border_col = if trusted {
        [0.98, 0.80, 0.20, 1.0]
    } else if selected {
        [0.25, 0.78, 1.00, 1.0]
    } else {
        [0.28, 0.34, 0.46, 1.0]
    };
    b.outline(x, y, size, size, border_col);
    if selected {
        b.outline(x - 1.0, y - 1.0, size + 2.0, size + 2.0, border_col);
    }
    if trusted {
        b.circle(x + size - 5.0, y + 5.0, 5.0, [0.98, 0.78, 0.15, 1.0]);
        b.text(x + size - 8.0, y + 1.0, 0.85, "*", [0.10, 0.08, 0.02, 1.0]);
    }
}

/// Draw a 40x40 custom emblem for the selected instrument based on its Kind and Sector.
pub fn draw_asset_emblem(b: &mut Batch, x: f32, y: f32, size: f32, kind: Kind, sector: Sector) {
    let (c_top, c_bot, accent) = match (kind, sector) {
        (Kind::Crypto, _) | (Kind::CryptoEtf, _) | (_, Sector::Crypto) => (
            [0.32, 0.12, 0.48, 1.0],
            [0.12, 0.05, 0.22, 1.0],
            [1.00, 0.78, 0.20, 1.0],
        ),
        (_, Sector::Tech) => (
            [0.08, 0.28, 0.48, 1.0],
            [0.04, 0.12, 0.24, 1.0],
            [0.20, 0.88, 1.00, 1.0],
        ),
        (_, Sector::Energy) => (
            [0.38, 0.24, 0.06, 1.0],
            [0.16, 0.09, 0.03, 1.0],
            [1.00, 0.82, 0.18, 1.0],
        ),
        (_, Sector::Health) => (
            [0.06, 0.34, 0.28, 1.0],
            [0.03, 0.14, 0.12, 1.0],
            [0.22, 0.95, 0.68, 1.0],
        ),
        (_, Sector::Industrials) => (
            [0.18, 0.24, 0.38, 1.0],
            [0.08, 0.10, 0.18, 1.0],
            [0.98, 0.55, 0.22, 1.0],
        ),
        (_, Sector::Metals) => (
            [0.38, 0.30, 0.08, 1.0],
            [0.16, 0.12, 0.04, 1.0],
            [1.00, 0.86, 0.28, 1.0],
        ),
        (_, Sector::Consumer) => (
            [0.36, 0.12, 0.28, 1.0],
            [0.15, 0.05, 0.12, 1.0],
            [0.98, 0.45, 0.78, 1.0],
        ),
        (_, Sector::Staples) => (
            [0.14, 0.32, 0.14, 1.0],
            [0.06, 0.14, 0.06, 1.0],
            [0.65, 0.92, 0.32, 1.0],
        ),
        _ => (
            [0.12, 0.24, 0.42, 1.0],
            [0.05, 0.10, 0.20, 1.0],
            [0.45, 0.78, 1.00, 1.0],
        ),
    };

    b.rect_grad_v(x, y, size, size, c_top, c_bot);
    b.glow(x + size * 0.5, y + size * 0.5, size * 0.48, [accent[0], accent[1], accent[2], 0.30]);
    b.outline(x, y, size, size, accent);

    let px = ((size - 8.0) / 10.0).max(2.0);
    let sx = x + (size - 10.0 * px) * 0.5;
    let sy = y + (size - 10.0 * px) * 0.5;

    let (rows, pal): (&[&str], &[(_, [f32; 4])]) = match (kind, sector) {
        (Kind::Crypto, _) | (Kind::CryptoEtf, _) | (_, Sector::Crypto) => (
            &[
                "...GGGG...",
                ".GGHHHHGG.",
                "GHHGGGGHHG",
                "GHGCCCCGHG",
                "GHGCWWCGHG",
                "GHGCWWCGHG",
                "GHGCCCCGHG",
                "GHHGGGGHHG",
                ".GGHHHHGG.",
                "...GGGG...",
            ],
            &[
                ('G', [0.98, 0.76, 0.18, 1.0]),
                ('H', [1.00, 0.92, 0.55, 1.0]),
                ('C', [0.25, 0.92, 1.00, 1.0]),
                ('W', [1.00, 1.00, 1.00, 1.0]),
            ],
        ),
        (_, Sector::Tech) => (
            &[
                "..P.P.P...",
                ".CCCCCCCC.",
                "PCddddddCP",
                ".CdGGGGdC.",
                "PCdGWWGdCP",
                ".CdGWWGdC.",
                "PCdGGGGdCP",
                ".CddddddC.",
                ".CCCCCCCC.",
                "..P.P.P...",
            ],
            &[
                ('P', [0.98, 0.80, 0.25, 1.0]),
                ('C', [0.20, 0.85, 1.00, 1.0]),
                ('d', [0.08, 0.22, 0.36, 1.0]),
                ('G', [0.20, 0.95, 0.65, 1.0]),
                ('W', [1.00, 1.00, 1.00, 1.0]),
            ],
        ),
        (_, Sector::Energy) => (
            &[
                ".....YYYY.",
                "....YYYY..",
                "...YYYY...",
                "..YYYYYY..",
                ".WWWWWWWW.",
                "...YYYY...",
                "..YYYY....",
                ".YYYY.....",
                "YYYY......",
                "YY........",
            ],
            &[
                ('Y', [1.00, 0.80, 0.15, 1.0]),
                ('W', [1.00, 0.98, 0.80, 1.0]),
            ],
        ),
        (_, Sector::Health) => (
            &[
                "...GGGG...",
                "...GGGG...",
                "...GGGG...",
                "GGGGWWGGGG",
                "GGGWWWWGGG",
                "GGGWWWWGGG",
                "GGGGWWGGGG",
                "...GGGG...",
                "...GGGG...",
                "...GGGG...",
            ],
            &[
                ('G', [0.20, 0.92, 0.62, 1.0]),
                ('W', [0.95, 1.00, 0.98, 1.0]),
            ],
        ),
        (_, Sector::Industrials) => (
            &[
                "....RR....",
                "...RRRR...",
                "...WWWW...",
                "..WWCCWW..",
                "..WWWWWW..",
                ".WWWWWWWW.",
                ".WW.WW.WW.",
                "RR..WW..RR",
                "....OO....",
                "...YYYY...",
            ],
            &[
                ('R', [0.95, 0.30, 0.25, 1.0]),
                ('W', [0.90, 0.94, 0.98, 1.0]),
                ('C', [0.20, 0.82, 1.00, 1.0]),
                ('O', [1.00, 0.55, 0.15, 1.0]),
                ('Y', [1.00, 0.90, 0.25, 1.0]),
            ],
        ),
        (_, Sector::Metals) => (
            &[
                "....WW....",
                "...GGGG...",
                "..HHHHHH..",
                "..GGGGGG..",
                "..........",
                ".GGGG.GGGG",
                "HHHHH.HHHH",
                "GGGGG.GGGG",
                "..........",
                "..........",
            ],
            &[
                ('G', [0.95, 0.74, 0.15, 1.0]),
                ('H', [1.00, 0.92, 0.55, 1.0]),
                ('W', [1.00, 1.00, 0.95, 1.0]),
            ],
        ),
        _ => (
            &[
                "..CCCCCC..",
                ".GGGGGGGG.",
                "CCCCCCCCCC",
                ".CC.CC.CC.",
                ".CC.CC.CC.",
                ".CC.CC.CC.",
                ".CC.CC.CC.",
                ".CC.CC.CC.",
                "GGGGGGGGGG",
                "CCCCCCCCCC",
            ],
            &[
                ('C', [0.55, 0.84, 1.00, 1.0]),
                ('G', [0.98, 0.80, 0.25, 1.0]),
            ],
        ),
    };

    draw_sprite(b, sx, sy, px, rows, pal, true);
}

/// Draw the illustrated banner inside the Media Center panel (TV Studio, Newspaper Masthead, or Wire Radar).
pub fn draw_media_scene(
    b: &mut Batch,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    source: Source,
    regime: Regime,
    frame: u64,
) {
    match source {
        Source::Tv => {
            // MONEY-TV 24 Live Broadcast Studio
            b.rect_grad_v(x, y, w, h, [0.08, 0.14, 0.30, 1.0], [0.04, 0.06, 0.14, 1.0]);
            draw_skyline(b, x + 2.0, y + 2.0, w - 4.0, h - 18.0, frame);

            // Studio big-screen monitor on the left showing market trend
            let mx = x + 10.0;
            let my = y + 6.0;
            let mw = 128.0;
            let mh = h - 24.0;
            b.rect(mx, my, mw, mh, [0.03, 0.08, 0.16, 0.95]);
            let mcol = match regime {
                Regime::Bull => [0.20, 0.92, 0.48, 1.0],
                Regime::Bear => [0.98, 0.28, 0.28, 1.0],
                Regime::Sideways => [0.98, 0.80, 0.22, 1.0],
            };
            b.outline(mx, my, mw, mh, mcol);
            let mlabel = match regime {
                Regime::Bull => "BULL RALLY ▲",
                Regime::Bear => "SELL-OFF ▼",
                Regime::Sideways => "CHOPPY MKT ◆",
            };
            b.text(mx + 6.0, my + 4.0, 0.95, mlabel, mcol);
            // Mini chart wave inside the studio monitor
            for i in 0..8 {
                let x0 = mx + 8.0 + i as f32 * 14.0;
                let x1 = x0 + 14.0;
                let slope = match regime {
                    Regime::Bull => -2.2,
                    Regime::Bear => 2.2,
                    Regime::Sideways => 0.0,
                };
                let y0 = my + mh * 0.65 + (i as f32 * slope) + ((i as f32 * 1.7).sin() * 6.0);
                let y1 = my + mh * 0.65 + ((i + 1) as f32 * slope) + (((i + 1) as f32 * 1.7).sin() * 6.0);
                b.line(x0, y0.clamp(my + 16.0, my + mh - 4.0), x1, y1.clamp(my + 16.0, my + mh - 4.0), 2.0, mcol);
            }

            // News anchor behind broadcast desk in the center-right
            let ax = x + 162.0;
            let ay = y + 4.0;
            b.glow(ax + 24.0, ay + 24.0, 32.0, [0.25, 0.65, 1.0, 0.30]);
            let mouth_open = (frame / 10) % 2 == 0;
            let anchor_rows: &[&str] = if mouth_open {
                &[
                    "...HHHHHH...",
                    "..HHHHHHHH..",
                    "..HSSSSSSH..",
                    "..SEESSEES..",
                    "..SSSSSSSS..",
                    "..SSRRRRSS..",
                    "...SSSSSS...",
                    ".NNNNWWNNNN.",
                    "NNNNNRRNNNNN",
                    "NNNNNRRNNNNM",
                    "DDDDDDDDDDDD",
                    "DDDDDDDDDDDD",
                ]
            } else {
                &[
                    "...HHHHHH...",
                    "..HHHHHHHH..",
                    "..HSSSSSSH..",
                    "..SEESSEES..",
                    "..SSSSSSSS..",
                    "..SSWWWWSS..",
                    "...SSSSSS...",
                    ".NNNNWWNNNN.",
                    "NNNNNRRNNNNN",
                    "NNNNNRRNNNNM",
                    "DDDDDDDDDDDD",
                    "DDDDDDDDDDDD",
                ]
            };
            let anchor_pal = [
                ('H', [0.88, 0.72, 0.25, 1.0]), // golden anchor hair
                ('S', [0.94, 0.76, 0.60, 1.0]), // skin
                ('E', [0.12, 0.14, 0.20, 1.0]), // eyes
                ('R', [0.92, 0.20, 0.24, 1.0]), // red tie / open mouth
                ('W', [0.98, 0.98, 0.98, 1.0]), // collar / smile
                ('N', [0.14, 0.24, 0.45, 1.0]), // studio suit
                ('M', [0.75, 0.80, 0.88, 1.0]), // microphone
                ('D', [0.18, 0.38, 0.72, 1.0]), // news desk
            ];
            draw_sprite(b, ax, ay, 3.2, anchor_rows, &anchor_pal, true);

            // Live badge + studio headline callout on right
            let lx = x + 212.0;
            b.rect(lx, y + 6.0, 64.0, 14.0, [0.85, 0.12, 0.16, 1.0]);
            let pulse = if (frame / 12) % 2 == 0 { [1.0, 1.0, 1.0, 1.0] } else { [1.0, 0.55, 0.55, 1.0] };
            b.circle(lx + 8.0, y + 13.0, 3.5, pulse);
            b.text(lx + 15.0, y + 9.0, 1.0, "ON AIR", [1.0, 1.0, 1.0, 1.0]);
            b.text_shadow(lx + 72.0, y + 8.0, 1.2, "MONEY-TV 24", [0.98, 0.82, 0.22, 1.0]);
            b.text(lx, y + 25.0, 1.0, "HOST: CHET BULLION", [0.65, 0.82, 1.0, 1.0]);
            b.text(lx, y + 36.0, 1.0, "\"BIG MONEY NEVER SLEEPS!\"", [0.85, 0.90, 0.98, 1.0]);

            // Lower-third breaking news chyron bar
            let cy = y + h - 15.0;
            b.rect_grad_h(x + 1.0, cy, w - 2.0, 14.0, [0.85, 0.14, 0.16, 1.0], [0.92, 0.62, 0.08, 1.0]);
            b.text(x + 8.0, cy + 3.0, 1.0, "BREAKING >>> TRADERS BRACE FOR NEXT WEEK'S ACTION", [1.0, 1.0, 1.0, 1.0]);
            b.outline(x, y, w, h, [0.30, 0.55, 0.95, 1.0]);
        }
        Source::Newspaper => {
            // THE DAILY BULL & BEAR Broadsheet Masthead (warm ivory/parchment tint on dark gold)
            b.rect_grad_v(x, y, w, h, [0.22, 0.19, 0.13, 1.0], [0.13, 0.11, 0.08, 1.0]);
            b.rect(x + 4.0, y + 4.0, w - 8.0, 2.0, [0.88, 0.74, 0.38, 0.85]);
            b.rect(x + 4.0, y + h - 6.0, w - 8.0, 2.0, [0.88, 0.74, 0.38, 0.85]);

            // Bull on left, Bear on right flanking the newspaper masthead
            draw_bull(b, x + 10.0, y + 12.0, 2.2);
            draw_bear(b, x + w - 54.0, y + 12.0, 2.2);

            b.text_shadow(x + 68.0, y + 10.0, 1.45, "THE DAILY BULL & BEAR", [0.98, 0.90, 0.68, 1.0]);
            b.rect(x + 68.0, y + 26.0, w - 136.0, 1.0, [0.72, 0.60, 0.34, 0.8]);
            b.text(x + 76.0, y + 31.0, 0.90, "EST. 1889  •  WALL STREET'S PAPER OF RECORD", [0.85, 0.76, 0.54, 1.0]);
            let outlook_str = match regime {
                Regime::Bull => "MARKET BAROMETER: FAIR SKIES & RISING TIDES",
                Regime::Bear => "MARKET BAROMETER: STORM WARNINGS ON THE FLOOR",
                Regime::Sideways => "MARKET BAROMETER: CROSSWINDS & ROTATION",
            };
            b.text(x + 68.0, y + 44.0, 0.90, outlook_str, [0.95, 0.84, 0.42, 1.0]);
            b.outline(x, y, w, h, [0.78, 0.64, 0.32, 1.0]);
        }
        Source::Wire => {
            // STREET WIRE Global Satellite & Radar Terminal
            b.rect_grad_v(x, y, w, h, [0.03, 0.16, 0.14, 1.0], [0.02, 0.08, 0.08, 1.0]);
            // Radar circle on the left
            let rcx = x + 36.0;
            let rcy = y + h * 0.5;
            let rr = (h * 0.40).min(24.0);
            b.glow(rcx, rcy, rr * 1.3, [0.10, 0.95, 0.65, 0.30]);
            b.circle(rcx, rcy, rr, [0.04, 0.24, 0.20, 1.0]);
            b.line(rcx - rr, rcy, rcx + rr, rcy, 1.0, [0.15, 0.65, 0.50, 0.7]);
            b.line(rcx, rcy - rr, rcx, rcy + rr, 1.0, [0.15, 0.65, 0.50, 0.7]);
            let ang = (frame as f32 * 0.08) % 6.28318;
            b.line(rcx, rcy, rcx + ang.cos() * rr, rcy + ang.sin() * rr, 2.0, [0.25, 1.0, 0.75, 1.0]);
            b.circle(rcx + 8.0, rcy - 6.0, 2.5, [0.30, 1.0, 0.80, 1.0]);
            b.circle(rcx - 9.0, rcy + 7.0, 2.0, [1.0, 0.82, 0.20, 1.0]);

            b.text_shadow(x + 74.0, y + 8.0, 1.35, "STREET WIRE  //  DIRECT FEED", [0.25, 0.98, 0.78, 1.0]);
            b.text(x + 74.0, y + 25.0, 0.95, "LOW-LATENCY INSTITUTIONAL TELEMETRY & FLASH ALERTS", [0.55, 0.88, 0.78, 1.0]);

            // Telemetry spectrum bars along bottom right
            for i in 0..24 {
                let bx = x + 74.0 + i as f32 * 13.0;
                let bh = 4.0 + (((i as u64 * 7 + frame / 4) % 13) as f32);
                let bcol = if bh > 12.0 {
                    [0.98, 0.82, 0.22, 0.9]
                } else {
                    [0.18, 0.90, 0.68, 0.85]
                };
                b.rect(bx, y + h - 6.0 - bh, 8.0, bh, bcol);
            }
            b.outline(x, y, w, h, [0.20, 0.82, 0.65, 1.0]);
        }
    }
}
