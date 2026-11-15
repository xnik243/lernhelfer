// Szene: Blöcke (prozedurale 16×16-Muster mit PBR, Normalen aus Höhenfeld, Parallax), Himmel, Würfel-Instanzen,
// Schattenkarte, Wasser mit Spiegelung/Brechung/Schaum. Gruppe 1 nur im Durchsichtig-Durchgang (HDR-Stufen).
@group(1) @binding(0) var scene_tex: texture_2d<f32>;
@group(1) @binding(1) var scene_depth: texture_2d<f32>; // Tiefe als unfilterbares float (textureLoad)
@group(1) @binding(2) var lin_samp: sampler;
// eigenes Texturpaket (4×4 Kacheln = Muster 0..15), sonst 1×1 weiß
@group(0) @binding(3) var pack_tex: texture_2d<f32>;
@group(0) @binding(4) var pack_samp: sampler;

struct VO {
  @builtin(position) pos: vec4<f32>,
  @location(0) wp: vec3<f32>,
  @location(1) ao: f32,
  @location(2) @interpolate(flat) a: u32,
  @location(3) @interpolate(flat) b: u32,
};

@vertex
fn vs(@location(0) a: u32, @location(1) b: u32) -> VO {
  let p = vec3<f32>(f32(a & 1023u), f32((a >> 10u) & 127u), f32((a >> 17u) & 1023u));
  var o: VO;
  o.pos = g.vp * vec4<f32>(p, 1.0);
  o.wp = p;
  o.ao = f32(a >> 30u) / 3.0;
  o.a = a;
  o.b = b;
  return o;
}

@vertex
fn vs_shadow(@location(0) a: u32, @location(1) b: u32) -> @builtin(position) vec4<f32> {
  let p = vec3<f32>(f32(a & 1023u), f32((a >> 10u) & 127u), f32((a >> 17u) & 1023u));
  return g.lvp * vec4<f32>(p, 1.0);
}

@fragment
fn fs_none() {}

// Pixelstufen der Blockmuster (16, 64, 128); par2.w < 0 = eigenes Paket aktiv
fn tex_res() -> f32 {
  let r = abs(g.par2.w);
  if r < 1.0 { return 16.0; }
  return r;
}
fn pack_on() -> bool { return g.par2.w < 0.0; }

// Kachel des Pakets für ein Muster (sRGB wie die Blockfarbe), ohne Ableitungen
fn pack_tile(pat: u32, uv: vec2<f32>) -> vec4<f32> {
  let n = vec2<f32>(textureDimensions(pack_tex)) * 0.25;
  let f = clamp(vec2<f32>(fract(uv.x), 1.0 - fract(uv.y)), 0.5 / n, vec2<f32>(1.0) - 0.5 / n);
  let tile = vec2<f32>(f32(pat & 3u), f32(pat >> 2u));
  return textureSampleLevel(pack_tex, pack_samp, (tile + f) * 0.25, 0.0);
}

// feine Mehr-Oktaven-Details ab 64er-Auflösung (Faktor um 1): Steinkorn, Holzfasern, Kristallfacetten
fn micro(pat: u32, uv: vec2<f32>, res: f32) -> f32 {
  let hd = smoothstep(20.0, 64.0, res);
  if hd <= 0.0 { return 1.0; }
  let q = floor(uv * res) / res;
  let p = vec3<f32>(q * 16.0, f32(pat) * 7.0);
  var d = 0.0;
  switch pat {
    case 6u, 8u, 10u, 12u: { return 1.0; }
    case 3u: {
      d = (noise3(vec3<f32>(q.x * 3.0, q.y * 64.0, 1.0)) - 0.5) * 0.24 + (noise3(vec3<f32>(q.x * 9.0, q.y * 170.0, 2.0)) - 0.5) * 0.12;
    }
    case 15u: {
      let f = fract(uv * 8.0) - vec2<f32>(0.5);
      d = (abs(f.x) - abs(f.y)) * 0.3 + (h2(q * res) - 0.5) * 0.08;
    }
    default: {
      d = (noise3(p) - 0.5) * 0.16 + (noise3(p * 2.7 + vec3<f32>(5.0)) - 0.5) * 0.10 + (h2(q * res + vec2<f32>(3.0)) - 0.5) * 0.08;
    }
  }
  return 1.0 + d * hd;
}

struct Mat { col: vec3<f32>, alpha: f32, rough: f32, metal: f32, emit: f32, };

