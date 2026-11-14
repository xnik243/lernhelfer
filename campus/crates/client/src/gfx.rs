//! Grafik über wgpu: WebGPU, sonst automatisch WebGL 2. Vier Stufen:
//! 0 Eco: ein Durchgang direkt aufs Bild (Himmel, Blöcke, Würfel, Durchsichtiges), keine Zusatztexturen.
//! 1 Hoch / 2 Ultra / 3 Extrem: Schattenkarte → HDR-Szene → durchsichtige Schicht (Wasser liest Szene + Tiefe)
//! → halb/Viertel/Achtel (Bloom, Unschärfe) → Abschluss (AO, Nebel, Strahlen, ACES) aufs Bild.
//! Alle Puffer, Texturen und Bindungen entstehen nur bei Größen- oder Stufenwechsel, nie in `render`.
use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

pub const MAX_QUADS: usize = 1 << 15; // je Stück
pub const MAX_INST: usize = 8192;
pub const MAX_LIGHTS: usize = 32;
const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// Eine Uniform für alles (nur vec4/mat4, Belegung der Kanäle siehe shaders/common.wgsl).
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Env {
    pub vp: [[f32; 4]; 4],
    pub ivp: [[f32; 4]; 4],
    pub lvp: [[f32; 4]; 4],          // Welt → Schattenkarte
    pub cam: [f32; 4],               // xyz Auge, w Zeit
    pub sun: [f32; 4],               // xyz zum Hauptlicht, w Schattenstärke
    pub sun_col: [f32; 4],           // rgb Lichtfarbe, w Belichtung
    pub sky_dir: [f32; 4],           // xyz zur Sonne, w Tag 0..1
    pub zen: [f32; 4],               // rgb Zenit, w Sterne
    pub hor: [f32; 4],               // rgb Horizont/Nebel, w Sichtweite
    pub amb: [f32; 4],               // rgb Himmelslicht, w Abendrot
    pub gnd: [f32; 4],               // rgb Bodenlicht, w Anzahl Punktlichter
    pub par: [f32; 4],               // x sRGB-Ziel, y Eckenschatten, z unter Wasser, w Stufe (setzt Gfx)
    pub par2: [f32; 4],              // x HDR 8 Bit, y direkt, z 1/Schattengröße (setzt Gfx), w Texturauflösung, negativ = eigenes Paket (setzt Gfx)
    pub scr: [f32; 4],               // x Breite, y Höhe, z 1/Breite, w 1/Höhe (setzt Gfx)
    pub lpos: [[f32; 4]; MAX_LIGHTS], // xyz Ort, w Reichweite
    pub lcol: [[f32; 4]; MAX_LIGHTS], // rgb Farbe × Stärke
}

/// Eine Würfel-Instanz: 3×4-Matrix (zeilenweise) und Farbe; Alpha > 127 = leuchtet, 1..8 = Riss-Stufe, 9 = Drahtrahmen.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Inst {
    pub m: [[f32; 4]; 3],
    pub col: [u8; 4],
}

#[derive(Default)]
pub struct ChunkGpu {
    buf: Option<wgpu::Buffer>,
    cap: u64,
    pub solid: u32, // Rechtecke
    pub clear: u32,
}

const COMMON: &str = include_str!("shaders/common.wgsl");
const SCENE: &str = include_str!("shaders/scene.wgsl");
const DOWN: &str = include_str!("shaders/down.wgsl");
const FINAL: &str = include_str!("shaders/final.wgsl");

/// Offscreen-Ziele einer Stufe und Größe samt Bindungen.
struct PostProcessTargets {
    key: (u32, u32, u8),
    shadow_size: u32,
    shadow: wgpu::TextureView,
    depth: wgpu::TextureView,
    scene: wgpu::TextureView,
    clear: wgpu::TextureView,
    half: wgpu::TextureView,
    q1: wgpu::TextureView,
    q2: wgpu::TextureView,
    bind_main: wgpu::BindGroup,
    bind_clear: wgpu::BindGroup,
    bind_down: wgpu::BindGroup,
    bind_b1: wgpu::BindGroup,
    bind_b2: wgpu::BindGroup,
    bind_post: wgpu::BindGroup,
}

