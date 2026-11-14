// Abschluss aufs Bild: Szene + durchsichtige Schicht, Umgebungsverdeckung (Ultra+), Tiefenunschärfe und Farbsaum (Extrem),
// Leere-Nebel und Lichtstrahlen (Ultra leicht, Extrem voll), Bloom, ACES, Farbe, Vignette.
@group(1) @binding(0) var t_scene: texture_2d<f32>;
@group(1) @binding(1) var t_clear: texture_2d<f32>;
@group(1) @binding(2) var t_depth: texture_2d<f32>;
@group(1) @binding(3) var t_half: texture_2d<f32>;
@group(1) @binding(4) var t_q1: texture_2d<f32>;
@group(1) @binding(5) var t_q2: texture_2d<f32>;
@group(1) @binding(6) var smp: sampler;

struct FO { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };

@vertex
fn vs_full(@builtin(vertex_index) k: u32) -> FO {
  let p = vec2<f32>(f32((k << 1u) & 2u), f32(k & 2u)) * 2.0 - 1.0;
  var o: FO;
  o.pos = vec4<f32>(p, 0.0, 1.0);
  o.uv = vec2<f32>(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5);
  return o;
}

fn depth_px(p: vec2<i32>) -> f32 {
  let q = clamp(p, vec2<i32>(0), vec2<i32>(g.scr.xy) - vec2<i32>(1));
  return textureLoad(t_depth, q, 0).r;
}

fn comp_uv(uv: vec2<f32>) -> vec3<f32> {
  let a = dec(textureSampleLevel(t_scene, smp, uv, 0.0).rgb);
  let b = textureSampleLevel(t_clear, smp, uv, 0.0);
  return dec(b.rgb) + a * (1.0 - b.a);
}

// lineare Tiefe (Abstand zum Auge) an einer Bildstelle, Himmel weit weg
fn lin_depth(uv: vec2<f32>) -> f32 {
  let d = depth_px(vec2<i32>(uv * g.scr.xy));
  if d >= 1.0 { return 10000.0; }
  return distance(world_from(uv, d), g.cam.xyz);
}

// Bloom tiefenbewusst hochrechnen: Abgriffe hinter/vor einer Kante zählen kaum (kein Hof um nahe Laternen)
fn bloom_up(uv: vec2<f32>, dc: f32, far: bool) -> vec3<f32> {
  var px = 1.0 / vec2<f32>(textureDimensions(t_q1));
  if far { px = 1.0 / vec2<f32>(textureDimensions(t_q2)); }
  var offs = array<vec2<f32>, 5>(vec2<f32>(0.0, 0.0), vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(-1.0, 1.0), vec2<f32>(1.0, 1.0));
  var acc = vec3<f32>(0.0);
  var ws = 0.0;
  for (var k = 0; k < 5; k = k + 1) {
    let tu = clamp(uv + offs[k] * px, vec2<f32>(0.0), vec2<f32>(1.0));
    var c = textureSampleLevel(t_q1, smp, tu, 0.0).rgb;
    if far { c = textureSampleLevel(t_q2, smp, tu, 0.0).rgb; }
    var w = 1.0;
    if k > 0 { w = exp(-abs(dc - lin_depth(tu)) * 14.0); }
    acc = acc + dec(c) * w;
    ws = ws + w;
  }
  return acc / ws;
}

// Umgebungsverdeckung aus 6 Spiral-Abgriffen der Tiefe
fn gtao(pix: vec2<i32>, wp: vec3<f32>, dist: f32) -> f32 {
  let uv = (vec2<f32>(pix) + 0.5) * g.scr.zw;
  let px = world_from(uv + vec2<f32>(g.scr.z, 0.0), depth_px(pix + vec2<i32>(1, 0)));
  let py = world_from(uv + vec2<f32>(0.0, g.scr.w), depth_px(pix + vec2<i32>(0, 1)));
  var n = normalize(cross(py - wp, px - wp));
  if dot(n, g.cam.xyz - wp) < 0.0 { n = -n; }
  let rad = clamp(g.scr.y * 0.55 / max(dist, 0.5), 4.0, 70.0);
  let rot = h2(vec2<f32>(pix)) * 6.2832;
  var occ = 0.0;
  for (var k = 0; k < 6; k = k + 1) {
    let fk = f32(k);
    let a = rot + fk * 2.4;
    let r = rad * (fk + 1.0) / 6.0;
    let q = pix + vec2<i32>(vec2<f32>(cos(a), sin(a)) * r);
    let d = depth_px(q);
    if d < 1.0 {
      let ps = world_from((vec2<f32>(q) + 0.5) * g.scr.zw, d);
      let v = ps - wp;
      let l = length(v);
      if l > 0.01 && l < 1.6 { occ = occ + max(dot(n, v / l) - 0.15, 0.0) * (1.0 - l / 1.6); }
    }
  }
  return clamp(1.0 - occ / 6.0 * 1.8, 0.35, 1.0);
}