// Muster je Blockseite (Farbe im sRGB-Raum wie entworfen), Rauheit, Metall, Leuchten
fn block_mat(b: u32, uv: vec2<f32>, wp: vec3<f32>, side: bool) -> Mat {
  let base = vec3<f32>(f32(b & 255u), f32((b >> 8u) & 255u), f32((b >> 16u) & 255u)) / 255.0;
  let pat = (b >> 24u) & 15u;
  let glow = f32((b >> 28u) & 1u);
  let res = tex_res();
  let tx = floor(uv * res);
  let px = tx / res;
  let r = h2(tx + f32(pat) * 17.0);
  let f = fract(uv);
  let t = g.cam.w;
  var m: Mat;
  m.col = base;
  m.alpha = 1.0;
  m.rough = 0.88;
  m.metal = 0.0;
  m.emit = glow * 1.5;
  var c = base;
  switch pat {
    case 1u: { // Wiese: oben gesprenkelt, an den Seiten Erde mit Grasrand
      let lip = 0.78 - 0.16 * h2(vec2<f32>(tx.x, 5.0));
      if side && fract(px.y) < lip { c = vec3<f32>(0.53, 0.39, 0.27) * (0.88 + 0.24 * r); } else { c = c * (0.86 + 0.28 * r); m.rough = 0.75; }
    }
    case 2u: { // Ziegel im Verband
      let row = floor(uv.y * 4.0);
      let bx = fract(uv.x * 2.0 + 0.5 * (row - 2.0 * floor(row / 2.0)));
      if fract(uv.y * 4.0) < 0.14 || bx < 0.07 { c = mix(c, vec3<f32>(0.80, 0.78, 0.74), 0.75); m.rough = 0.95; }
      else { c = c * (0.86 + 0.26 * h2(vec2<f32>(floor(uv.x * 2.0 + 0.5 * (row - 2.0 * floor(row / 2.0))), row))); m.rough = 0.7; }
    }
    case 3u: { // Bretter, leicht lackiert
      let board = floor(uv.y * 4.0);
      c = c * (0.90 + 0.18 * h2(vec2<f32>(floor(uv.x * res), board)));
      if fract(uv.y * 4.0) < 0.10 { c = c * 0.72; }
      m.rough = 0.5;
    }
    case 4u: { c = c * (0.84 + 0.30 * r); }
    case 5u: { // Fliesen im Schachmuster mit Fuge, poliert
      let q = floor(uv * 2.0);
      if (q.x + q.y) - 2.0 * floor((q.x + q.y) / 2.0) > 0.5 { c = c * 0.80; }
      let e = fract(uv * 2.0);
      if e.x < 0.06 || e.y < 0.06 { c = c * 0.70; m.rough = 0.9; } else { m.rough = 0.3; }
    }
    case 6u: { // Leuchtstein, Neon, Kristall
      c = c * (0.92 + 0.10 * sin(t * 2.0 + (wp.x + wp.z) * 0.7)) + 0.10 * r;
      m.alpha = 0.78;
      m.rough = 0.25;
      m.emit = glow * 1.6;
    }
    case 7u: { c = c * (0.78 + 0.34 * h2(vec2<f32>(floor(uv.x * 8.0), floor(uv.y * 2.0)))); m.rough = 0.92; }
    case 8u: { // Wasser (einfach) / Lava
      c = c + 0.05 * sin(wp.x * 1.3 + t * 1.6) + 0.05 * sin(wp.z * 1.7 - t * 1.2) + 0.04 * r;
      m.alpha = 0.72;
      m.rough = 0.08;
      if glow > 0.5 { m.alpha = 1.0; m.emit = 1.5 + 0.4 * sin(t * 1.3 + wp.x + wp.z); m.rough = 0.6; }
    }
    case 9u: { c = c * (0.72 + 0.46 * r); m.rough = 0.8; }
    case 10u: { // Schimmer: Farbe wandert; durchsichtig = Portal mit Wirbel
      let w = 0.5 + 0.5 * sin(t * 1.4 + (wp.x + wp.y + wp.z) * 0.9);
      c = mix(c, c.zxy * 1.15, w) * (0.92 + 0.16 * r);
      m.rough = 0.25;
      m.metal = 0.6;
      if ((b >> 29u) & 1u) == 1u {
        let q = fract(px) - 0.5;
        let a = atan2(q.y, q.x) + length(q) * 9.0 - t * 2.5;
        c = c * (0.7 + 0.5 * (0.5 + 0.5 * sin(a * 3.0)));
        m.alpha = 0.72;
        m.metal = 0.0;
        m.emit = 1.1;
      }
    }
    case 11u: { // Bücherregal
      if side {
        if fract(uv.y * 2.0) < 0.12 { c = c * 0.8; }
        else {
          let id = vec2<f32>(floor(uv.x * 8.0), floor(uv.y * 2.0));
          c = mix(vec3<f32>(0.70, 0.22, 0.20), vec3<f32>(0.20, 0.36, 0.70), h2(id + 3.0)) * (0.75 + 0.5 * h2(id + 9.0));
        }
      } else { c = c * (0.92 + 0.12 * r); }
      m.rough = 0.7;
    }
    case 12u: { // Glas: Rahmen deckend, Fläche fast klar
      m.alpha = 0.30;
      m.rough = 0.05;
      if f.x < 0.08 || f.x > 0.92 || f.y < 0.08 || f.y > 0.92 { m.alpha = 0.9; m.rough = 0.5; }
    }
    case 13u: { // Bruchstein
      let q = fract(px);
      let row = floor(q.y * 4.0);
      let sx = q.x * 3.0 + h2(vec2<f32>(row, floor(uv.y))) * 2.0;
      let id = vec2<f32>(floor(sx), row);
      let e = vec2<f32>(fract(sx), fract(q.y * 4.0));
      if e.x < 0.13 || e.y < 0.2 { c = c * 0.55; } else { c = c * (0.78 + 0.3 * h2(id + floor(uv) * 7.0) + 0.1 * r); }
    }
    case 14u: { // Steinziegel, geschliffen
      let q = fract(px);
      let row = floor(q.y * 2.0);
      let bx = fract(q.x + 0.5 * row);
      let by = fract(q.y * 2.0);
      if by < 0.125 || bx < 0.0625 { c = c * 0.58; }
      else if by > 0.82 { c = c * 1.12; m.rough = 0.45; }
      else { c = c * (0.88 + 0.16 * r); m.rough = 0.45; }
    }
    case 15u: { // Kristallader: Fels mit glitzernden Edelstein-Einschlüssen
      let rock = vec3<f32>(0.47, 0.48, 0.52) * (0.8 + 0.3 * r);
      let v = h2(floor(uv * 8.0) + 41.0);
      if v > 0.68 {
        let tw = 0.75 + 0.25 * sin(t * 3.0 + v * 40.0);
        c = c * (0.9 + 0.4 * r) * tw;
        m.rough = 0.12;
        m.metal = 0.85;
        m.emit = 0.9 * tw;
      } else { c = rock; m.emit = 0.0; }
    }
    default: { c = c * (0.95 + 0.08 * r); }
  }
  if pack_on() {
    let tl = pack_tile(pat, uv);
    c = base * tl.rgb;
    m.alpha = m.alpha * tl.a;
  } else {
    c = c * micro(pat, uv, res);
  }
  m.col = c;
  return m;
}

