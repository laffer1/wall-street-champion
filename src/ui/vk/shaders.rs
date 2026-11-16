//! WGSL shader sources, compiled to SPIR-V at runtime with naga (pure Rust —
//! no shader toolchain needed on Linux/FreeBSD/MidnightBSD).
//!
//! The FSR passes are ports of AMD's FidelityFX FSR 1.0 reference
//! (GPUOpen-Effects/FidelityFX-FSR, ffx_fsr1.h): EASU = edge-adaptive spatial
//! upsampling (12-tap Lanczos-style kernel), RCAS = robust contrast-adaptive
//! sharpening (5-tap with min/max clamp + noise detection).

/// Scene pass: colored quads + bitmap-font text into the low-res offscreen.
pub const SCENE_WGSL: &str = r#"
struct U { res: vec2f, pad: vec2f };
@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var font: texture_2d<f32>;

struct VIn {
    @location(0) pos: vec2f,   // pixel coords, y down
    @location(1) col: vec4f,
    @location(2) uv: vec2f,    // font atlas pixel coords
    @location(3) mode: f32,    // 0 = solid color, 1 = font glyph
};
struct VOut {
    @builtin(position) pos: vec4f,
    @location(0) col: vec4f,
    @location(1) uv: vec2f,
    @location(2) mode: f32,
};
@vertex
fn vs(v: VIn) -> VOut {
    var o: VOut;
    // pixel coords are y-down; Vulkan viewport maps NDC y=+1 to the top here,
    // so invert y when converting to NDC.
    o.pos = vec4f(v.pos.x / u.res.x * 2.0 - 1.0, 1.0 - v.pos.y / u.res.y * 2.0, 0.0, 1.0);
    o.col = v.col; o.uv = v.uv; o.mode = v.mode;
    return o;
}
@fragment
fn fs(v: VOut) -> @location(0) vec4f {
    if (v.mode < 0.5) { return v.col; }
    let a = textureLoad(font, vec2i(v.uv), 0).r;
    return vec4f(v.col.rgb, v.col.a * a);
}
"#;

/// Present pass: fullscreen triangle sampling the (possibly FSR-upscaled)
/// image into the swapchain.
pub const PRESENT_WGSL: &str = r#"
@group(0) @binding(0) var src: texture_2d<f32>;
struct VOut { @builtin(position) pos: vec4f };
@vertex
fn vs(@builtin(vertex_index) vi: u32) -> VOut {
    var o: VOut;
    let x = f32(i32(vi & 1u) << 2) - 1.0;
    let y = f32(i32(vi & 2u) << 1) - 1.0;
    o.pos = vec4f(x, y, 0.0, 1.0);
    return o;
}
@fragment
fn fs(v: VOut) -> @location(0) vec4f {
    return textureLoad(src, vec2i(v.pos.xy), 0);
}
"#;

/// FSR 1.0 EASU (edge-adaptive spatial upsampling). Port of FsrEasuF from
/// ffx_fsr1.h. One invocation per output pixel.
pub const EASU_WGSL: &str = r#"
struct C { con0: vec4f, con1: vec4f, con2: vec4f, con3: vec4f };
@group(0) @binding(0) var<uniform> c: C;
@group(0) @binding(1) var src: texture_2d<f32>;
@group(0) @binding(2) var dst: texture_storage_2d<rgba16float, write>;

fn luma(rgb: vec3f) -> f32 { return rgb.b * 0.5 + (rgb.r * 0.5 + rgb.g); }

fn tap(p: vec2i) -> vec3f {
    let d = vec2i(textureDimensions(src));
    return textureLoad(src, clamp(p, vec2i(0, 0), d - vec2i(1, 1)), 0).rgb;
}

// Accumulate direction and length (FsrEasuSetF). corner: 0=s 1=t 2=u 3=v.
fn setF(dir: ptr<function, vec2f>, len: ptr<function, f32>, pp: vec2f, corner: u32,
        lA: f32, lB: f32, lC: f32, lD: f32, lE: f32) {
    var w = 0.0;
    if (corner == 0u) { w = (1.0 - pp.x) * (1.0 - pp.y); }
    if (corner == 1u) { w = pp.x * (1.0 - pp.y); }
    if (corner == 2u) { w = (1.0 - pp.x) * pp.y; }
    if (corner == 3u) { w = pp.x * pp.y; }
    let dc = lD - lC;
    let cb = lC - lB;
    var lenX = max(abs(dc), abs(cb));
    lenX = 1.0 / max(lenX, 1e-20);
    let dirX = lD - lB;
    (*dir).x += dirX * w;
    lenX = clamp(abs(dirX) * lenX, 0.0, 1.0);
    lenX *= lenX;
    *len += lenX * w;
    let ec = lE - lC;
    let ca = lC - lA;
    var lenY = max(abs(ec), abs(ca));
    lenY = 1.0 / max(lenY, 1e-20);
    let dirY = lE - lA;
    (*dir).y += dirY * w;
    lenY = clamp(abs(dirY) * lenY, 0.0, 1.0);
    lenY *= lenY;
    *len += lenY * w;
}

