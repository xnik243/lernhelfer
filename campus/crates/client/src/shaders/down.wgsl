// Verkleinern und Bloom: Szene halb (für Tiefenunschärfe), helle Stellen auf ein Viertel, Kawase-Weichzeichner auf ein Achtel.
@group(1) @binding(0) var src_a: texture_2d<f32>;
@group(1) @binding(1) var src_b: texture_2d<f32>;
@group(1) @binding(2) var smp: sampler;

struct FO { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };

@vertex
fn vs_full(@builtin(vertex_index) k: u32) -> FO {
  let p = vec2<f32>(f32((k << 1u) & 2u), f32(k & 2u)) * 2.0 - 1.0;
  var o: FO;
  o.pos = vec4<f32>(p, 0.0, 1.0);
  o.uv = vec2<f32>(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5);
  return o;
}

// feste Szene (a) mit durchsichtiger Schicht (b, vormultipliert) darüber
fn comp(uv: vec2<f32>) -> vec3<f32> {
  let a = dec(textureSampleLevel(src_a, smp, uv, 0.0).rgb);
  let b = textureSampleLevel(src_b, smp, uv, 0.0);
  return dec(b.rgb) + a * (1.0 - b.a);
}

@fragment
fn fs_down(i: FO) -> @location(0) vec4<f32> {
  let px = 1.0 / vec2<f32>(textureDimensions(src_a));
  let c = comp(i.uv + px * vec2<f32>(-1.0, -1.0)) + comp(i.uv + px * vec2<f32>(1.0, -1.0)) + comp(i.uv + px * vec2<f32>(-1.0, 1.0)) + comp(i.uv + px * vec2<f32>(1.0, 1.0));
  return vec4<f32>(enc(c * 0.25), 1.0);
}

// Zelt-Filter mit 5 Abgriffen, nur was heller als die Schwelle ist (weiches Knie)
@fragment
fn fs_bright(i: FO) -> @location(0) vec4<f32> {
  let px = 1.0 / vec2<f32>(textureDimensions(src_a));
  var c = dec(textureSampleLevel(src_a, smp, i.uv, 0.0).rgb) * 0.4;
  c = c + dec(textureSampleLevel(src_a, smp, i.uv + px * vec2<f32>(-1.0, -1.0), 0.0).rgb) * 0.15;
  c = c + dec(textureSampleLevel(src_a, smp, i.uv + px * vec2<f32>(1.0, -1.0), 0.0).rgb) * 0.15;
  c = c + dec(textureSampleLevel(src_a, smp, i.uv + px * vec2<f32>(-1.0, 1.0), 0.0).rgb) * 0.15;
  c = c + dec(textureSampleLevel(src_a, smp, i.uv + px * vec2<f32>(1.0, 1.0), 0.0).rgb) * 0.15;
  // nach Belichtung: nur was wirklich überstrahlt; Schwelle 1,4 mit weichem Knie 0,3
  let l = luma(c) * g.sun_col.w;
  let th = 1.4;
  let knee = th * 0.3;
  var soft = clamp(l - th + knee, 0.0, 2.0 * knee);
  soft = soft * soft / (4.0 * knee + 0.0001);
  let w = max(soft, l - th) / max(l, 0.0001);
  return vec4<f32>(enc(min(c * w, vec3<f32>(40.0))), 1.0);
}

@fragment
fn fs_blur(i: FO) -> @location(0) vec4<f32> {
  let px = 1.5 / vec2<f32>(textureDimensions(src_a));
  var c = dec(textureSampleLevel(src_a, smp, i.uv, 0.0).rgb) * 0.2;
  c = c + dec(textureSampleLevel(src_a, smp, i.uv + px * vec2<f32>(-1.0, -1.0), 0.0).rgb) * 0.2;
  c = c + dec(textureSampleLevel(src_a, smp, i.uv + px * vec2<f32>(1.0, -1.0), 0.0).rgb) * 0.2;
  c = c + dec(textureSampleLevel(src_a, smp, i.uv + px * vec2<f32>(-1.0, 1.0), 0.0).rgb) * 0.2;
  c = c + dec(textureSampleLevel(src_a, smp, i.uv + px * vec2<f32>(1.0, 1.0), 0.0).rgb) * 0.2;
  return vec4<f32>(enc(c), 1.0);
}