// Höhenfeld 0..1 (1 = Oberfläche) für Normalen und Parallax: Fugen, Furchen, Facetten
fn height(pat: u32, uv: vec2<f32>, side: bool) -> f32 {
  let res = tex_res();
  if res > 20.0 && !pack_on() { return height0(pat, uv, side, res) + (micro(pat, uv, res) - 1.0) * 0.5; }
  return height0(pat, uv, side, res);
}

// Fugen mit Fase ab HD (Abstand zur Fuge in Blockeinheiten), sonst harte Stufe wie bei 16
fn bevel(edge: f32, lo: f32, res: f32) -> f32 {
  if res > 20.0 { return mix(lo, 1.0, smoothstep(0.0, 0.025, edge)); }
  return select(lo, 1.0, edge > 0.0);
}

fn height0(pat: u32, uv: vec2<f32>, side: bool, res: f32) -> f32 {
  let tx = floor(uv * res);
  let r = h2(tx + f32(pat) * 17.0);
  switch pat {
    case 2u: {
      let row = floor(uv.y * 4.0);
      let bx = fract(uv.x * 2.0 + 0.5 * (row - 2.0 * floor(row / 2.0)));
      return mix(0.15, 0.9 + 0.1 * r, bevel(min((fract(uv.y * 4.0) - 0.14) * 0.25, (bx - 0.07) * 0.5), 0.0, res));
    }
    case 3u: {
      if fract(uv.y * 4.0) < 0.10 { return 0.35; }
      return 0.95 + 0.05 * r;
    }
    case 5u: {
      let e = fract(uv * 2.0);
      if e.x < 0.06 || e.y < 0.06 { return 0.4; }
      return 1.0;
    }
    case 7u: {
      if !side { return 0.8 + 0.2 * r; }
      return 0.35 + 0.65 * h2(vec2<f32>(floor(uv.x * 8.0), floor(uv.y * 2.0)));
    }
    case 11u: {
      if !side { return 1.0; }
      if fract(uv.y * 2.0) < 0.12 { return 1.0; }
      return 0.45 + 0.25 * h2(vec2<f32>(floor(uv.x * 8.0), floor(uv.y * 2.0)) + 9.0);
    }
    case 13u: {
      let q = fract(tx / res);
      let row = floor(q.y * 4.0);
      let sx = q.x * 3.0 + h2(vec2<f32>(row, floor(uv.y))) * 2.0;
      let e = vec2<f32>(fract(sx), fract(q.y * 4.0));
      if e.x < 0.13 || e.y < 0.2 { return 0.0; }
      let dome = min(min(e.x - 0.13, 1.0 - e.x), min(e.y - 0.2, 1.0 - e.y));
      return 0.6 + 0.4 * clamp(dome * 5.0, 0.0, 1.0) * (0.8 + 0.2 * h2(vec2<f32>(floor(sx), row)));
    }
    case 14u: {
      let q = fract(tx / res);
      let row = floor(q.y * 2.0);
      let bx = fract(q.x + 0.5 * row);
      let by = fract(q.y * 2.0);
      return bevel(min((by - 0.125) * 0.5, bx - 0.0625), 0.1, res);
    }
    case 15u: {
      let v = h2(floor(uv * 8.0) + 41.0);
      let fq = floor(fract(uv * 8.0) * (res / 8.0)) / (res / 8.0);
      if v > 0.68 { return 1.0 - 0.3 * abs(fq.x - fq.y); }
      return 0.55 + 0.2 * r;
    }
    case 4u, 9u: { return 0.75 + 0.25 * r; }
    case 1u: { return 0.85 + 0.15 * r; }
    default: { return 1.0; }
  }
}