struct Pipes {
    sky: wgpu::RenderPipeline,
    solid: wgpu::RenderPipeline,
    inst: wgpu::RenderPipeline,
    clear: wgpu::RenderPipeline,
}

pub struct Gfx {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    direct: Pipes,
    hdr: Pipes,
    sh_solid: wgpu::RenderPipeline,
    sh_inst: wgpu::RenderPipeline,
    p_down: wgpu::RenderPipeline,
    p_bright: wgpu::RenderPipeline,
    p_blur: wgpu::RenderPipeline,
    p_final: wgpu::RenderPipeline,
    bgl_main: wgpu::BindGroupLayout,
    bgl_clear: wgpu::BindGroupLayout,
    bgl_two: wgpu::BindGroupLayout,
    bgl_post: wgpu::BindGroupLayout,
    bind_env: wgpu::BindGroup,
    samp_lin: wgpu::Sampler,
    samp_cmp: wgpu::Sampler,
    env: wgpu::Buffer,
    t: PostProcessTargets,
    idx: wgpu::Buffer,
    cube: wgpu::Buffer,
    inst: wgpu::Buffer,
    samp_pack: wgpu::Sampler,
    pack: wgpu::TextureView, // Texturpaket (4×4 Kacheln) oder 1×1 weiß
    pack_on: bool,
    tex_res: f32,
    hdr_fmt: wgpu::TextureFormat,
    hdr8: bool,
    tier: u8,
    pub webgpu: bool,
    pub srgb: bool,
    pub adapter_name: String,
}

const VOXEL_ATTRS: [wgpu::VertexAttribute; 2] = wgpu::vertex_attr_array![0 => Uint32, 1 => Uint32];
const CUBE_ATTRS: [wgpu::VertexAttribute; 2] = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3];
const INST_ATTRS: [wgpu::VertexAttribute; 4] = wgpu::vertex_attr_array![2 => Float32x4, 3 => Float32x4, 4 => Float32x4, 5 => Unorm8x4];

fn tex(device: &wgpu::Device, w: u32, h: u32, format: wgpu::TextureFormat) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d { width: w.max(1), height: h.max(1), depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&Default::default())
}

fn bgl(device: &wgpu::Device, entries: &[(wgpu::ShaderStages, wgpu::BindingType)]) -> wgpu::BindGroupLayout {
    let e: Vec<wgpu::BindGroupLayoutEntry> = entries.iter().enumerate().map(|(i, &(visibility, ty))| wgpu::BindGroupLayoutEntry { binding: i as u32, visibility, ty, count: None }).collect();
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: None, entries: &e })
}

fn bind(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, res: &[wgpu::BindingResource]) -> wgpu::BindGroup {
    let e: Vec<wgpu::BindGroupEntry> = res.iter().enumerate().map(|(i, r)| wgpu::BindGroupEntry { binding: i as u32, resource: r.clone() }).collect();
    device.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout, entries: &e })
}

/// RGBA8-Textur (Farben im sRGB-Raum wie die Blockfarben) mit Inhalt.
fn rgba_tex(device: &wgpu::Device, queue: &wgpu::Queue, rgba: &[u8], w: u32, h: u32) -> wgpu::TextureView {
    let desc = wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    };
    device.create_texture_with_data(queue, &desc, wgpu::util::TextureDataOrder::LayerMajor, rgba).create_view(&Default::default())
}

const fn tex_float(filterable: bool) -> wgpu::BindingType {
    wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false }
}

