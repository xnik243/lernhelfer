// Gemeinsamer Teil aller Shader: Umgebung (eine Uniform, nur vec4/mat4), Schattenkarte, Himmel, Licht, Ausgabe.
struct Env {
  vp: mat4x4<f32>,
  ivp: mat4x4<f32>,
  lvp: mat4x4<f32>,      // Welt → Schattenkarte
  cam: vec4<f32>,        // xyz Auge, w Zeit in s
  sun: vec4<f32>,        // xyz Richtung zum Hauptlicht (Sonne oder Mond), w Schattenstärke 0..1
  sun_col: vec4<f32>,    // rgb Lichtfarbe linear (HDR), w Belichtung
  sky_dir: vec4<f32>,    // xyz Richtung zur Sonne (Scheibe; Mond gegenüber), w Tag 0..1
  zen: vec4<f32>,        // rgb Zenit, w Sterne 0..1
  hor: vec4<f32>,        // rgb Horizont/Nebel, w Sichtweite
  amb: vec4<f32>,        // rgb Himmelslicht, w Abendrot 0..1
  gnd: vec4<f32>,        // rgb Bodenlicht, w Anzahl Punktlichter
  par: vec4<f32>,        // x Ziel ist sRGB, y Eckenschatten an, z unter Wasser, w Stufe 0 Eco .. 3 Extrem
  par2: vec4<f32>,       // x HDR in 8 Bit gepackt, y direkt aufs Bild (Eco), z 1/Schattenkartengröße, w Texturauflösung (negativ = eigenes Paket)
  scr: vec4<f32>,        // x Breite, y Höhe, z 1/Breite, w 1/Höhe (Pixel)
  lpos: array<vec4<f32>, 32>, // xyz Ort, w Reichweite
  lcol: array<vec4<f32>, 32>, // rgb Farbe × Stärke, w frei
};
@group(0) @binding(0) var<uniform> g: Env;
@group(0) @binding(1) var shadow_map: texture_depth_2d;
@group(0) @binding(2) var shadow_cmp: sampler_comparison;

const PI: f32 = 3.14159265;

fn h2(p: vec2<f32>) -> f32 {
  var q = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
  q = q + dot(q, q.yzx + 33.33);
  return fract((q.x + q.y) * q.z);
}

fn h3(p: vec3<f32>) -> f32 {
  var q = fract(p * 0.1031);
  q = q + dot(q, q.zyx + 31.32);
  return fract((q.x + q.y) * q.z);
}

fn noise3(p: vec3<f32>) -> f32 {
  let i = floor(p);
  let f = fract(p);
  let u = f * f * (3.0 - 2.0 * f);
  let a = mix(mix(h3(i), h3(i + vec3<f32>(1.0, 0.0, 0.0)), u.x), mix(h3(i + vec3<f32>(0.0, 1.0, 0.0)), h3(i + vec3<f32>(1.0, 1.0, 0.0)), u.x), u.y);
  let b = mix(mix(h3(i + vec3<f32>(0.0, 0.0, 1.0)), h3(i + vec3<f32>(1.0, 0.0, 1.0)), u.x), mix(h3(i + vec3<f32>(0.0, 1.0, 1.0)), h3(i + vec3<f32>(1.0, 1.0, 1.0)), u.x), u.y);
  return mix(a, b, u.z);
}

fn to_lin(c: vec3<f32>) -> vec3<f32> {
  return pow(max(c, vec3<f32>(0.0)), vec3<f32>(2.2));
}