// Filtering for a single tap (FsrEasuTapF).
fn tapF(aC: ptr<function, vec3f>, aW: ptr<function, f32>, off: vec2f, dir: vec2f,
        len2: vec2f, lob: f32, clp: f32, col: vec3f) {
    var v = vec2f(off.x * dir.x + off.y * dir.y, off.x * (-dir.y) + off.y * dir.x);
    v *= len2;
    var d2 = v.x * v.x + v.y * v.y;
    d2 = min(d2, clp);
    // Lanczos2 approximation: (25/16*(2/5*x^2-1)^2 - (25/16-1)) * (lob*x^2-1)^2
    var wB = 2.0 / 5.0 * d2 - 1.0;
    var wA = lob * d2 - 1.0;
    wB *= wB; wA *= wA;
    let w = wB * wA;
    *aC += col * w;
    *aW += w;
}

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) gid: vec3u) {
    let od = textureDimensions(dst);
    if (gid.x >= od.x || gid.y >= od.y) { return; }
    let ip = vec2f(f32(gid.x), f32(gid.y));
    // Position of 'f' in the input image.
    var pp = ip * c.con0.xy + c.con0.zw;
    let fp = floor(pp);
    pp -= fp;
    let base = vec2i(fp);
    // 12-tap kernel:
    //    b c
    //  e f g h
    //  i j k l
    //    n o
    let b = tap(base + vec2i( 0, -1));
    let cc = tap(base + vec2i( 1, -1));
    let e = tap(base + vec2i(-1,  0));
    let f = tap(base + vec2i( 0,  0));
    let g = tap(base + vec2i( 1,  0));
    let h = tap(base + vec2i( 2,  0));
    let i = tap(base + vec2i(-1,  1));
    let j = tap(base + vec2i( 0,  1));
    let k = tap(base + vec2i( 1,  1));
    let l = tap(base + vec2i( 2,  1));
    let n = tap(base + vec2i( 0,  2));
    let o = tap(base + vec2i( 1,  2));
    let bL = luma(b); let cL = luma(cc); let eL = luma(e); let fL = luma(f);
    let gL = luma(g); let hL = luma(h); let iL = luma(i); let jL = luma(j);
    let kL = luma(k); let lL = luma(l); let nL = luma(n); let oL = luma(o);
    var dir = vec2f(0.0, 0.0);
    var len = 0.0;
    setF(&dir, &len, pp, 0u, bL, eL, fL, gL, jL);
    setF(&dir, &len, pp, 1u, cL, fL, gL, hL, kL);
    setF(&dir, &len, pp, 2u, fL, iL, jL, kL, nL);
    setF(&dir, &len, pp, 3u, gL, jL, kL, lL, oL);
    // Normalize with approximation, cleanup close to zero.
    let dir2 = dir * dir;
    var dirR = dir2.x + dir2.y;
    let zro = dirR < 1.0 / 32768.0;
    dirR = inverseSqrt(max(dirR, 1e-20));
    if (zro) { dirR = 1.0; dir.x = 1.0; }
    dir *= dirR;
    // Transform {0..2} to {0..1}, shape with square.
    len = len * 0.5;
    len *= len;
    // Stretch kernel {1.0 vert|horz, sqrt(2) diagonal}.
    let stretch = (dir.x * dir.x + dir.y * dir.y) / max(max(abs(dir.x), abs(dir.y)), 1e-6);
    let len2 = vec2f(1.0 + (stretch - 1.0) * len, 1.0 - 0.5 * len);
    let lob = 0.5 + (0.25 - 0.04 - 0.5) * len;
    let clp = 1.0 / lob;
    // Min/max of 4 nearest (f g j k).
    let min4 = min(min(min(f, g), j), k);
    let max4 = max(max(max(f, g), j), k);
    var aC = vec3f(0.0, 0.0, 0.0);
    var aW = 0.0;
    tapF(&aC, &aW, vec2f( 0.0, -1.0) - pp, dir, len2, lob, clp, b);
    tapF(&aC, &aW, vec2f( 1.0, -1.0) - pp, dir, len2, lob, clp, cc);
    tapF(&aC, &aW, vec2f(-1.0,  1.0) - pp, dir, len2, lob, clp, i);
    tapF(&aC, &aW, vec2f( 0.0,  1.0) - pp, dir, len2, lob, clp, j);
    tapF(&aC, &aW, vec2f( 0.0,  0.0) - pp, dir, len2, lob, clp, f);
    tapF(&aC, &aW, vec2f(-1.0,  0.0) - pp, dir, len2, lob, clp, e);
    tapF(&aC, &aW, vec2f( 1.0,  1.0) - pp, dir, len2, lob, clp, k);
    tapF(&aC, &aW, vec2f( 2.0,  1.0) - pp, dir, len2, lob, clp, l);
    tapF(&aC, &aW, vec2f( 2.0,  0.0) - pp, dir, len2, lob, clp, h);
    tapF(&aC, &aW, vec2f( 1.0,  0.0) - pp, dir, len2, lob, clp, g);
    tapF(&aC, &aW, vec2f( 1.0,  2.0) - pp, dir, len2, lob, clp, o);
    tapF(&aC, &aW, vec2f( 0.0,  2.0) - pp, dir, len2, lob, clp, n);
    // Normalize and dering.
    let pix = min(max4, max(min4, aC * (1.0 / max(aW, 1e-20))));
    textureStore(dst, vec2i(gid.xy), vec4f(pix, 1.0));
}
"#;