fn has_relief(pat: u32) -> bool {
  return pat == 1u || pat == 2u || pat == 3u || pat == 4u || pat == 5u || pat == 7u || pat == 9u || pat == 11u || pat == 13u || pat == 14u || pat == 15u;
}
fn has_depth(pat: u32) -> bool {
  return pat == 2u || pat == 3u || pat == 7u || pat == 11u || pat == 13u || pat == 14u || pat == 15u;
}

struct Face { n: vec3<f32>, t: vec3<f32>, bt: vec3<f32>, uv: vec2<f32>, side: bool, };

fn face_of(a: u32, wp: vec3<f32>) -> Face {
  let nb = (a >> 27u) & 7u;
  let ax = nb >> 1u;
  let sg = f32(nb & 1u) * 2.0 - 1.0;
  var fc: Face;
  fc.side = ax != 1u;
  if ax == 0u {
    fc.n = vec3<f32>(sg, 0.0, 0.0); fc.uv = wp.zy; fc.t = vec3<f32>(0.0, 0.0, 1.0); fc.bt = vec3<f32>(0.0, 1.0, 0.0);
  } else if ax == 1u {
    fc.n = vec3<f32>(0.0, sg, 0.0); fc.uv = wp.xz; fc.t = vec3<f32>(1.0, 0.0, 0.0); fc.bt = vec3<f32>(0.0, 0.0, 1.0);
  } else {
    fc.n = vec3<f32>(0.0, 0.0, sg); fc.uv = wp.xy; fc.t = vec3<f32>(1.0, 0.0, 0.0); fc.bt = vec3<f32>(0.0, 1.0, 0.0);
  }
  return fc;
}

const RELIEF: f32 = 0.06; // Tiefe der Fugen in Blockeinheiten

// Blockfläche fertig beleuchtet (linear, HDR, mit Nebel) und Deckkraft
fn block_color(i: VO) -> vec4<f32> {
  let fc = face_of(i.a, i.wp);
  let pat = (i.b >> 24u) & 15u;
  let tier = g.par.w;
  let to_eye = g.cam.xyz - i.wp;
  let dist = length(to_eye);
  let v = to_eye / max(dist, 0.001);
  var uv = fc.uv;
  var pshadow = 1.0;
  // Extrem: Parallax Occlusion Mapping in der Nähe (4 Schritte linear + 2 Halbierungen) mit Selbstschatten
  if tier > 2.5 && dist < 14.0 && has_depth(pat) {
    let vt = vec3<f32>(dot(v, fc.t), dot(v, fc.bt), dot(v, fc.n));
    if vt.z > 0.05 {
      let shift = -vt.xy / vt.z * RELIEF * (1.0 - smoothstep(10.0, 14.0, dist));
      var lo = 0.0;
      var hi = 1.0;
      var found = false;
      for (var k = 1; k <= 4; k = k + 1) {
        let dpt = f32(k) * 0.25;
        if !found && 1.0 - height(pat, fc.uv + shift * dpt, fc.side) <= dpt { hi = dpt; found = true; }
        if !found { lo = dpt; }
      }
      if found {
        for (var k = 0; k < 2; k = k + 1) {
          let mid = (lo + hi) * 0.5;
          if 1.0 - height(pat, fc.uv + shift * mid, fc.side) <= mid { hi = mid; } else { lo = mid; }
        }
        uv = fc.uv + shift * hi;
        let ls = g.sun.xyz;
        let lt = vec3<f32>(dot(ls, fc.t), dot(ls, fc.bt), dot(ls, fc.n));
        if lt.z > 0.05 {
          let h0 = height(pat, uv, fc.side);
          for (var k = 1; k <= 3; k = k + 1) {
            let up = f32(k) / 3.0 * (1.0 - h0);
            if height(pat, uv + lt.xy / lt.z * RELIEF * up, fc.side) > h0 + up + 0.02 { pshadow = 0.45; }
          }
        }
      }
    }
  }
  var m = block_mat(i.b, uv, i.wp, fc.side);
  var nn = fc.n;
  if tier > 1.5 && has_relief(pat) && dist < 40.0 {
    let e = 0.5 / tex_res();
    let hu = height(pat, uv + vec2<f32>(e, 0.0), fc.side) - height(pat, uv - vec2<f32>(e, 0.0), fc.side);
    let hv = height(pat, uv + vec2<f32>(0.0, e), fc.side) - height(pat, uv - vec2<f32>(0.0, e), fc.side);
    let k = RELIEF / (2.0 * e) * (1.0 - smoothstep(25.0, 40.0, dist));
    nn = normalize(fc.n - (fc.t * hu + fc.bt * hv) * k);
  }
  var alb = to_lin(m.col);
  // Wetter auf Oberseiten: Regen macht nass (glatter, dunkler, Tropfen-Ringe), Schnee legt Frost-Schimmer
  if g.wx.y > 0.01 && fc.n.y > 0.5 {
    if abs(g.wx.x - 1.0) < 0.5 {
      m.rough = m.rough * mix(1.0, 0.25, g.wx.y);
      alb = alb * mix(1.0, 0.72, g.wx.y);
      let cell = floor(i.wp.xz * 2.0);
      let rh = h2(cell + vec2<f32>(13.0, 7.0));
      let ph = fract(g.cam.w * 0.9 + rh);
      let ctr = (cell + vec2<f32>(0.25) + vec2<f32>(rh, fract(rh * 7.3)) * 0.5) * 0.5;
      let dv = i.wp.xz - ctr;
      let r = length(dv);
      let ring = (1.0 - smoothstep(0.0, 0.03, abs(r - ph * 0.22))) * (1.0 - ph);
      let dir = dv / max(r, 0.001);
      nn = normalize(nn + vec3<f32>(dir.x, 0.0, dir.y) * ring * 0.35 * g.wx.y);
    } else if abs(g.wx.x - 2.0) < 0.5 {
      alb = mix(alb, vec3<f32>(0.86, 0.92, 1.0), 0.35 * g.wx.y);
    }
  }
  let ao = mix(1.0, 0.35 + 0.65 * i.ao, g.par.y);
  var c = light_surface(alb, nn, fc.n, i.wp, ao, m.rough, m.metal, pshadow);
  // Kristall-Glitzern: einzelne Pixel blitzen im Blickwinkel auf
  if m.metal > 0.5 && tier > 0.5 {
    let sp = h2(floor(uv * tex_res()) + floor(g.cam.w * 2.0) * 0.37);
    let rv = reflect(-v, nn);
    c = c + alb * step(0.93, sp) * (0.6 + 2.5 * pow(max(dot(rv, g.sun.xyz), 0.0), 6.0)) * (0.5 + g.sky_dir.w);
  }
  c = c + alb * m.emit;
  return vec4<f32>(fog_mix(c, i.wp), m.alpha);
}