// Nebel der Leere unter den Inseln (unter y 4), von der Sonne/dem Mond angeleuchtet
fn void_mist(col: vec3<f32>, wp: vec3<f32>, steps: i32) -> vec3<f32> {
  let o = g.cam.xyz;
  let rd = wp - o;
  let len = length(rd);
  let dir = rd / max(len, 0.001);
  let top = 4.0;
  let bot = -45.0;
  // Strahlabschnitt innerhalb der Schicht bot..top
  var t0 = 0.0;
  var t1 = len;
  if abs(dir.y) > 0.0001 {
    let ta = (top - o.y) / dir.y;
    let tb = (bot - o.y) / dir.y;
    t0 = max(t0, min(ta, tb));
    t1 = min(t1, max(ta, tb));
  } else if o.y > top || o.y < bot { return col; }
  if t1 <= t0 { return col; }
  let st = (t1 - t0) / f32(steps);
  let t = g.cam.w;
  var trans = 1.0;
  var acc = vec3<f32>(0.0);
  let phase = 0.6 + 0.8 * pow(max(dot(dir, g.sun.xyz), 0.0), 4.0);
  let lc = g.amb.xyz * 0.9 + g.sun_col.xyz * 0.25 * phase + vec3<f32>(0.02, 0.01, 0.04);
  for (var k = 0; k < steps; k = k + 1) {
    let p = o + dir * (t0 + (f32(k) + h2(wp.xz + f32(k))) * st);
    let n = noise3(p * 0.07 + vec3<f32>(t * 0.08, -t * 0.02, t * 0.05)) * 0.65 + noise3(p * 0.19 - vec3<f32>(t * 0.05, 0.0, t * 0.11)) * 0.35;
    let dens = smoothstep(top, top - 18.0, p.y) * smoothstep(0.32, 0.75, n) * 0.06;
    let a = 1.0 - exp(-dens * st);
    acc = acc + lc * a * trans;
    trans = trans * (1.0 - a);
  }
  return col * trans + acc;
}

// Lichtstrahlen: vom Pixel zur Sonne auf dem Bildschirm, Himmel = Licht, Blöcke = Schatten
fn god_rays(uv: vec2<f32>, steps: i32) -> vec3<f32> {
  let sc = g.vp * vec4<f32>(g.cam.xyz + g.sun.xyz * 300.0, 1.0);
  if sc.w <= 0.0 { return vec3<f32>(0.0); }
  let su = vec2<f32>(sc.x / sc.w * 0.5 + 0.5, 0.5 - sc.y / sc.w * 0.5);
  let dl = (su - uv) / f32(steps) * 0.85;
  var p = uv + dl * h2(uv * g.scr.xy);
  var acc = 0.0;
  var decay = 1.0;
  for (var k = 0; k < steps; k = k + 1) {
    p = p + dl;
    if p.x < 0.0 || p.y < 0.0 || p.x > 1.0 || p.y > 1.0 { break; }
    let d = depth_px(vec2<i32>(p * g.scr.xy));
    acc = acc + step(0.99999, d) * decay;
    decay = decay * 0.92;
  }
  let toward = pow(max(dot(normalize(world_from(vec2<f32>(0.5), 1.0) - g.cam.xyz), g.sun.xyz), 0.0), 2.0);
  let fade = 1.0 - smoothstep(0.3, 1.2, length(su - uv));
  return g.sun_col.xyz * acc / f32(steps) * 0.35 * toward * fade * (0.6 + g.amb.w * 1.5) * g.sun.w;
}

@fragment
fn fs_final(i: FO) -> @location(0) vec4<f32> {
  let tier = g.par.w;
  let pix = vec2<i32>(i.pos.xy);
  let uv = i.uv;
  let d = depth_px(pix);
  var wp = world_from(uv, d);
  var dist = distance(wp, g.cam.xyz);
  if d >= 1.0 { dist = 10000.0; wp = g.cam.xyz + normalize(wp - g.cam.xyz) * 300.0; }

  var a = dec(textureLoad(t_scene, pix, 0).rgb);
  let b = textureLoad(t_clear, pix, 0);
  if tier > 1.5 && d < 1.0 && dist < 80.0 && g.par.y > 0.5 { a = a * gtao(pix, wp, dist); }
  var col = dec(b.rgb) + a * (1.0 - b.a);

  if tier > 2.5 {
    // Farbsaum zum Rand hin
    let off = (uv - 0.5) * 0.004;
    col = vec3<f32>(comp_uv(uv + off).r, col.g, comp_uv(uv - off).b);
    // ferne Inseln weich (Tiefenunschärfe erst ab 48 m, voll ab 96 m; davor nie)
    if d < 1.0 {
      let hp = 1.0 / vec2<f32>(textureDimensions(t_half));
      let bl = (dec(textureSampleLevel(t_half, smp, uv + hp * vec2<f32>(-1.5, -1.5), 0.0).rgb) + dec(textureSampleLevel(t_half, smp, uv + hp * vec2<f32>(1.5, -1.5), 0.0).rgb)
              + dec(textureSampleLevel(t_half, smp, uv + hp * vec2<f32>(-1.5, 1.5), 0.0).rgb) + dec(textureSampleLevel(t_half, smp, uv + hp * vec2<f32>(1.5, 1.5), 0.0).rgb)) * 0.25;
      col = mix(col, bl, smoothstep(48.0, 96.0, dist) * 0.85);
    }
  }
  if tier > 1.5 && g.par.z < 0.5 {
    var steps = 4;
    if tier > 2.5 { steps = 8; }
    col = void_mist(col, wp, steps);
    var rs = 6;
    if tier > 2.5 { rs = 12; }
    col = col + god_rays(uv, rs);
  }
  // Bloom
  col = col + bloom_up(uv, dist, false) * 0.4;
  if tier > 1.5 { col = col + bloom_up(uv, dist, true) * 0.35; }

  var o = present(col);
  // Farbe: etwas lebendiger, Vignette
  let l = luma(o);
  let sat = max(o.r, max(o.g, o.b)) - min(o.r, min(o.g, o.b));
  o = mix(vec3<f32>(l), o, 1.0 + 0.22 * (1.0 - sat));
  let q = uv - 0.5;
  o = o * (1.0 - 0.32 * dot(q, q) * 2.0);
  return vec4<f32>(clamp(o, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}