fn luma(c: vec3<f32>) -> f32 {
  return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

// Himmel in Richtung d (linear, HDR). disc = Sonnen-/Mondscheibe und Sterne mitzeichnen (nicht im Nebel).
fn sky(d: vec3<f32>, disc: bool) -> vec3<f32> {
  if g.par.z > 0.5 { return g.hor.xyz; }
  let up = clamp(d.y, -1.0, 1.0);
  var c = mix(g.hor.xyz, g.zen.xyz, pow(max(up, 0.0), 0.5));
  // unten: dunstige Leere unter den Inseln
  c = mix(c, g.hor.xyz * 0.6 + g.zen.xyz * 0.2, clamp(-up * 2.5, 0.0, 1.0));
  let sd = g.sky_dir.xyz;
  let dusk = g.amb.w;
  let s = max(dot(d, sd), 0.0);
  let vis = smoothstep(-0.12, 0.02, sd.y);
  let glow = mix(vec3<f32>(1.0, 0.86, 0.62), vec3<f32>(1.0, 0.42, 0.14), dusk);
  // Hof um die Sonne; bei Untergang breit und rot über dem Horizont
  c = c + glow * vis * (pow(s, 48.0) * 0.5 + pow(s, 6.0) * 0.35 * dusk) * (0.4 + 0.6 * g.sky_dir.w + dusk);
  c = c + glow * dusk * 0.25 * pow(1.0 - abs(up), 6.0) * (0.5 + 0.5 * s);
  if disc {
    c = c + glow * smoothstep(0.9990, 0.9996, s) * 22.0 * vis;
    let m = max(dot(d, -sd), 0.0);
    let mvis = smoothstep(-0.05, 0.05, -sd.y);
    c = c + vec3<f32>(0.85, 0.92, 1.1) * (smoothstep(0.9993, 0.9997, m) * 4.0 + pow(m, 80.0) * 0.12) * mvis;
    if g.zen.w > 0.01 && up > 0.0 {
      let cell = floor(d * 260.0);
      let hs = h3(cell);
      let star = step(0.9978, hs) * (0.55 + 0.45 * sin(g.cam.w * 2.5 + hs * 400.0));
      c = c + vec3<f32>(star * 3.0 * g.zen.w * smoothstep(0.0, 0.25, up));
    }
  }
  return c;
}

fn fog_mix(c: vec3<f32>, wp: vec3<f32>) -> vec3<f32> {
  let d = distance(wp, g.cam.xyz);
  let f = clamp((d - g.hor.w * 0.55) / (g.hor.w * 0.45), 0.0, 1.0);
  return mix(c, sky(normalize(wp - g.cam.xyz), false), f * f);
}

fn aces(x: vec3<f32>) -> vec3<f32> {
  return clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), vec3<f32>(0.0), vec3<f32>(1.0));
}

// linear HDR → Bildschirm (Belichtung, ACES, Gamma falls das Ziel kein sRGB ist)
fn present(c: vec3<f32>) -> vec3<f32> {
  var o = aces(c * g.sun_col.w);
  if g.par.x < 0.5 { o = pow(o, vec3<f32>(1.0 / 2.2)); }
  return o;
}

// HDR in 8-Bit-Zielen: Bereich zusammendrücken / wieder aufziehen
fn enc(c: vec3<f32>) -> vec3<f32> {
  if g.par2.x > 0.5 { return c / (vec3<f32>(1.0) + c); }
  return c;
}
fn dec(c: vec3<f32>) -> vec3<f32> {
  if g.par2.x > 0.5 { return c / max(vec3<f32>(1.0) - c, vec3<f32>(0.002)); }
  return c;
}
fn out_scene(c: vec3<f32>) -> vec3<f32> {
  if g.par2.y > 0.5 { return present(c); }
  return enc(c);
}

// Bildschirm (uv 0..1, y nach unten) + Tiefe → Welt
fn world_from(uv: vec2<f32>, d: f32) -> vec3<f32> {
  let w = g.ivp * vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, d, 1.0);
  return w.xyz / w.w;
}