impl Gfx {
    pub async fn new(canvas: web_sys::HtmlCanvasElement, w: u32, h: u32) -> Result<Gfx, String> {
        // WebGPU, wenn der Browser es wirklich kann; sonst fällt wgpu selbst auf WebGL 2 zurück
        let instance = wgpu::util::new_instance_with_webgpu_detection(&wgpu::InstanceDescriptor { backends: wgpu::Backends::BROWSER_WEBGPU | wgpu::Backends::GL, ..Default::default() }).await;
        let surface = instance.create_surface(wgpu::SurfaceTarget::Canvas(canvas)).map_err(|e| format!("Zeichenfläche: {e}"))?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions { power_preference: wgpu::PowerPreference::HighPerformance, compatible_surface: Some(&surface), force_fallback_adapter: false })
            .await
            .ok_or("Dieser Browser bietet weder WebGPU noch WebGL 2.")?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor { label: None, required_features: wgpu::Features::empty(), required_limits: wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits()), memory_hints: wgpu::MemoryHints::Performance }, None)
            .await
            .map_err(|e| format!("Grafikgerät: {e}"))?;
        let caps = surface.get_capabilities(&adapter);
        let format = caps.formats[0];
        let config = wgpu::SurfaceConfiguration { usage: wgpu::TextureUsages::RENDER_ATTACHMENT, format, width: w, height: h, present_mode: wgpu::PresentMode::Fifo, alpha_mode: caps.alpha_modes[0], view_formats: vec![], desired_maximum_frame_latency: 2 };
        surface.configure(&device, &config);

        // HDR-Ziel: Rgba16Float, wenn darstellbar und filterbar; sonst Rgba8Unorm mit zusammengedrücktem Bereich
        let ff = adapter.get_texture_format_features(wgpu::TextureFormat::Rgba16Float);
        let ok16 = ff.allowed_usages.contains(wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING) && ff.flags.contains(wgpu::TextureFormatFeatureFlags::FILTERABLE);
        let (hdr_fmt, hdr8) = if ok16 { (wgpu::TextureFormat::Rgba16Float, false) } else { (wgpu::TextureFormat::Rgba8Unorm, true) };

        let module = |src: &str| device.create_shader_module(wgpu::ShaderModuleDescriptor { label: None, source: wgpu::ShaderSource::Wgsl(format!("{COMMON}\n{src}").into()) });
        let (m_scene, m_down, m_final) = (module(SCENE), module(DOWN), module(FINAL));
        let env = device.create_buffer(&wgpu::BufferDescriptor { label: None, size: std::mem::size_of::<Env>() as u64, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let vf = wgpu::ShaderStages::VERTEX_FRAGMENT;
        let fr = wgpu::ShaderStages::FRAGMENT;
        let uni = wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None };
        let lin = wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering);
        let bgl_env = bgl(&device, &[(vf, uni)]);
        let bgl_main = bgl(&device, &[
            (vf, uni),
            (fr, wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Depth, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false }),
            (fr, wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison)),
            (fr, tex_float(true)), // Texturpaket
            (fr, lin),
        ]);
        let bgl_clear = bgl(&device, &[(fr, tex_float(true)), (fr, tex_float(false)), (fr, lin)]);
        let bgl_two = bgl(&device, &[(fr, tex_float(true)), (fr, tex_float(true)), (fr, lin)]);
        let bgl_post = bgl(&device, &[(fr, tex_float(true)), (fr, tex_float(true)), (fr, tex_float(false)), (fr, tex_float(true)), (fr, tex_float(true)), (fr, tex_float(true)), (fr, lin)]);
        let bind_env = bind(&device, &bgl_env, &[env.as_entire_binding()]);
        let layout = |l: &[&wgpu::BindGroupLayout]| device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: l, push_constant_ranges: &[] });
        let (l_env, l_main, l_clear, l_two, l_post) = (layout(&[&bgl_env]), layout(&[&bgl_main]), layout(&[&bgl_main, &bgl_clear]), layout(&[&bgl_main, &bgl_two]), layout(&[&bgl_main, &bgl_post]));

        let voxel = [wgpu::VertexBufferLayout { array_stride: 8, step_mode: wgpu::VertexStepMode::Vertex, attributes: &VOXEL_ATTRS }];
        let boxes = [
            wgpu::VertexBufferLayout { array_stride: 24, step_mode: wgpu::VertexStepMode::Vertex, attributes: &CUBE_ATTRS },
            wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<Inst>() as u64, step_mode: wgpu::VertexStepMode::Instance, attributes: &INST_ATTRS },
        ];
        let ds = |write: bool, cmp: wgpu::CompareFunction, bias: wgpu::DepthBiasState| Some(wgpu::DepthStencilState { format: DEPTH, depth_write_enabled: write, depth_compare: cmp, stencil: Default::default(), bias });
        let nb = wgpu::DepthBiasState::default();
        #[allow(clippy::too_many_arguments)]
        let pipe = |m: &wgpu::ShaderModule, l: &wgpu::PipelineLayout, vs: &str, fs: &str, buffers: &[wgpu::VertexBufferLayout], fmt: Option<wgpu::TextureFormat>, blend: Option<wgpu::BlendState>, depth: Option<wgpu::DepthStencilState>, cull: Option<wgpu::Face>| {
            let targets = [fmt.map(|format| wgpu::ColorTargetState { format, blend, write_mask: wgpu::ColorWrites::ALL })];
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(l),
                vertex: wgpu::VertexState { module: m, entry_point: Some(vs), buffers, compilation_options: Default::default() },
                fragment: Some(wgpu::FragmentState { module: m, entry_point: Some(fs), targets: if fmt.is_some() { &targets[..] } else { &[] }, compilation_options: Default::default() }),
                primitive: wgpu::PrimitiveState { cull_mode: cull, ..Default::default() },
                depth_stencil: depth,
                multisample: Default::default(),
                multiview: None,
                cache: None,
            })
        };
        let back = Some(wgpu::Face::Back);
        let rep = Some(wgpu::BlendState::REPLACE);
        let less = wgpu::CompareFunction::Less;
        let scene_pipes = |fmt: wgpu::TextureFormat, direct: bool| Pipes {
            sky: pipe(&m_scene, &l_main, "vss", "fss", &[], Some(fmt), None, ds(false, wgpu::CompareFunction::Always, nb), None),
            solid: pipe(&m_scene, &l_main, "vs", "fs", &voxel, Some(fmt), rep, ds(true, less, nb), back),
            inst: pipe(&m_scene, &l_main, "vsi", "fsi", &boxes, Some(fmt), rep, ds(true, less, nb), back),
            clear: if direct {
                pipe(&m_scene, &l_main, "vs", "fs", &voxel, Some(fmt), Some(wgpu::BlendState::ALPHA_BLENDING), ds(false, less, nb), back)
            } else {
                pipe(&m_scene, &l_clear, "vs", "fs_clear_hdr", &voxel, Some(fmt), Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING), None, back)
            },
        };
        let direct = scene_pipes(format, true);
        let hdr = scene_pipes(hdr_fmt, false);
        // Schatten: nur Tiefe, beide Seiten, Neigungs-Versatz gegen Schattenakne
        let sb = wgpu::DepthBiasState { constant: 2, slope_scale: 1.5, clamp: 0.0 };
        let sh_solid = pipe(&m_scene, &l_env, "vs_shadow", "fs_none", &voxel, None, None, ds(true, less, sb), None);
        let sh_inst = pipe(&m_scene, &l_env, "vsi_shadow", "fs_none", &boxes, None, None, ds(true, less, sb), None);
        let p_down = pipe(&m_down, &l_two, "vs_full", "fs_down", &[], Some(hdr_fmt), None, None, None);
        let p_bright = pipe(&m_down, &l_two, "vs_full", "fs_bright", &[], Some(hdr_fmt), None, None, None);
        let p_blur = pipe(&m_down, &l_two, "vs_full", "fs_blur", &[], Some(hdr_fmt), None, None, None);
        let p_final = pipe(&m_final, &l_post, "vs_full", "fs_final", &[], Some(format), None, None, None);

        let clamp = wgpu::AddressMode::ClampToEdge;
        let samp_lin = device.create_sampler(&wgpu::SamplerDescriptor { address_mode_u: clamp, address_mode_v: clamp, mag_filter: wgpu::FilterMode::Linear, min_filter: wgpu::FilterMode::Linear, ..Default::default() });
        let samp_cmp = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: clamp,
            address_mode_v: clamp,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });
        // Texturpaket: Pixel scharf, ohne Paket 1×1 weiß
        let near = wgpu::FilterMode::Nearest;
        let samp_pack = device.create_sampler(&wgpu::SamplerDescriptor { address_mode_u: clamp, address_mode_v: clamp, mag_filter: near, min_filter: near, ..Default::default() });
        let pack = rgba_tex(&device, &queue, &[255u8; 4], 1, 1);

        // ein Indexpuffer für alle Rechtecke: 0 1 2, 0 2 3 – immer wieder
        let indices: Vec<u32> = (0..MAX_QUADS as u32).flat_map(|q| [0, 1, 2, 0, 2, 3].map(|k| q * 4 + k)).collect();
        let idx = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: bytemuck::cast_slice(&indices), usage: wgpu::BufferUsages::INDEX });
        // Einheitswürfel, dieselbe Eckenfolge wie beim Meshing
        let mut cube: Vec<[f32; 6]> = Vec::with_capacity(24);
        for d in 0..3usize {
            let (u, v) = ((d + 1) % 3, (d + 2) % 3);
            for s in [-1.0f32, 1.0] {
                let corners = [(-0.5, -0.5), (0.5, -0.5), (0.5, 0.5), (-0.5, 0.5)];
                let order = if s > 0.0 { [0, 1, 2, 3] } else { [0, 3, 2, 1] };
                for k in order {
                    let mut p = [0.0f32; 6];
                    p[d] = 0.5 * s;
                    p[u] = corners[k].0;
                    p[v] = corners[k].1;
                    p[3 + d] = s;
                    cube.push(p);
                }
            }
        }
        let cube = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: bytemuck::cast_slice(&cube), usage: wgpu::BufferUsages::VERTEX });
        let inst = device.create_buffer(&wgpu::BufferDescriptor { label: None, size: (MAX_INST * std::mem::size_of::<Inst>()) as u64, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let info = adapter.get_info();
        let webgpu = info.backend == wgpu::Backend::BrowserWebGpu;
        let adapter_name = format!("{} {}", info.name, info.driver).trim().to_string();
        let t = Self::make_targets(&device, &bgl_main, &bgl_clear, &bgl_two, &bgl_post, &env, &samp_lin, &samp_cmp, &pack, &samp_pack, hdr_fmt, w, h, 0);
        Ok(Gfx {
            surface, device, queue, config, direct, hdr, sh_solid, sh_inst, p_down, p_bright, p_blur, p_final,
            bgl_main, bgl_clear, bgl_two, bgl_post, bind_env, samp_lin, samp_cmp, env, t, idx, cube, inst,
            samp_pack, pack, pack_on: false, tex_res: 16.0, hdr_fmt, hdr8, tier: 0, webgpu, srgb: format.is_srgb(), adapter_name,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn make_targets(
        device: &wgpu::Device, bgl_main: &wgpu::BindGroupLayout, bgl_clear: &wgpu::BindGroupLayout, bgl_two: &wgpu::BindGroupLayout, bgl_post: &wgpu::BindGroupLayout,
        env: &wgpu::Buffer, lin: &wgpu::Sampler, cmp: &wgpu::Sampler, pack: &wgpu::TextureView, psamp: &wgpu::Sampler, hdr: wgpu::TextureFormat, w: u32, h: u32, tier: u8,
    ) -> PostProcessTargets {
        let (w, h) = (w.max(1), h.max(1));
        let shadow_size = match tier { 0 => 1, 1 => 1024, _ => 2048 };
        // Eco braucht nur die Tiefe; alles Übrige 1×1, damit die Bindungen gültig bleiben
        let (fw, fh) = if tier == 0 { (1, 1) } else { (w, h) };
        let shadow = tex(device, shadow_size, shadow_size, DEPTH);
        let depth = tex(device, w, h, DEPTH);
        let scene = tex(device, fw, fh, hdr);
        let clear = tex(device, fw, fh, hdr);
        let half = tex(device, fw.div_ceil(2), fh.div_ceil(2), hdr);
        let q1 = tex(device, fw.div_ceil(4), fh.div_ceil(4), hdr);
        let q2 = tex(device, fw.div_ceil(8), fh.div_ceil(8), hdr);
        use wgpu::BindingResource::{Sampler as S, TextureView as T};
        let bind_main = bind(device, bgl_main, &[env.as_entire_binding(), T(&shadow), S(cmp), T(pack), S(psamp)]);
        let bind_clear = bind(device, bgl_clear, &[T(&scene), T(&depth), S(lin)]);
        let bind_down = bind(device, bgl_two, &[T(&scene), T(&clear), S(lin)]);
        let bind_b1 = bind(device, bgl_two, &[T(&half), T(&half), S(lin)]);
        let bind_b2 = bind(device, bgl_two, &[T(&q1), T(&q1), S(lin)]);
        let bind_post = bind(device, bgl_post, &[T(&scene), T(&clear), T(&depth), T(&half), T(&q1), T(&q2), S(lin)]);
        PostProcessTargets { key: (w, h, tier), shadow_size, shadow, depth, scene, clear, half, q1, q2, bind_main, bind_clear, bind_down, bind_b1, bind_b2, bind_post }
    }

    fn ensure_targets(&mut self) {
        let key = (self.config.width, self.config.height, self.tier);
        if self.t.key != key {
            self.t = Self::make_targets(&self.device, &self.bgl_main, &self.bgl_clear, &self.bgl_two, &self.bgl_post, &self.env, &self.samp_lin, &self.samp_cmp, &self.pack, &self.samp_pack, self.hdr_fmt, key.0, key.1, key.2);
        }
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        if w == 0 || h == 0 || (w, h) == (self.config.width, self.config.height) {
            return;
        }
        self.config.width = w;
        self.config.height = h;
        self.surface.configure(&self.device, &self.config);
        self.ensure_targets();
    }
    /// Shader-Stufe 0 Eco, 1 Hoch, 2 Ultra, 3 Extrem.
    pub fn set_tier(&mut self, tier: u8) {
        self.tier = tier.min(3);
        self.ensure_targets();
    }
    /// Pixelstufen der Blockmuster (16 Klassik, 64 HD, 128 Ultra).
    pub fn set_tex_res(&mut self, res: f32) {
        self.tex_res = if res.is_finite() { res.clamp(8.0, 256.0) } else { 16.0 };
    }
    /// Eigenes Texturpaket: RGBA8, 4×4 Kacheln = Muster 0..15, wird mit der Blockfarbe multipliziert.
    pub fn set_texture_pack(&mut self, rgba: &[u8], w: u32, h: u32) {
        let max = self.device.limits().max_texture_dimension_2d;
        if w < 4 || h < 4 || w > max || h > max || rgba.len() < (w as usize) * (h as usize) * 4 {
            return;
        }
        self.pack = rgba_tex(&self.device, &self.queue, &rgba[..(w as usize) * (h as usize) * 4], w, h);
        self.pack_on = true;
        self.rebind_main();
    }
    pub fn clear_texture_pack(&mut self) {
        self.pack = rgba_tex(&self.device, &self.queue, &[255u8; 4], 1, 1);
        self.pack_on = false;
        self.rebind_main();
    }
    fn rebind_main(&mut self) {
        use wgpu::BindingResource::{Sampler as S, TextureView as T};
        self.t.bind_main = bind(&self.device, &self.bgl_main, &[self.env.as_entire_binding(), T(&self.t.shadow), S(&self.samp_cmp), T(&self.pack), S(&self.samp_pack)]);
    }
    pub fn tier(&self) -> u8 {
        self.tier
    }
    pub fn aspect(&self) -> f32 {
        self.config.width as f32 / self.config.height as f32
    }

    /// Gitter eines Stücks hochladen; der Puffer wächst nur, wenn er zu klein ist.
    pub fn upload(&self, c: &mut ChunkGpu, solid: &[[u32; 2]], clear: &[[u32; 2]]) {
        let sq = (solid.len() / 4).min(MAX_QUADS);
        let cq = (clear.len() / 4).min(MAX_QUADS - sq);
        let bytes = ((sq + cq) * 32) as u64;
        c.solid = sq as u32;
        c.clear = cq as u32;
        if bytes == 0 {
            return;
        }
        if c.buf.is_none() || c.cap < bytes {
            c.cap = bytes.next_power_of_two().max(4096);
            c.buf = Some(self.device.create_buffer(&wgpu::BufferDescriptor { label: None, size: c.cap, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false }));
        }
        let Some(b) = &c.buf else { return };
        if sq > 0 {
            self.queue.write_buffer(b, 0, bytemuck::cast_slice(&solid[..sq * 4]));
        }
        if cq > 0 {
            self.queue.write_buffer(b, (sq * 32) as u64, bytemuck::cast_slice(&clear[..cq * 4]));
        }
    }

    /// Ein Bild. `visible`: Stücke im Sichtkegel (vorn zuerst), `shadow_vis`: Stücke im Schattenbereich.
    pub fn render(&mut self, e: &mut Env, chunks: &[ChunkGpu], visible: &[u32], shadow_vis: &[u32], insts: &[Inst]) -> u32 {
        let frame = match self.surface.get_current_texture() {
            Ok(f) => f,
            Err(_) => {
                self.surface.configure(&self.device, &self.config);
                return 0;
            }
        };
        let view = frame.texture.create_view(&Default::default());
        let tier = self.tier;
        let (w, h) = (self.config.width as f32, self.config.height as f32);
        e.par[0] = self.srgb as u8 as f32;
        e.par[3] = tier as f32;
        e.par2 = [self.hdr8 as u8 as f32, (tier == 0) as u8 as f32, 1.0 / self.t.shadow_size as f32, if self.pack_on { -self.tex_res } else { self.tex_res }];
        e.scr = [w, h, 1.0 / w, 1.0 / h];
        self.queue.write_buffer(&self.env, 0, bytemuck::bytes_of(e));
        let n = insts.len().min(MAX_INST);
        if n > 0 {
            self.queue.write_buffer(&self.inst, 0, bytemuck::cast_slice(&insts[..n]));
        }
        let mut quads = 0;
        let t = &self.t;
        let mut enc = self.device.create_command_encoder(&Default::default());
        if tier == 0 {
            // Eco: alles in einem Durchgang direkt aufs Bild
            let mut rp = begin(&mut enc, Some(&view), Some(&t.depth));
            rp.set_bind_group(0, &t.bind_main, &[]);
            rp.set_pipeline(&self.direct.sky);
            rp.draw(0..3, 0..1);
            rp.set_index_buffer(self.idx.slice(..), wgpu::IndexFormat::Uint32);
            rp.set_pipeline(&self.direct.solid);
            quads += draw_solid(&mut rp, chunks, visible);
            rp.set_pipeline(&self.direct.inst);
            draw_inst(&mut rp, &self.cube, &self.inst, n);
            rp.set_pipeline(&self.direct.clear);
            quads += draw_clear(&mut rp, chunks, visible);
        } else {
            {
                // Schattenkarte aus Sicht von Sonne/Mond
                let mut rp = begin(&mut enc, None, Some(&t.shadow));
                rp.set_bind_group(0, &self.bind_env, &[]);
                rp.set_index_buffer(self.idx.slice(..), wgpu::IndexFormat::Uint32);
                rp.set_pipeline(&self.sh_solid);
                draw_solid(&mut rp, chunks, shadow_vis);
                rp.set_pipeline(&self.sh_inst);
                draw_inst(&mut rp, &self.cube, &self.inst, n);
            }
            {
                // feste Szene in HDR
                let mut rp = begin(&mut enc, Some(&t.scene), Some(&t.depth));
                rp.set_bind_group(0, &t.bind_main, &[]);
                rp.set_pipeline(&self.hdr.sky);
                rp.draw(0..3, 0..1);
                rp.set_index_buffer(self.idx.slice(..), wgpu::IndexFormat::Uint32);
                rp.set_pipeline(&self.hdr.solid);
                quads += draw_solid(&mut rp, chunks, visible);
                rp.set_pipeline(&self.hdr.inst);
                draw_inst(&mut rp, &self.cube, &self.inst, n);
            }
            {
                // durchsichtige Schicht: liest Szene und Tiefe (Wasser: Brechung, Schaum, Spiegelung)
                let mut rp = begin(&mut enc, Some(&t.clear), None);
                rp.set_bind_group(0, &t.bind_main, &[]);
                rp.set_bind_group(1, &t.bind_clear, &[]);
                rp.set_index_buffer(self.idx.slice(..), wgpu::IndexFormat::Uint32);
                rp.set_pipeline(&self.hdr.clear);
                quads += draw_clear(&mut rp, chunks, visible);
            }
            full(&mut enc, &t.half, &self.p_down, &t.bind_main, &t.bind_down);
            full(&mut enc, &t.q1, &self.p_bright, &t.bind_main, &t.bind_b1);
            if tier >= 2 {
                full(&mut enc, &t.q2, &self.p_blur, &t.bind_main, &t.bind_b2);
            }
            full(&mut enc, &view, &self.p_final, &t.bind_main, &t.bind_post);
        }
        self.queue.submit(Some(enc.finish()));
        frame.present();
        quads
    }
}

fn begin<'e>(enc: &'e mut wgpu::CommandEncoder, color: Option<&wgpu::TextureView>, depth: Option<&wgpu::TextureView>) -> wgpu::RenderPass<'e> {
    let cols = [color.map(|view| wgpu::RenderPassColorAttachment { view, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store } })];
    enc.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: None,
        color_attachments: if color.is_some() { &cols[..] } else { &[] },
        depth_stencil_attachment: depth.map(|view| wgpu::RenderPassDepthStencilAttachment { view, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }), stencil_ops: None }),
        timestamp_writes: None,
        occlusion_query_set: None,
    })
}