// Feste Blöcke (alle Stufen) und durchsichtige Blöcke in Eco (direkt mit Mischung)
@fragment
fn fs(i: VO) -> @location(0) vec4<f32> {
  let c = block_color(i);
  return vec4<f32>(out_scene(c.rgb), c.a);
}

// ---------- Wasser (HDR-Stufen): Wellen-Normalen, Fresnel, Brechung, Tiefenfarbe, Schaum, Bildschirm-Spiegelung ----------

fn scene_depth_at(uv: vec2<f32>) -> f32 {
  let px = clamp(vec2<i32>(uv * g.scr.xy), vec2<i32>(0), vec2<i32>(g.scr.xy) - vec2<i32>(1));
  return textureLoad(scene_depth, px, 0).r;
}

fn ssr(wp: vec3<f32>, rd: vec3<f32>) -> vec4<f32> {
  var p = wp;
  var stp = 0.5;
  for (var k = 0; k < 12; k = k + 1) {
    p = p + rd * stp;
    stp = stp * 1.4;
    let c = g.vp * vec4<f32>(p, 1.0);
    if c.w <= 0.01 { break; }
    let ndc = c.xyz / c.w;
    let uv = vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
    if uv.x < 0.0 || uv.y < 0.0 || uv.x > 1.0 || uv.y > 1.0 { break; }
    let d = scene_depth_at(uv);
    if d < 1.0 && d < ndc.z {
      let ps = world_from(uv, d);
      if distance(ps, p) < stp * 1.2 + 0.4 {
        let edge = smoothstep(0.0, 0.1, min(min(uv.x, 1.0 - uv.x), min(uv.y, 1.0 - uv.y)));
        return vec4<f32>(dec(textureSampleLevel(scene_tex, lin_samp, uv, 0.0).rgb), edge * (1.0 - f32(k) / 14.0));
      }
    }
  }
  return vec4<f32>(0.0);
}