/// FSR 1.0 RCAS (robust contrast-adaptive sharpening). Port of FsrRcasF.
pub const RCAS_WGSL: &str = r#"
struct C { sharp: f32, pad: vec3f };
@group(0) @binding(0) var<uniform> c: C;
@group(0) @binding(1) var src: texture_2d<f32>;
@group(0) @binding(2) var dst: texture_storage_2d<rgba16float, write>;

fn luma(rgb: vec3f) -> f32 { return rgb.b * 0.5 + (rgb.r * 0.5 + rgb.g); }

fn ld(p: vec2i) -> vec3f {
    let d = vec2i(textureDimensions(src));
    return textureLoad(src, clamp(p, vec2i(0, 0), d - vec2i(1, 1)), 0).rgb;
}

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) gid: vec3u) {
    let od = textureDimensions(dst);
    if (gid.x >= od.x || gid.y >= od.y) { return; }
    let sp = vec2i(gid.xy);
    //    b
    //  d e f
    //    h
    let b = ld(sp + vec2i( 0, -1));
    let d = ld(sp + vec2i(-1,  0));
    let e = ld(sp + vec2i( 0,  0));
    let f = ld(sp + vec2i( 1,  0));
    let h = ld(sp + vec2i( 0,  1));
    let bL = luma(b); let dL = luma(d); let eL = luma(e); let fL = luma(f); let hL = luma(h);
    // Noise detection.
    var nz = 0.25 * bL + 0.25 * dL + 0.25 * fL + 0.25 * hL - eL;
    let rng = max(max(max(bL, dL), max(eL, fL)), hL) - min(min(min(bL, dL), min(eL, fL)), hL);
    nz = clamp(abs(nz) / max(rng, 1e-6), 0.0, 1.0);
    nz = -0.5 * nz + 1.0;
    // Min/max of ring, per channel.
    let mn4 = min(min(min(b, d), f), h);
    let mx4 = max(max(max(b, d), f), h);
    // Limiters.
    let hitMin = min(mn4, e) * (1.0 / (4.0 * max(mx4, vec3f(1e-6))));
    let hitMax = (vec3f(1.0) - max(mx4, e)) * (1.0 / (4.0 * max(mn4, vec3f(1e-6)) - vec3f(4.0)));
    let lobe3 = max(-hitMin, hitMax);
    let lobe = max(-0.1875, min(max(max(lobe3.r, lobe3.g), lobe3.b), 0.0)) * c.sharp;
    // Apply noise removal and clamp.
    let pix = min(mx4, max(mn4, lobe * nz + e));
    textureStore(dst, sp, vec4f(pix, 1.0));
}
"#;

/// Compile WGSL source to SPIR-V words via naga.
pub fn wgsl_to_spv(src: &str, entry: &str) -> Result<Vec<u32>, String> {
    let module = naga::front::wgsl::parse_str(src).map_err(|e| format!("wgsl parse: {:?}", e))?;
    let mut validator = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    );
    let info = validator.validate(&module).map_err(|e| format!("wgsl validate: {:?}", e))?;
    let options = naga::back::spv::Options::default();
    let _ = entry; // all modules are single-entry-point relevant; write whole module
    naga::back::spv::write_vec(&module, &info, &options, None).map_err(|e| format!("spv write: {:?}", e))
}