fn full(enc: &mut wgpu::CommandEncoder, target: &wgpu::TextureView, pipe: &wgpu::RenderPipeline, b0: &wgpu::BindGroup, b1: &wgpu::BindGroup) {
    let mut rp = begin(enc, Some(target), None);
    rp.set_bind_group(0, b0, &[]);
    rp.set_bind_group(1, b1, &[]);
    rp.set_pipeline(pipe);
    rp.draw(0..3, 0..1);
}

fn draw_solid(rp: &mut wgpu::RenderPass<'_>, chunks: &[ChunkGpu], list: &[u32]) -> u32 {
    let mut q = 0;
    for &i in list {
        let c = &chunks[i as usize];
        let Some(b) = &c.buf else { continue };
        if c.solid > 0 {
            rp.set_vertex_buffer(0, b.slice(..c.solid as u64 * 32));
            rp.draw_indexed(0..c.solid * 6, 0, 0..1);
            q += c.solid;
        }
    }
    q
}

/// Durchsichtiges von hinten nach vorn.
fn draw_clear(rp: &mut wgpu::RenderPass<'_>, chunks: &[ChunkGpu], visible: &[u32]) -> u32 {
    let mut q = 0;
    for &i in visible.iter().rev() {
        let c = &chunks[i as usize];
        let Some(b) = &c.buf else { continue };
        if c.clear > 0 {
            rp.set_vertex_buffer(0, b.slice(c.solid as u64 * 32..(c.solid + c.clear) as u64 * 32));
            rp.draw_indexed(0..c.clear * 6, 0, 0..1);
            q += c.clear;
        }
    }
    q
}

fn draw_inst(rp: &mut wgpu::RenderPass<'_>, cube: &wgpu::Buffer, inst: &wgpu::Buffer, n: usize) {
    if n > 0 {
        rp.set_vertex_buffer(0, cube.slice(..));
        rp.set_vertex_buffer(1, inst.slice(..(n * std::mem::size_of::<Inst>()) as u64));
        rp.draw_indexed(0..36, 0, 0..n as u32);
    }
}