fn water(i: VO, dz: f32) -> vec4<f32> {
  let fc = face_of(i.a, i.wp);
  let t = g.cam.w;
  let wp = i.wp;
  let v = normalize(g.cam.xyz - wp);
  var nn = fc.n;
  if !fc.side {
    // vier Wellenzüge, Steigung analytisch (Gerstner-artig)
    var sl = vec2<f32>(0.0);
    var wd = array<vec4<f32>, 4>(
      vec4<f32>(0.96, 0.29, 1.3, 0.05), vec4<f32>(-0.37, 0.93, 1.9, 0.035),
      vec4<f32>(0.71, -0.71, 4.1, 0.012), vec4<f32>(-0.98, -0.2, 6.3, 0.008));
    for (var k = 0; k < 4; k = k + 1) {
      let w = wd[k];
      let ph = dot(w.xy, wp.xz) * w.z + t * (1.0 + w.z * 0.35);
      sl = sl + w.xy * (w.w * w.z * cos(ph));
    }
    nn = normalize(vec3<f32>(-sl.x, 1.0, -sl.y)) * fc.n.y;
  } else {
    // Wasserfall: Strähnen laufen nach unten
    let u = dot(wp, fc.t);
    let st = sin(u * 21.0 + h2(vec2<f32>(floor(u * 16.0), 1.0)) * 6.0) * 0.12 + sin((wp.y + t * 3.0) * 9.0 + u * 3.0) * 0.1;
    nn = normalize(fc.n + fc.t * st + vec3<f32>(0.0, 0.08 * sin((wp.y + t * 2.6) * 14.0), 0.0));
  }
  let suv = i.pos.xy * g.scr.zw;
  var thick = 40.0;
  if dz < 1.0 { thick = distance(world_from(suv, dz), wp); }
  // Brechung: verschobener Blick auf den Grund, aber nie auf Dinge vor dem Wasser
  var ruv = clamp(suv + nn.xz * 0.035 * clamp(thick, 0.0, 1.5) * (1.0 - f32(fc.side) * 0.5), vec2<f32>(0.001), vec2<f32>(0.999));
  var rdz = scene_depth_at(ruv);
  if rdz < i.pos.z { ruv = suv; rdz = dz; }
  var refr = dec(textureSampleLevel(scene_tex, lin_samp, ruv, 0.0).rgb);
  var rthick = thick;
  if rdz < 1.0 {
    let pg = world_from(ruv, rdz);
    rthick = distance(pg, wp);
    // Lichtnetz (Kaustik) auf dem Grund
    let cz = sin(pg.x * 2.1 + sin(pg.z * 1.7 + t) * 1.3 + t * 0.9) * sin(pg.z * 2.3 + sin(pg.x * 1.3 - t * 1.1) + t * 0.7);
    refr = refr * (1.0 + pow(abs(cz), 3.0) * 1.4 * g.sky_dir.w * exp(-rthick * 0.35));
  }
  let tint = mix(vec3<f32>(1.0), to_lin(vec3<f32>(f32(i.b & 255u), f32((i.b >> 8u) & 255u), f32((i.b >> 16u) & 255u)) / 255.0) * 2.2, 0.35);
  let absorb = exp(-rthick * vec3<f32>(0.42, 0.12, 0.075));
  let deep = vec3<f32>(0.005, 0.03, 0.085) * (g.amb.xyz * 2.0 + g.sun_col.xyz * 0.15);
  let body = mix(deep, refr * tint, absorb);
  // Spiegelung: Himmel, ab Ultra Bildschirm-Strahl
  let rd = reflect(-v, nn);
  var refl = sky(rd, true);
  if g.par.w > 1.5 {
    let hit = ssr(wp, rd);
    refl = mix(refl, hit.rgb, hit.a);
  }
  let ndv = max(dot(nn, v), 0.0);
  let fr = 0.02 + 0.98 * pow(1.0 - ndv, 5.0);
  var c = mix(body, refl, fr);
  let sh = shadow_at(wp, fc.n, max(dot(fc.n, g.sun.xyz), 0.0));
  c = c + g.sun_col.xyz * pow(max(dot(rd, g.sun.xyz), 0.0), 350.0) * 6.0 * sh;
  c = c + point_lights(wp, nn) * 0.08;
  // Schaum an flachen Rändern: Pixel-Ringe, die sich bewegen
  if thick < 0.35 && !fc.side {
    let q = floor(wp.xz * 16.0);
    let edge = 1.0 - thick / 0.35;
    let n0 = h2(q + floor(t * 3.0) * 7.0);
    let ring = 0.5 + 0.5 * sin(thick * 40.0 - t * 4.0);
    if n0 < edge * (0.55 + 0.45 * ring) {
      c = mix(c, vec3<f32>(0.92, 0.96, 1.0) * (g.amb.xyz * 1.4 + g.sun_col.xyz * 0.55 * sh), 0.85);
    }
  }
  if fc.side {
    c = c + vec3<f32>(0.9, 0.95, 1.0) * step(0.86, h2(floor(vec2<f32>(dot(wp, fc.t), wp.y + t * 6.0) * 16.0))) * 0.25 * (g.amb.xyz + g.sun_col.xyz * 0.3);
  }
  return vec4<f32>(fog_mix(c, wp), 1.0);
}

// Durchsichtige Blöcke in den HDR-Stufen: eigene Schicht, vormultipliert; Tiefentest von Hand gegen die feste Szene
@fragment
fn fs_clear_hdr(i: VO) -> @location(0) vec4<f32> {
  let dz = textureLoad(scene_depth, vec2<i32>(i.pos.xy), 0).r;
  if i.pos.z >= dz { discard; }
  let pat = (i.b >> 24u) & 15u;
  let glow = (i.b >> 28u) & 1u;
  if pat == 8u && glow == 0u {
    let w = water(i, dz);
    return vec4<f32>(enc(w.rgb), 1.0);
  }
  let c = block_color(i);
  return vec4<f32>(enc(c.rgb) * c.a, c.a);
}

// ---------- Himmel ----------

struct SO { @builtin(position) pos: vec4<f32>, @location(0) ndc: vec2<f32> };

@vertex
fn vss(@builtin(vertex_index) k: u32) -> SO {
  let p = vec2<f32>(f32((k << 1u) & 2u), f32(k & 2u)) * 2.0 - 1.0;
  var o: SO;
  o.pos = vec4<f32>(p, 1.0, 1.0);
  o.ndc = p;
  return o;
}