// Schattenkarte: Hoch 4 Abgriffe, Ultra/Extrem 9 gedrehte Poisson-Abgriffe
fn shadow_at(wp: vec3<f32>, n: vec3<f32>, ndl: f32) -> f32 {
  if g.par.w < 0.5 || g.sun.w < 0.01 { return 1.0; }
  let texel = g.par2.z;
  let p4 = g.lvp * vec4<f32>(wp + n * (0.025 + 0.05 * (1.0 - ndl)), 1.0);
  let p = p4.xyz / p4.w;
  let uv = vec2<f32>(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5);
  if uv.x < 0.0 || uv.y < 0.0 || uv.x > 1.0 || uv.y > 1.0 || p.z > 1.0 { return 1.0; }
  let z = p.z - 0.0004;
  var s = 0.0;
  if g.par.w < 1.5 {
    for (var k = 0u; k < 4u; k = k + 1u) {
      let o = vec2<f32>(f32(k & 1u) - 0.5, f32(k >> 1u) - 0.5) * texel;
      s = s + textureSampleCompareLevel(shadow_map, shadow_cmp, uv + o, z);
    }
    s = s * 0.25;
  } else {
    var pd = array<vec2<f32>, 9>(
      vec2<f32>(0.0, 0.0), vec2<f32>(-0.94, -0.40), vec2<f32>(0.95, -0.77), vec2<f32>(-0.09, -0.93), vec2<f32>(0.34, 0.29),
      vec2<f32>(-0.92, 0.46), vec2<f32>(0.54, 0.90), vec2<f32>(-0.38, 0.92), vec2<f32>(0.97, 0.18));
    let a = h2(floor(wp.xz * 16.0) + floor(wp.y * 16.0)) * 6.2832;
    let cs = cos(a);
    let sn = sin(a);
    for (var k = 0u; k < 9u; k = k + 1u) {
      let q = pd[k];
      let o = vec2<f32>(q.x * cs - q.y * sn, q.x * sn + q.y * cs) * texel * 1.6;
      s = s + textureSampleCompareLevel(shadow_map, shadow_cmp, uv + o, z);
    }
    s = s / 9.0;
  }
  // zum Rand der Karte hin weich auslaufen
  let e = min(min(uv.x, 1.0 - uv.x), min(uv.y, 1.0 - uv.y));
  s = mix(1.0, s, smoothstep(0.0, 0.06, e));
  return mix(1.0, s, g.sun.w);
}

// farbige Punktlichter (Laternen, Portale, Kristalle), nach Entfernung sortiert vom Spiel
fn point_lights(wp: vec3<f32>, n: vec3<f32>) -> vec3<f32> {
  var acc = vec3<f32>(0.0);
  let cnt = min(u32(g.gnd.w), 32u);
  for (var k = 0u; k < cnt; k = k + 1u) {
    let lp = g.lpos[k];
    let l = lp.xyz - wp;
    let d = length(l);
    if d < lp.w {
      let x = d / lp.w;
      let fall = (1.0 - x * x) * (1.0 - x * x) / (1.0 + 0.35 * d * d);
      let ndl = max(dot(n, l / max(d, 0.001)), 0.0) * 0.85 + 0.15;
      acc = acc + g.lcol[k].xyz * fall * ndl;
    }
  }
  return acc;
}

// Beleuchtung einer Fläche: Sonne/Mond mit Schatten, Himmels-/Bodenlicht, Punktlichter, ab Hoch GGX-Glanz + Himmelsspiegelung
fn light_surface(alb: vec3<f32>, nn: vec3<f32>, ng: vec3<f32>, wp: vec3<f32>, ao: f32, rough: f32, metal: f32, pshadow: f32) -> vec3<f32> {
  let v = normalize(g.cam.xyz - wp);
  let l = g.sun.xyz;
  let ndl_g = dot(ng, l);
  let ndl = max(dot(nn, l), 0.0);
  var sh = 0.0;
  if ndl_g > 0.0 { sh = shadow_at(wp, ng, ndl_g) * pshadow; }
  let direct = g.sun_col.xyz * ndl * sh;
  let hemi = mix(g.gnd.xyz, g.amb.xyz, nn.y * 0.5 + 0.5) * ao;
  let pl = point_lights(wp, nn) * (0.55 + 0.45 * ao);
  var c = alb * (1.0 - metal * 0.85) * (direct + hemi + pl);
  if g.par.w > 0.5 {
    let f0 = mix(vec3<f32>(0.04), alb, metal);
    let hv = normalize(l + v);
    let ndh = max(dot(nn, hv), 0.0);
    let a = max(rough * rough, 0.02);
    let a2 = a * a;
    let dd = ndh * ndh * (a2 - 1.0) + 1.0;
    let dist = a2 / (PI * dd * dd);
    let ldh = max(dot(l, hv), 0.0);
    let fr = f0 + (vec3<f32>(1.0) - f0) * pow(1.0 - ldh, 5.0);
    c = c + min(dist, 60.0) * fr * 0.25 / max(ldh * ldh, 0.1) * direct;
    let ndv = max(dot(nn, v), 0.0);
    let fv = f0 + (vec3<f32>(1.0) - f0) * pow(1.0 - ndv, 5.0) * (1.0 - rough);
    c = c + fv * sky(reflect(-v, nn), false) * ao * (1.0 - rough * 0.8) * 0.8;
  }
  return c;
}