@fragment
fn fss(i: SO) -> @location(0) vec4<f32> {
  let w = g.ivp * vec4<f32>(i.ndc, 1.0, 1.0);
  let d = normalize(w.xyz / w.w - g.cam.xyz);
  return vec4<f32>(out_scene(sky(d, true)), 1.0);
}

// ---------- Würfel-Instanzen (Figuren, Cosmetics, Begleiter, Partikel, Wolken) ----------

struct IO {
  @builtin(position) pos: vec4<f32>,
  @location(0) n: vec3<f32>,
  @location(1) col: vec4<f32>,
  @location(2) wp: vec3<f32>,
  @location(3) lp: vec3<f32>,
  @location(4) ln: vec3<f32>,
};

// Pixel-Riss beim Abbauen: gezackte Strahlen aus der Mitte, je Stufe länger (16×16-Raster der Blockseite)
fn crack(px: vec2<f32>, stage: f32) -> bool {
  let q = px - vec2<f32>(7.5, 7.5);
  let r = length(q);
  let reach = stage * 1.25;
  if r > reach { return false; }
  let a = atan2(q.y, q.x);
  for (var k = 0; k < 6; k = k + 1) {
    let fk = f32(k);
    let base = fk * 1.0472 + (h2(vec2<f32>(fk, 3.0)) - 0.5) * 0.8;
    let wob = (h2(vec2<f32>(fk, floor(r / 2.5))) - 0.5) * 0.7;
    var d = a - base - wob;
    d = d - 6.2832 * round(d / 6.2832);
    if abs(d) * max(r, 1.0) < 0.75 && r < reach * (0.55 + 0.45 * h2(vec2<f32>(fk, 9.0))) { return true; }
  }
  return false;
}

@vertex
fn vsi(@location(0) p: vec3<f32>, @location(1) n: vec3<f32>, @location(2) m0: vec4<f32>, @location(3) m1: vec4<f32>, @location(4) m2: vec4<f32>, @location(5) col: vec4<f32>) -> IO {
  let q = vec4<f32>(p, 1.0);
  let w = vec3<f32>(dot(m0, q), dot(m1, q), dot(m2, q));
  var o: IO;
  o.pos = g.vp * vec4<f32>(w, 1.0);
  o.n = vec3<f32>(dot(m0.xyz, n), dot(m1.xyz, n), dot(m2.xyz, n));
  o.col = col;
  o.wp = w;
  o.lp = p;
  o.ln = n;
  return o;
}

// Schattenwurf der Instanzen; Riss- und Rahmen-Würfel werfen keinen (hinter die ferne Ebene geschoben)
@vertex
fn vsi_shadow(@location(0) p: vec3<f32>, @location(1) n: vec3<f32>, @location(2) m0: vec4<f32>, @location(3) m1: vec4<f32>, @location(4) m2: vec4<f32>, @location(5) col: vec4<f32>) -> @builtin(position) vec4<f32> {
  let k = u32(col.w * 255.0 + 0.5);
  if k >= 1u && k <= 9u { return vec4<f32>(0.0, 0.0, 2.0, 1.0); }
  let q = vec4<f32>(p, 1.0);
  return g.lvp * vec4<f32>(dot(m0, q), dot(m1, q), dot(m2, q), 1.0);
}

@fragment
fn fsi(i: IO) -> @location(0) vec4<f32> {
  let fw = fwidth(i.lp);
  let k = u32(i.col.w * 255.0 + 0.5);
  if k >= 1u && k <= 9u {
    let an = abs(i.ln);
    var uv = i.lp.xy;
    var du = fw.xy;
    if an.x > 0.5 { uv = i.lp.zy; du = fw.zy; } else if an.y > 0.5 { uv = i.lp.xz; du = fw.xz; }
    if k == 9u {
      // Drahtrahmen: nur die Kanten, gut 1,5 Pixel breit
      let e = vec2<f32>(0.5, 0.5) - abs(uv);
      let wd = max(du * 1.6, vec2<f32>(0.006, 0.006));
      if e.x > wd.x && e.y > wd.y { discard; }
      return vec4<f32>(out_scene(vec3<f32>(0.0)), 1.0);
    }
    if !crack(floor((uv + vec2<f32>(0.5, 0.5)) * 16.0), f32(k)) { discard; }
    return vec4<f32>(out_scene(vec3<f32>(0.004)), 1.0);
  }
  let nn = normalize(i.n);
  let alb = to_lin(i.col.xyz);
  var c = light_surface(alb, nn, nn, i.wp, 1.0, 0.8, 0.0, 1.0);
  if i.col.w > 0.5 { c = c * 0.4 + alb * 1.4; }
  return vec4<f32>(out_scene(fog_mix(c, i.wp)), 1.0);
}

// ---------- Wetter-Partikel: ohne Vertex-Puffer, alles aus Ecke (vertex_index) + Instanz + Hash ----------

struct WO {
  @builtin(position) pos: vec4<f32>,
  @location(0) col: vec4<f32>,                  // rgb beleuchtet (linear), a Deckkraft
  @location(1) q: vec2<f32>,                    // Ecke -1..1
  @location(2) wp: vec3<f32>,
  @location(3) @interpolate(flat) kind: u32,    // 1 Regen, 2 Schnee, 3 Kirschblüten
};

@vertex
fn vs_wx(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> WO {
  var o: WO;
  o.pos = vec4<f32>(0.0, 0.0, 0.0, 1.0);
  let kind = u32(clamp(g.wx.x + 0.5, 0.0, 3.0));
  let fi = f32(ii);
  // Instanzen über dem Stärke-Anteil (von 2048 = WX_PARTICLES) fallen auf Fläche 0 zusammen
  if kind == 0u || fi >= g.wx.y * 2048.0 { return o; }
  var cs = array<vec2<f32>, 6>(vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0));
  let c = cs[vi % 6u];
  let t = g.cam.w;
  let h = vec3<f32>(h2(vec2<f32>(fi, 1.37)), h2(vec2<f32>(fi, 7.11)), h2(vec2<f32>(fi, 3.73)));
  let hk = h2(vec2<f32>(fi, 5.29));
  let bx = vec3<f32>(32.0, 24.0, 32.0);
  var seed = h * bx;
  var ax = vec3<f32>(0.0, -1.0, 0.0);
  var hs = vec2<f32>(0.05, 0.05); // halbe Breite, halbe Länge
  var alb = vec3<f32>(1.0);
  var alpha = 0.95;
  if kind == 1u {
    // Sommerregen: schräge Striche, gut 22 Blöcke/s
    let vel = vec3<f32>(3.0, -22.0, 1.5) * (0.85 + 0.3 * hk);
    seed = seed + vel * t;
    ax = normalize(vel);
    hs = vec2<f32>(0.018, 0.4);
    alb = vec3<f32>(0.62, 0.70, 0.82);
    alpha = 0.45;
  } else if kind == 2u {
    // Schnee: kleine Würfelchen, die schaukelnd sinken
    seed = seed + vec3<f32>(sin(t * 1.3 + hk * 6.28) * 0.5 + t * 0.4, -t * (1.2 + 0.6 * hk), cos(t * 1.1 + hk * 9.0) * 0.4);
    hs = vec2<f32>(0.045, 0.045);
    alb = vec3<f32>(0.95, 0.97, 1.0);
  } else {
    // Kirschblüten: rosa Plättchen im Sinus-Wind
    seed = seed + vec3<f32>(sin(t * 0.7 + h.y * 0.25 + hk * 6.28) * 1.6 + t * 1.4, -t * (0.9 + 0.4 * hk), cos(t * 0.5 + hk * 4.0) * 0.8);
    hs = vec2<f32>(0.07, 0.05);
    alb = to_lin(vec3<f32>(1.0, 0.72, 0.82));
  }
  // Kasten 32×24×32 um die Kamera, die Welt-Lage bleibt beim Gehen stehen
  let rel = fract((seed - g.cam.xyz) / bx) * bx;
  let p = g.cam.xyz - bx * 0.5 + rel;
  let e = min(rel, bx - rel);
  let fade = smoothstep(0.0, 3.0, min(e.x, min(e.y, e.z)));
  let tc = g.cam.xyz - p;
  let dist = length(tc);
  let vdir = tc / max(dist, 0.001);
  var right = cross(vec3<f32>(0.0, 1.0, 0.0), vdir);
  if dot(right, right) < 0.0001 { right = vec3<f32>(1.0, 0.0, 0.0); }
  right = normalize(right);
  var up = cross(vdir, right);
  if kind == 1u {
    right = cross(ax, vdir);
    if dot(right, right) < 0.0001 { right = vec3<f32>(1.0, 0.0, 0.0); }
    right = normalize(right);
    up = ax;
  } else {
    // drehen; Blüten flattern zusätzlich (eine Achse schrumpft)
    let a = t * (1.5 + hk) + hk * 6.28;
    let r2 = right * cos(a) + up * sin(a);
    up = up * cos(a) - right * sin(a);
    if kind == 3u { up = up * (0.35 + 0.65 * abs(sin(t * 2.3 + hk * 5.0))); }
    right = r2;
  }
  let wp = p + right * (c.x * hs.x) + up * (c.y * hs.y);
  let lit = g.amb.xyz * 1.2 + g.gnd.xyz * 0.3 + g.sun_col.xyz * max(g.sun.y, 0.0) * 0.6 + point_lights(p, vdir) * 0.5;
  o.pos = g.vp * vec4<f32>(wp, 1.0);
  o.col = vec4<f32>(alb * lit, alpha * fade * smoothstep(0.3, 1.5, dist));
  o.q = c;
  o.wp = wp;
  o.kind = kind;
  return o;
}

@fragment
fn fs_wx(i: WO) -> @location(0) vec4<f32> {
  var a = i.col.a;
  if i.kind == 1u {
    a = a * (1.0 - abs(i.q.x)) * (1.0 - i.q.y * i.q.y * 0.6);
  } else if i.kind == 3u {
    if dot(i.q, i.q) > 1.0 { discard; }
  }
  if a < 0.003 { discard; }
  return vec4<f32>(out_scene(fog_mix(i.col.rgb, i.wp)), a);
}
