//! # pizzart-render
//!
//! Compositing su GPU con wgpu: un solo codice per DX12, Metal, Vulkan e WebGPU.
//!
//! ## Come funziona
//!
//! Ogni layer viene disegnato sopra il risultato precedente in **ping-pong**:
//! due texture della dimensione del documento si alternano come sorgente e
//! destinazione. Un solo pass per layer, e dentro lo shader la stessa formula
//! di fusione che il core usa su CPU (`blend.rs`).
//!
//! La parità CPU/GPU è un test, non una promessa: `gpu_e_cpu_concordano`
//! compone lo stesso documento nei due modi e confronta i pixel.
//!
//! ## Backend
//!
//! Attenzione: chiedere un backend che non è stato compilato (per esempio
//! Vulkan su una build senza la feature) in wgpu 23 provoca un access
//! violation, non un errore. Per questo `BACKENDS` è scelto a tempo di
//! compilazione per piattaforma.

#![forbid(unsafe_code)]

pub mod cpu;

use std::future::Future;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use std::thread::{self, Thread};

use bytemuck::{Pod, Zeroable};
/// Nome locale del buffer ritagliato: il tipo vive nel core, così il renderer
/// non deve dipendere dal motore di disegno (lo stesso tipo servirà agli effetti).
use pizzart_core::PlacedImage as StrokeBuffer;
use pizzart_core::{BlendMode, Document, Layer, Mask, Node};

/// Backend wgpu selezionato per questa piattaforma.
///
/// Deve contenere solo backend compilati: vedi la nota nel doc del crate.
#[cfg(target_os = "windows")]
pub const BACKENDS: wgpu::Backends = wgpu::Backends::DX12;
#[cfg(target_os = "macos")]
pub const BACKENDS: wgpu::Backends = wgpu::Backends::METAL;
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub const BACKENDS: wgpu::Backends = wgpu::Backends::VULKAN;

const SHADER: &str = include_str!("composite.wgsl");

/// Esegue una future fino al completamento, parcheggiando il thread quando è in attesa.
pub fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = Box::pin(future);
    let parker = Arc::new(Parker(thread::current()));
    let waker = Waker::from(parker);
    let mut cx = Context::from_waker(&waker);
    loop {
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(value) => return value,
            Poll::Pending => thread::park(),
        }
    }
}

struct Parker(Thread);

impl Wake for Parker {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}

/// Parametri del singolo pass, replicati 1:1 nello shader.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct Uniforms {
    /// Da pixel del documento a pixel del layer.
    inv: [[f32; 4]; 4],
    doc_size: [f32; 2],
    layer_size: [f32; 2],
    mode: u32,
    opacity: f32,
    _pad: [f32; 2],
}

/// Immagine composta, pronta per l'export.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderedImage {
    pub width: u32,
    pub height: u32,
    /// RGBA8, alpha non premoltiplicato.
    pub rgba: Vec<u8>,
}

impl RenderedImage {
    pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let i = ((y as usize * self.width as usize) + x as usize) * 4;
        Some([
            self.rgba[i],
            self.rgba[i + 1],
            self.rgba[i + 2],
            self.rgba[i + 3],
        ])
    }
}

pub struct HeadlessRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    /// Layout e pipeline degli effetti: uno per entry point di `effects.wgsl`.
    effects_bind_group_layout: wgpu::BindGroupLayout,
    effect_pipelines: Vec<wgpu::RenderPipeline>,
    /// Pass di combinazione (sharpen, glow): leggono anche l'originale.
    combine_bind_group_layout: wgpu::BindGroupLayout,
    combine_pipelines: Vec<wgpu::RenderPipeline>,
    sampler: wgpu::Sampler,
    format: wgpu::TextureFormat,
    /// Maschera 1×1 opaca, legata ai layer senza maschera: un solo percorso nello shader.
    default_mask: wgpu::Texture,
    adapter_name: String,
}

impl HeadlessRenderer {
    /// Prepara un renderer headless. `None` se questa macchina non ha una GPU raggiungibile.
    pub fn new() -> Option<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: BACKENDS,
            ..Default::default()
        });
        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        }))?;
        let adapter_name = adapter.get_info().name;
        let (device, queue) =
            block_on(adapter.request_device(&wgpu::DeviceDescriptor::default(), None)).ok()?;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("composite"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("composite bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // Maschera del layer (canale alpha, R8). Si lega sempre: una
                // texture 1×1 bianca quando il layer non ha maschera.
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("composite layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let format = wgpu::TextureFormat::Rgba8Unorm;
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("composite pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                // La fusione la calcola lo shader: qui si scrive il risultato.
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Nearest per scelta: il riferimento CPU campiona il pixel più vicino,
        // e la parità dei test vale più di un filtro morbido. Il bilineare
        // arriverà come opzione di qualità, con test separati.
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("composite sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        // Effetti: un layout condiviso, una pipeline per entry point.
        let effects_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("effects bgl"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let effects_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("effects layout"),
            bind_group_layouts: &[&effects_bind_group_layout],
            push_constant_ranges: &[],
        });

        let effects_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("effects"),
            source: wgpu::ShaderSource::Wgsl(include_str!("effects.wgsl").into()),
        });

        // Maschera neutra (1x1 bianco): legata quando il layer non ha maschera.
        let default_mask = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("default mask"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &default_mask,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[255u8],
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(1),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );

        let mut effect_pipelines = Vec::new();
        for entry_point in EFFECT_ENTRY_POINTS {
            effect_pipelines.push(
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some(entry_point),
                    layout: Some(&effects_layout),
                    vertex: wgpu::VertexState {
                        module: &effects_shader,
                        entry_point: Some("vs_main"),
                        buffers: &[],
                        compilation_options: Default::default(),
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &effects_shader,
                        entry_point: Some(entry_point),
                        // La fusione la calcola lo shader.
                        targets: &[Some(wgpu::ColorTargetState {
                            format,
                            blend: None,
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                        compilation_options: Default::default(),
                    }),
                    primitive: wgpu::PrimitiveState::default(),
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState::default(),
                    multiview: None,
                    cache: None,
                }),
            );
        }

        // Pass di combinazione: come sopra, più l'originale (binding 3).
        let combine_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("effects combine bgl"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            });

        let combine_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("effects combine layout"),
            bind_group_layouts: &[&combine_bind_group_layout],
            push_constant_ranges: &[],
        });

        let mut combine_pipelines = Vec::new();
        for entry_point in COMBINE_ENTRY_POINTS {
            combine_pipelines.push(device.create_render_pipeline(
                &wgpu::RenderPipelineDescriptor {
                    label: Some(entry_point),
                    layout: Some(&combine_layout),
                    vertex: wgpu::VertexState {
                        module: &effects_shader,
                        entry_point: Some("vs_main"),
                        buffers: &[],
                        compilation_options: Default::default(),
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &effects_shader,
                        entry_point: Some(entry_point),
                        targets: &[Some(wgpu::ColorTargetState {
                            format,
                            blend: None,
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                        compilation_options: Default::default(),
                    }),
                    primitive: wgpu::PrimitiveState::default(),
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState::default(),
                    multiview: None,
                    cache: None,
                },
            ));
        }

        Some(Self {
            device,
            queue,
            pipeline,
            bind_group_layout,
            effects_bind_group_layout,
            effect_pipelines,
            combine_bind_group_layout,
            combine_pipelines,
            sampler,
            format,
            default_mask,
            adapter_name,
        })
    }

    /// Nome della GPU in uso (utile nei log e nei report di benchmark).
    pub fn adapter_name(&self) -> &str {
        &self.adapter_name
    }

    /// Compone il documento in un'immagine RGBA8.
    pub fn render(&self, doc: &Document) -> Result<RenderedImage, RenderError> {
        self.compose_tree(doc, &[])
    }
    /// Alias storico di [`Self::render_document_with_tiles`].
    pub fn render_document_with_strokes(
        &self,
        doc: &Document,
        tiles: &[StrokeLayer],
    ) -> Result<RenderedImage, RenderError> {
        self.render_document_with_tiles(doc, tiles)
    }

    /// Compone il documento e, sopra di esso, le tessere già caricate su GPU.
    ///
    /// I tratti si disegnano nell'ordine in cui sono passati: l'ultimo finisce
    /// sopra gli altri, come ci si aspetta da un'applicazione reale.
    pub fn render_document_with_tiles(
        &self,
        doc: &Document,
        strokes: &[StrokeLayer],
    ) -> Result<RenderedImage, RenderError> {
        self.compose_tree(doc, strokes)
    }
    /// Carica in GPU il buffer ritagliato di uno stroke.
    ///
    /// La texture ha le dimensioni del **ritaglio**, non della tela: uno stroke
    /// di 100×100 su una tela 4000×4000 occupa 40 KB, non 64 MB. La posizione
    /// (`offset_x`, `offset_y`) viaggia con il buffer e diventa la matrice di
    /// traslazione nella composizione.
    ///
    /// Il buffer prodotto da `Stroke::render_bbox` contiene **già** l'opacità del
    /// pennello: per una parità perfetta con la CPU si passa `opacity = 1.0`.
    /// Un valore minore è un moltiplicatore in più (fade, dissolvenza).
    pub fn upload_tile(
        &self,
        stroke: &StrokeBuffer,
        blend: BlendMode,
        opacity: f32,
    ) -> Result<StrokeLayer, StrokeError> {
        if stroke.is_empty() {
            return Err(StrokeError::EmptyTile);
        }
        let attesi = stroke.width as usize * stroke.height as usize * 4;
        if stroke.rgba.len() != attesi {
            return Err(StrokeError::BufferSizeMismatch {
                expected: attesi,
                actual: stroke.rgba.len(),
            });
        }
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("stroke"),
            size: wgpu::Extent3d {
                width: stroke.width,
                height: stroke.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &stroke.rgba,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(stroke.width * 4),
                rows_per_image: Some(stroke.height),
            },
            wgpu::Extent3d {
                width: stroke.width,
                height: stroke.height,
                depth_or_array_layers: 1,
            },
        );
        Ok(StrokeLayer {
            texture,
            width: stroke.width,
            height: stroke.height,
            offset_x: stroke.offset_x,
            offset_y: stroke.offset_y,
            blend,
            opacity: opacity.clamp(0.0, 1.0),
        })
    }

    /// Alias storico di [`Self::upload_tile`]: uno stroke è solo una tessera
    /// come un'altra (testo, forma, sticker).
    pub fn upload_stroke(
        &self,
        tile: &StrokeBuffer,
        blend: BlendMode,
        opacity: f32,
    ) -> Result<StrokeLayer, StrokeError> {
        self.upload_tile(tile, blend, opacity)
    }

    /// Applica una catena di effetti a una tessera, restituendone una nuova.
    ///
    /// Ogni effetto è una sequenza di **pass separati**; lo shader di un effetto
    /// non conosce gli altri e non sa nulla del blend. `is_last` dice a ogni pass
    /// se applicare blend e opacità: solo l'ultimo lo fa, altrimenti un blur con
    /// `Multiply` moltiplicherebbe due volte.
    ///
    /// Servono tre texture: due per il ping-pong e una come **originale** per i
    /// pass di combinazione (sharpen, glow), che confrontano il risultato del
    /// kernel con l'ingresso dell'effetto.
    pub fn apply_effects(
        &self,
        input: &StrokeBuffer,
        effects: &[Effect],
    ) -> Result<StrokeBuffer, RenderError> {
        if input.is_empty() || effects.is_empty() {
            return Ok(input.clone());
        }
        let (w, h) = (input.width, input.height);

        let make = |label: &str| {
            self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            })
        };

        let textures = [make("fx a"), make("fx b"), make("fx c")];
        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &textures[0],
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &input.rgba,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(w * 4),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );

        let mut current = 0usize;
        let mut encoder = self.device.create_command_encoder(&Default::default());

        for effect in effects {
            let passes = effect.shader.passes();
            let last = passes.len() - 1;
            // L'originale dell'effetto non viene mai sovrascritto: i pass
            // lavorano sulle altre due texture.
            let original = current;
            let spares: [usize; 2] = match current {
                0 => [1, 2],
                1 => [0, 2],
                _ => [0, 1],
            };

            for (pass_index, op) in passes.iter().enumerate() {
                let source = current;
                let destination = spares[pass_index % 2];

                let uniforms = FxUniforms {
                    params: effect.params,
                    blend: blend_mode_code(effect.blend),
                    opacity: effect.opacity.clamp(0.0, 1.0),
                    is_last: if pass_index == last { 1.0 } else { 0.0 },
                    radius: effect.radius,
                };
                let uniform_buffer = wgpu::util::DeviceExt::create_buffer_init(
                    &self.device,
                    &wgpu::util::BufferInitDescriptor {
                        label: Some("fx params"),
                        contents: bytemuck::bytes_of(&uniforms),
                        usage: wgpu::BufferUsages::UNIFORM,
                    },
                );

                let input_view = textures[source].create_view(&Default::default());
                let output_view = textures[destination].create_view(&Default::default());
                let original_view = textures[original].create_view(&Default::default());

                let bind_group = match op {
                    PassOp::Single(pipeline) => {
                        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                            label: Some("fx bg"),
                            layout: &self.effects_bind_group_layout,
                            entries: &[
                                wgpu::BindGroupEntry {
                                    binding: 0,
                                    resource: uniform_buffer.as_entire_binding(),
                                },
                                wgpu::BindGroupEntry {
                                    binding: 1,
                                    resource: wgpu::BindingResource::TextureView(&input_view),
                                },
                                wgpu::BindGroupEntry {
                                    binding: 2,
                                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                                },
                            ],
                        });
                        (group, *pipeline, false)
                    }
                    PassOp::SharpenCombine | PassOp::GlowCombine => {
                        let pipeline = match op {
                            PassOp::SharpenCombine => COMBINE_SHARPEN,
                            PassOp::GlowCombine => COMBINE_GLOW,
                            PassOp::Single(_) => unreachable!(),
                        };
                        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                            label: Some("fx combine bg"),
                            layout: &self.combine_bind_group_layout,
                            entries: &[
                                wgpu::BindGroupEntry {
                                    binding: 0,
                                    resource: uniform_buffer.as_entire_binding(),
                                },
                                wgpu::BindGroupEntry {
                                    binding: 1,
                                    resource: wgpu::BindingResource::TextureView(&input_view),
                                },
                                wgpu::BindGroupEntry {
                                    binding: 2,
                                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                                },
                                wgpu::BindGroupEntry {
                                    binding: 3,
                                    resource: wgpu::BindingResource::TextureView(&original_view),
                                },
                            ],
                        });
                        (group, pipeline, true)
                    }
                };

                let (bind_group, pipeline_index, is_combine) = bind_group;
                {
                    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("fx pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &output_view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });
                    let pipeline = if is_combine {
                        &self.combine_pipelines[pipeline_index]
                    } else {
                        &self.effect_pipelines[pipeline_index]
                    };
                    pass.set_pipeline(pipeline);
                    pass.set_bind_group(0, &bind_group, &[]);
                    pass.draw(0..6, 0..1);
                }

                current = destination;
            }
        }

        self.queue.submit(Some(encoder.finish()));
        let rgba = self.read_rgba(&textures[current], w, h)?;
        Ok(StrokeBuffer {
            rgba,
            width: w,
            height: h,
            offset_x: input.offset_x,
            offset_y: input.offset_y,
            canvas_w: input.canvas_w,
            canvas_h: input.canvas_h,
        })
    }

    /// Crea una texture bersaglio delle dimensioni del documento.
    fn make_target(&self, w: u32, h: u32, label: &str) -> wgpu::Texture {
        self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        })
    }

    fn clear_target(&self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::Texture) {
        let view = target.create_view(&Default::default());
        let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("clear"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
    }

    /// Disegna una sorgente sopra il buffer corrente e restituisce il nuovo buffer.
    ///
    /// Una sorgente può essere un layer caricato **o il composito di un gruppo**:
    /// per lo shader non c'è differenza, cambia solo da dove arriva la texture.
    #[allow(clippy::too_many_arguments)]
    fn draw_source(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        prev: &wgpu::Texture,
        source: &wgpu::Texture,
        mask: &wgpu::Texture,
        inv: [[f32; 4]; 4],
        size: (u32, u32),
        blend: BlendMode,
        opacity: f32,
        doc_size: (u32, u32),
    ) -> wgpu::Texture {
        let uniforms = Uniforms {
            inv,
            doc_size: [doc_size.0 as f32, doc_size.1 as f32],
            layer_size: [size.0 as f32, size.1 as f32],
            mode: blend_mode_code(blend),
            opacity: opacity.clamp(0.0, 1.0),
            _pad: [0.0; 2],
        };
        let uniform_buffer = wgpu::util::DeviceExt::create_buffer_init(
            &self.device,
            &wgpu::util::BufferInitDescriptor {
                label: Some("uniforms"),
                contents: bytemuck::bytes_of(&uniforms),
                usage: wgpu::BufferUsages::UNIFORM,
            },
        );

        let prev_view = prev.create_view(&Default::default());
        let source_view = source.create_view(&Default::default());
        let mask_view = mask.create_view(&Default::default());
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("composite bg"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&prev_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&source_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&mask_view),
                },
            ],
        });

        let dst = self.make_target(doc_size.0, doc_size.1, "composite");
        let dst_view = dst.create_view(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("composite pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &dst_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.draw(0..6, 0..1);
        }
        dst
    }

    /// Compone l'albero in una texture: un livello di annidamento per gruppo.
    ///
    /// Il gruppo viene composto **prima** in una texture propria e poi disegnato
    /// come una sorgente qualsiasi, con il suo blend/opacità/maschera. Costo:
    /// una texture piena per gruppo. Su 4000×4000 sono 64 MB per gruppo.
    fn draw_level(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        nodes: &[Node],
        w: u32,
        h: u32,
    ) -> wgpu::Texture {
        let mut current = self.make_target(w, h, "level");
        self.clear_target(encoder, &current);

        for node in nodes {
            match node {
                Node::Layer(layer) => {
                    if !layer.is_drawable() {
                        continue;
                    }
                    let texture = self.upload_layer(layer);
                    let mask_texture = self.upload_mask_opt(layer.mask.as_deref());
                    let mask = mask_texture.as_ref().unwrap_or(&self.default_mask);
                    current = self.draw_source(
                        encoder,
                        &current,
                        &texture,
                        mask,
                        invert_matrix(layer),
                        (layer.width, layer.height),
                        layer.blend,
                        layer.opacity,
                        (w, h),
                    );
                }
                Node::Group(group) => {
                    if !group.visible || group.opacity <= 0.0 {
                        continue;
                    }
                    let interno = self.draw_level(encoder, &group.children, w, h);
                    let mask_texture = self.upload_mask_opt(group.mask.as_deref());
                    let mask = mask_texture.as_ref().unwrap_or(&self.default_mask);
                    current = self.draw_source(
                        encoder,
                        &current,
                        &interno,
                        mask,
                        IDENTITY,
                        (w, h),
                        group.blend,
                        group.opacity,
                        (w, h),
                    );
                }
            }
        }
        current
    }

    /// Compone l'albero e, sopra, le tessere già caricate.
    fn compose_tree(
        &self,
        doc: &Document,
        tiles: &[StrokeLayer],
    ) -> Result<RenderedImage, RenderError> {
        let (w, h) = (doc.width, doc.height);
        if w == 0 || h == 0 {
            return Err(RenderError::EmptyDocument);
        }
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let mut current = self.draw_level(&mut encoder, &doc.root, w, h);

        for tile in tiles {
            current = self.draw_source(
                &mut encoder,
                &current,
                &tile.texture,
                &self.default_mask,
                translation_columns(-(tile.offset_x as f32), -(tile.offset_y as f32)),
                (tile.width, tile.height),
                tile.blend,
                tile.opacity,
                (w, h),
            );
        }

        self.queue.submit(Some(encoder.finish()));
        let rgba = self.read_rgba(&current, w, h)?;
        Ok(RenderedImage {
            width: w,
            height: h,
            rgba,
        })
    }

    /// Legge una texture RGBA8 in memoria, togliendo il padding di riga della GPU.
    fn read_rgba(&self, texture: &wgpu::Texture, w: u32, h: u32) -> Result<Vec<u8>, RenderError> {
        let unpadded = w * 4;
        let padded = unpadded.div_ceil(256) * 256;
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: (padded as u64) * (h as u64),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: &readback,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(h),
                },
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit(Some(encoder.finish()));

        let slice = readback.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.device.poll(wgpu::Maintain::Wait);
        match rx.recv() {
            Ok(Ok(())) => {
                let data = slice.get_mapped_range();
                let mut rgba = Vec::with_capacity((unpadded * h) as usize);
                for row in 0..h {
                    let start = (row * padded) as usize;
                    rgba.extend_from_slice(&data[start..start + unpadded as usize]);
                }
                drop(data);
                readback.unmap();
                Ok(rgba)
            }
            _ => Err(RenderError::Readback),
        }
    }

    /// Carica i pixel del layer in una texture dedicata.
    fn upload_layer(&self, layer: &Layer) -> wgpu::Texture {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("layer"),
            size: wgpu::Extent3d {
                width: layer.width.max(1),
                height: layer.height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &layer.pixels,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(layer.width * 4),
                rows_per_image: Some(layer.height),
            },
            wgpu::Extent3d {
                width: layer.width.max(1),
                height: layer.height.max(1),
                depth_or_array_layers: 1,
            },
        );
        texture
    }

    /// Carica una maschera, se c'è.
    fn upload_mask_opt(&self, mask: Option<&Mask>) -> Option<wgpu::Texture> {
        mask.map(|m| self.upload_mask(m))
    }

    /// Carica il canale alpha di una maschera in una texture R8.
    fn upload_mask(&self, mask: &pizzart_core::Mask) -> wgpu::Texture {
        let (w, h) = (mask.width.max(1), mask.height.max(1));
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("layer mask"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        // L'inversione si applica qui, una volta, invece che nel shader:
        // la GPU riceve sempre "quanto si vede", non il flag.
        let alpha: Vec<u8> = if mask.inverted {
            mask.alpha.iter().map(|a| 255 - a).collect()
        } else {
            mask.alpha.clone()
        };
        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &alpha,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(w),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        texture
    }
}

/// Matrice inversa della trasformazione del layer, in layout colonna (WGSL).
fn invert_matrix(layer: &Layer) -> [[f32; 4]; 4] {
    let m = layer
        .transform
        .doc_to_layer()
        .unwrap_or(pizzart_core::Mat4::IDENTITY);
    let c = |i: usize| [m.m[i * 4], m.m[i * 4 + 1], m.m[i * 4 + 2], m.m[i * 4 + 3]];
    [c(0), c(1), c(2), c(3)]
}

/// Un tratto caricato in GPU, pronto per essere composto sopra il documento.
pub struct StrokeLayer {
    pub texture: wgpu::Texture,
    pub width: u32,
    pub height: u32,
    /// Posizione del ritaglio nella tela: la texture occupa solo questa regione.
    pub offset_x: i32,
    pub offset_y: i32,
    pub blend: BlendMode,
    /// Moltiplicatore aggiuntivo: il buffer di `Stroke::render_bbox` porta già
    /// l'opacità del pennello, quindi normalmente vale 1.0.
    pub opacity: f32,
}

impl std::fmt::Debug for StrokeLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StrokeLayer")
            .field("size", &(self.width, self.height))
            .field("offset", &(self.offset_x, self.offset_y))
            .field("blend", &self.blend)
            .field("opacity", &self.opacity)
            .finish()
    }
}

/// Errori di caricamento di un tratto.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StrokeError {
    /// Il tratto non ha pixel (fuori tela, o opacità zero).
    EmptyTile,
    /// Il buffer non corrisponde alle dimensioni dichiarate.
    BufferSizeMismatch { expected: usize, actual: usize },
}

impl std::fmt::Display for StrokeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StrokeError::EmptyTile => write!(f, "tratto senza pixel: niente da caricare"),
            StrokeError::BufferSizeMismatch { expected, actual } => write!(
                f,
                "buffer di {actual} byte, attesi {expected}: dimensioni incoerenti"
            ),
        }
    }
}

impl std::error::Error for StrokeError {}

/// Matrice di traslazione in layout colonna (WGSL).
///
/// Porta un punto del documento nello spazio del ritaglio:
/// `p_ritaglio = p_documento - offset`.
fn translation_columns(tx: f32, ty: f32) -> [[f32; 4]; 4] {
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [tx, ty, 0.0, 1.0],
    ]
}

/// Codice numerico del blend mode, identico all'ordine di `BlendMode::ALL`
/// (e allo shader: `mode` è lo stesso intero).
fn blend_mode_code(mode: BlendMode) -> u32 {
    match mode {
        BlendMode::Normal => 0,
        BlendMode::Multiply => 1,
        BlendMode::Screen => 2,
        BlendMode::Overlay => 3,
        BlendMode::Darken => 4,
        BlendMode::Lighten => 5,
        BlendMode::ColorDodge => 6,
        BlendMode::ColorBurn => 7,
        BlendMode::HardLight => 8,
        BlendMode::SoftLight => 9,
        BlendMode::Difference => 10,
        BlendMode::Exclusion => 11,
    }
}

/// Quale trasformazione applica un effetto.
///
/// Gli effetti *point-wise* leggono un pixel e ne producono uno. Blur, sharpen
/// e glow sono **kernel**: leggono pixel vicini e costano più pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectShader {
    /// `params[0]` = delta da sommare a ogni canale (-1..1).
    Brightness,
    /// `params[0]` = fattore di contrasto attorno al grigio medio (-1..1).
    Contrast,
    /// `params[0]` = fattore di saturazione attorno alla luminanza (-1..1).
    Saturate,
    /// `radius` = raggio, `params[1]` = sigma (0 = automatico).
    Blur,
    /// `params[0]` = intensità (0..2), `radius` = raggio del kernel.
    Sharpen,
    /// `params[0]` = soglia, `params[1]` = intensità, `radius` = raggio del kernel.
    Glow,
}

/// Un pass concreto della catena di un effetto.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PassOp {
    /// Pass a un solo ingresso; l'indice sceglie la pipeline.
    Single(usize),
    /// Ultimo pass di sharpen: legge anche l'originale.
    SharpenCombine,
    /// Ultimo pass di glow: legge anche l'originale.
    GlowCombine,
}

/// Indici delle pipeline a un ingresso, nell'ordine di `EFFECT_ENTRY_POINTS`.
const PIXEL_BRIGHTNESS: usize = 0;
const PIXEL_CONTRAST: usize = 1;
const PIXEL_SATURATE: usize = 2;
const PIXEL_BLUR_H: usize = 3;
const PIXEL_BLUR_V: usize = 4;

/// Entry point a un ingresso, nell'ordine in cui si costruiscono le pipeline.
const EFFECT_ENTRY_POINTS: [&str; 5] = [
    "fs_brightness",
    "fs_contrast",
    "fs_saturate",
    "fs_blur_h",
    "fs_blur_v",
];

/// Entry point che leggono anche l'originale (due texture).
const COMBINE_ENTRY_POINTS: [&str; 2] = ["fs_sharpen_combine", "fs_glow_combine"];
const COMBINE_SHARPEN: usize = 0;
const COMBINE_GLOW: usize = 1;

impl EffectShader {
    /// I pass da eseguire, in ordine.
    fn passes(self) -> &'static [PassOp] {
        const POINT_BRIGHTNESS: [PassOp; 1] = [PassOp::Single(PIXEL_BRIGHTNESS)];
        const POINT_CONTRAST: [PassOp; 1] = [PassOp::Single(PIXEL_CONTRAST)];
        const POINT_SATURATE: [PassOp; 1] = [PassOp::Single(PIXEL_SATURATE)];
        const BLUR: [PassOp; 2] = [PassOp::Single(PIXEL_BLUR_H), PassOp::Single(PIXEL_BLUR_V)];
        const SHARPEN: [PassOp; 3] = [
            PassOp::Single(PIXEL_BLUR_H),
            PassOp::Single(PIXEL_BLUR_V),
            PassOp::SharpenCombine,
        ];
        const GLOW: [PassOp; 3] = [
            PassOp::Single(PIXEL_BLUR_H),
            PassOp::Single(PIXEL_BLUR_V),
            PassOp::GlowCombine,
        ];
        match self {
            EffectShader::Brightness => &POINT_BRIGHTNESS,
            EffectShader::Contrast => &POINT_CONTRAST,
            EffectShader::Saturate => &POINT_SATURATE,
            EffectShader::Blur => &BLUR,
            EffectShader::Sharpen => &SHARPEN,
            EffectShader::Glow => &GLOW,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            EffectShader::Brightness => "Luminosità",
            EffectShader::Contrast => "Contrasto",
            EffectShader::Saturate => "Saturazione",
            EffectShader::Blur => "Sfocatura",
            EffectShader::Sharpen => "Nitidezza",
            EffectShader::Glow => "Bagliore",
        }
    }
}

/// Descrizione di un effetto: nessuno stato, nessuna GPU, solo intenzione.
///
/// Per applicarlo serve un renderer (`apply_effects`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Effect {
    pub name: &'static str,
    pub shader: EffectShader,
    /// Parametri passati allo shader; `params[0]` è quello dell'effetto semplice.
    pub params: [f32; 4],
    /// Raggio del kernel interno: 0 per gli effetti point-wise.
    pub radius: f32,
    pub blend: BlendMode,
    /// 0 = effetto invisibile, 1 = effetto pieno.
    pub opacity: f32,
}

impl Effect {
    fn simple(name: &'static str, shader: EffectShader, value: f32) -> Self {
        Self {
            name,
            shader,
            params: [value, 0.0, 0.0, 0.0],
            radius: 0.0,
            blend: BlendMode::Normal,
            opacity: 1.0,
        }
    }

    /// `delta` in -1..1: sommato a ogni canale, poi clamp.
    pub fn brightness(delta: f32) -> Self {
        Self::simple("brightness", EffectShader::Brightness, delta)
    }

    /// `factor` in -1..1: 0 = nessun cambiamento, 1 = contrasto massimo.
    pub fn contrast(factor: f32) -> Self {
        Self::simple("contrast", EffectShader::Contrast, factor)
    }

    /// `factor` in -1..1: -1 = bianco e nero, 0 = identità, 1 = saturazione doppia.
    pub fn saturate(factor: f32) -> Self {
        Self::simple("saturate", EffectShader::Saturate, factor)
    }

    /// Sfocatura gaussiana separabile.
    ///
    /// `radius` in pixel (0..16). `sigma = 0` lascia scegliere al motore
    /// (`radius / 2`), che è il valore con cui si ottiene un kernel morbido
    /// senza aloni.
    pub fn blur(radius: f32) -> Self {
        Self {
            radius: radius.clamp(0.0, 16.0),
            ..Self::simple("blur", EffectShader::Blur, radius.clamp(0.0, 16.0))
        }
    }

    /// Imposta il sigma del blur (0 = automatico). Ignorato dagli altri effetti.
    pub fn with_sigma(mut self, sigma: f32) -> Self {
        self.params[1] = sigma.max(0.0);
        self
    }

    /// Nitidezza (unsharp mask): `out = orig + amount * (orig - blur(orig))`.
    ///
    /// `amount` in 0..2, `radius` in 0.5..16.
    pub fn sharpen(amount: f32, radius: f32) -> Self {
        Self {
            params: [amount.clamp(0.0, 2.0), 0.0, 0.0, 0.0],
            radius: radius.clamp(0.5, 16.0),
            ..Self::simple("sharpen", EffectShader::Sharpen, 0.0)
        }
    }

    /// Bagliore: aggiunge con Screen la parte di sfocato sopra la soglia.
    pub fn glow(threshold: f32, intensity: f32, radius: f32) -> Self {
        Self {
            params: [threshold.clamp(0.0, 1.0), intensity.max(0.0), 0.0, 0.0],
            radius: radius.clamp(0.5, 16.0),
            ..Self::simple("glow", EffectShader::Glow, 0.0)
        }
    }

    pub fn with_blend(mut self, blend: BlendMode) -> Self {
        self.blend = blend;
        self
    }

    pub fn with_opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity.clamp(0.0, 1.0);
        self
    }
}

/// Parametri del pass di un effetto, replicati in `effects.wgsl`.
/// Matrice identità: il composito di un gruppo è già nello spazio del documento.
const IDENTITY: [[f32; 4]; 4] = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct FxUniforms {
    params: [f32; 4],
    blend: u32,
    opacity: f32,
    /// 1.0 se questo pass deve applicare blend e opacità (ultimo dell'effetto).
    is_last: f32,
    /// Raggio del kernel interno (0 per gli effetti point-wise).
    radius: f32,
}

/// Errori del renderer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenderError {
    /// Documento o tessera senza dimensioni.
    EmptyDocument,
    /// Tratto senza pixel da caricare.
    EmptyTile,
    /// Buffer che non corrisponde alle dimensioni dichiarate.
    BufferSizeMismatch { expected: usize, actual: usize },
    /// La lettura dalla GPU è fallita.
    Readback,
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RenderError::EmptyDocument => write!(f, "documento di dimensione nulla"),
            RenderError::EmptyTile => write!(f, "tessera senza pixel"),
            RenderError::BufferSizeMismatch { expected, actual } => write!(
                f,
                "buffer di {actual} byte, attesi {expected}: dimensioni incoerenti"
            ),
            RenderError::Readback => write!(f, "lettura dalla GPU fallita"),
        }
    }
}

impl std::error::Error for RenderError {}

impl From<StrokeError> for RenderError {
    fn from(e: StrokeError) -> Self {
        match e {
            StrokeError::EmptyTile => RenderError::EmptyTile,
            StrokeError::BufferSizeMismatch { expected, actual } => {
                RenderError::BufferSizeMismatch { expected, actual }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pizzart_core::{BlendMode, Layer, Transform};

    #[test]
    fn block_on_completa_una_future_immediata() {
        assert_eq!(block_on(async { 40 + 2 }), 42);
    }

    #[test]
    fn block_on_attende_una_future_svegliata_da_un_altro_thread() {
        struct Later(u32);

        impl Future for Later {
            type Output = u32;

            fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<u32> {
                if self.0 == 0 {
                    return Poll::Ready(7);
                }
                self.0 -= 1;
                let waker = cx.waker().clone();
                thread::spawn(move || {
                    thread::sleep(std::time::Duration::from_millis(5));
                    waker.wake();
                });
                Poll::Pending
            }
        }

        assert_eq!(block_on(Later(2)), 7);
    }

    #[test]
    fn i_codici_dei_blend_mode_seguono_l_ordine_del_core() {
        for (atteso, mode) in BlendMode::ALL.iter().enumerate() {
            assert_eq!(blend_mode_code(*mode), atteso as u32, "{mode:?}");
        }
        assert_eq!(blend_mode_code(BlendMode::Multiply), 1);
        assert_eq!(blend_mode_code(BlendMode::Exclusion), 11);
    }

    #[test]
    fn la_matrice_inversa_e_in_layout_colonna() {
        let mut l = Layer::transparent("x", 4, 4);
        l.transform = Transform::at(10.0, 20.0);
        let inv = invert_matrix(&l);
        // traslazione inversa: -10, -20 finisce nella quarta colonna (indice 3)
        assert!((inv[3][0] + 10.0).abs() < 1e-5, "colonna 3 = traslazione");
        assert!((inv[3][1] + 20.0).abs() < 1e-5);
    }

    /// Il test che conta: la GPU deve produrre gli stessi pixel della CPU.
    #[test]
    fn gpu_e_cpu_concordano() {
        let Some(renderer) = HeadlessRenderer::new() else {
            eprintln!("nessuna GPU disponibile: test di parità saltato");
            return;
        };

        let mut doc = Document::new(24, 16);
        doc.add_layer(Layer::filled("fondo", 24, 16, [40, 80, 160, 255]));

        let mut patch = Layer::filled("patch", 8, 8, [230, 120, 30, 255]);
        patch.transform = Transform::at(5.0, 3.0);
        patch.blend = BlendMode::Multiply;
        patch.opacity = 0.8;
        doc.add_layer(patch);

        let mut scaled = Layer::filled("scalato", 4, 4, [0, 255, 90, 200]);
        scaled.transform.scale_x = 2.0;
        scaled.transform.scale_y = 2.0;
        scaled.transform.x = 14.0;
        scaled.transform.y = 6.0;
        scaled.blend = BlendMode::Screen;
        doc.add_layer(scaled);

        let cpu = doc.composite();
        let gpu = renderer.render(&doc).expect("render GPU");

        assert_eq!(gpu.rgba.len(), cpu.len());
        let mut max_diff = 0i32;
        let mut worst = 0usize;
        for (i, (a, b)) in cpu.iter().zip(gpu.rgba.iter()).enumerate() {
            let d = (*a as i32 - *b as i32).abs();
            if d > max_diff {
                max_diff = d;
                worst = i;
            }
        }
        assert!(
            max_diff <= 2,
            "differenza massima {max_diff} (byte {worst}): CPU={} GPU={} su adapter {}",
            cpu[worst],
            gpu.rgba[worst],
            renderer.adapter_name()
        );
    }

    #[test]
    fn un_documento_vuoto_produce_un_immagine_trasparente() {
        let Some(renderer) = HeadlessRenderer::new() else {
            eprintln!("nessuna GPU disponibile: test saltato");
            return;
        };
        let doc = Document::new(4, 4);
        let img = renderer.render(&doc).expect("render");
        assert_eq!(img.rgba, vec![0u8; 4 * 4 * 4]);
    }

    #[test]
    fn le_dimensioni_nulle_sono_un_errore() {
        let Some(renderer) = HeadlessRenderer::new() else {
            eprintln!("nessuna GPU disponibile: test saltato");
            return;
        };
        assert!(renderer.render(&Document::new(0, 0)).is_err());
    }

    // ---------------------------------------------------------------------
    // Integrazione draw -> render: dal tablet alla GPU
    // ---------------------------------------------------------------------

    use pizzart_draw::{Brush, Point, Stroke};

    fn tratto_di_prova(blend: BlendMode, opacity: f32) -> Stroke {
        let brush = Brush::new(18.0)
            .hardness(0.5)
            .opacity(opacity)
            .color([40, 90, 200])
            .blend(blend);
        let mut stroke = Stroke::new(brush);
        for i in 0..10 {
            let t = i as f32 / 9.0;
            stroke.add(Point::new(
                12.0 + t * 40.0,
                20.0 + (t * 3.0 * std::f32::consts::PI).sin() * 8.0,
            ));
        }
        stroke
    }

    fn documento_bianco(w: u32, h: u32) -> Document {
        let mut doc = Document::new(w, h);
        doc.add_layer(Layer::filled("fondo", w, h, [255, 255, 255, 255]));
        doc
    }

    fn max_diff(a: &[u8], b: &[u8]) -> (i32, usize) {
        let mut max = 0i32;
        let mut at = 0usize;
        for (i, (x, y)) in a.iter().zip(b.iter()).enumerate() {
            let d = (*x as i32 - *y as i32).abs();
            if d > max {
                max = d;
                at = i;
            }
        }
        (max, at)
    }

    /// Il contratto dell'integrazione: uno stroke disegnato su CPU e lo stesso
    /// stroke disegnato su GPU devono dare gli stessi pixel.
    #[test]
    fn parita_stroke_cpu_gpu() {
        let Some(renderer) = HeadlessRenderer::new() else {
            eprintln!("nessuna GPU disponibile: test di parità saltato");
            return;
        };
        let (w, h) = (64u32, 48u32);
        let doc = documento_bianco(w, h);

        // CPU: buffer del tratto composto sul fondo bianco.
        let stroke = tratto_di_prova(BlendMode::Multiply, 0.7);
        let mut cpu = doc.composite();
        stroke.render_over(&mut cpu, w, h);

        // GPU: stesso buffer caricato come texture e composto sopra il documento.
        // L'opacità del pennello è già dentro il buffer: qui il moltiplicatore è 1.0.
        let layer = renderer
            .upload_stroke(&stroke.render_bbox(w, h), BlendMode::Multiply, 1.0)
            .expect("upload");
        let gpu = renderer
            .render_document_with_strokes(&doc, std::slice::from_ref(&layer))
            .expect("render con stroke");

        let (diff, at) = max_diff(&cpu, &gpu.rgba);
        assert!(
            diff <= 3,
            "differenza {diff}/255 al byte {at}: CPU={} GPU={} (adapter {})",
            cpu[at],
            gpu.rgba[at],
            renderer.adapter_name()
        );
    }

    /// Tutti e 12 i modi, ciascuno su una fascia del canvas: se uno solo sbaglia
    /// la formula, il test lo trova.
    #[test]
    fn parita_di_tutti_i_blend_modi_con_gli_stroke() {
        let Some(renderer) = HeadlessRenderer::new() else {
            eprintln!("nessuna GPU disponibile: test saltato");
            return;
        };
        let (w, h) = (96u32, 12u32 * 12u32);
        let mut doc = Document::new(w, h);
        doc.add_layer(Layer::filled("grigio", w, h, [140, 140, 140, 255]));

        let mut cpu = doc.composite();
        let mut strokes = Vec::new();
        for (i, mode) in BlendMode::ALL.iter().enumerate() {
            let y = i as f32 * 12.0 + 6.0;
            let mut stroke = Stroke::new(
                Brush::new(8.0)
                    .hardness(0.9)
                    .color([220, 60, 60])
                    .blend(*mode),
            );
            for k in 0..8 {
                let x = 8.0 + k as f32 * 11.0;
                stroke.add(Point::new(x, y));
            }
            stroke.render_over(&mut cpu, w, h);
            strokes.push(
                renderer
                    .upload_stroke(&stroke.render_bbox(w, h), *mode, 1.0)
                    .expect("upload"),
            );
        }

        let gpu = renderer
            .render_document_with_strokes(&doc, &strokes)
            .expect("render");
        let (diff, at) = max_diff(&cpu, &gpu.rgba);
        assert!(
            diff <= 3,
            "differenza {diff}/255 al byte {at} (riga {}): CPU={} GPU={}",
            at / (w as usize * 4) / 12,
            cpu[at],
            gpu.rgba[at]
        );
    }

    /// L'opacità passata a `upload_stroke` è un moltiplicatore in più:
    /// 0.5 su un tratto al 70% deve equivalere a un pennello al 35%.
    #[test]
    fn l_opacita_del_layer_stroke_e_un_moltiplicatore() {
        let Some(renderer) = HeadlessRenderer::new() else {
            eprintln!("nessuna GPU disponibile: test saltato");
            return;
        };
        let (w, h) = (64u32, 48u32);
        let doc = documento_bianco(w, h);

        let stroke = tratto_di_prova(BlendMode::Normal, 0.7);
        let mut cpu_meta = doc.composite();
        let meta = tratto_di_prova(BlendMode::Normal, 0.35);
        meta.render_over(&mut cpu_meta, w, h);

        let layer = renderer
            .upload_stroke(&stroke.render_bbox(w, h), BlendMode::Normal, 0.5)
            .expect("upload");
        let gpu = renderer
            .render_document_with_strokes(&doc, std::slice::from_ref(&layer))
            .expect("render");

        let (diff, at) = max_diff(&cpu_meta, &gpu.rgba);
        assert!(diff <= 3, "differenza {diff}/255 al byte {at}");
    }

    #[test]
    fn upload_stroke_rifiuta_buffer_incoerenti() {
        use pizzart_core::PlacedImage;
        let Some(renderer) = HeadlessRenderer::new() else {
            eprintln!("nessuna GPU disponibile: test saltato");
            return;
        };

        // buffer coerente: 4x4 RGBA8
        let ok = PlacedImage {
            rgba: vec![255; 4 * 4 * 4],
            width: 4,
            height: 4,
            offset_x: 0,
            offset_y: 0,
            canvas_w: 8,
            canvas_h: 8,
        };
        assert!(renderer.upload_stroke(&ok, BlendMode::Normal, 1.0).is_ok());

        // dimensione dichiarata e buffer non coincidono (buffer non vuoto)
        let incoerente = PlacedImage {
            rgba: vec![0u8; 60],
            width: 4,
            height: 4,
            offset_x: 0,
            offset_y: 0,
            canvas_w: 8,
            canvas_h: 8,
        };
        match renderer.upload_stroke(&incoerente, BlendMode::Normal, 1.0) {
            Err(StrokeError::BufferSizeMismatch { expected, actual }) => {
                assert_eq!(expected, 64);
                assert_eq!(actual, 60);
            }
            altro => panic!("atteso BufferSizeMismatch, ottenuto {altro:?}"),
        }

        // tratto fuori tela: niente da caricare
        assert_eq!(
            renderer
                .upload_stroke(&PlacedImage::empty(8, 8), BlendMode::Normal, 1.0)
                .unwrap_err(),
            StrokeError::EmptyTile
        );
    }

    /// Il contratto del ritaglio: lo stesso tratto, caricato come tile, deve
    /// dare la stessa immagine del percorso senza ritaglio.
    #[test]
    fn parita_stroke_bbox_cpu_gpu() {
        let Some(renderer) = HeadlessRenderer::new() else {
            eprintln!("nessuna GPU disponibile: test saltato");
            return;
        };
        let (w, h) = (80u32, 56u32);
        let doc = documento_bianco(w, h);
        let stroke = tratto_di_prova(BlendMode::Multiply, 0.7);

        let mut cpu = doc.composite();
        stroke.render_over(&mut cpu, w, h);

        let tile = stroke.render_bbox(w, h);
        assert!(
            !tile.is_empty() && tile.width < w,
            "il tile deve essere più piccolo della tela"
        );
        let layer = renderer
            .upload_stroke(&tile, BlendMode::Multiply, 1.0)
            .expect("upload");
        let gpu = renderer
            .render_document_with_strokes(&doc, std::slice::from_ref(&layer))
            .expect("render");

        let (diff, at) = max_diff(&cpu, &gpu.rgba);
        assert!(
            diff <= 3,
            "differenza {diff}/255 al byte {at}: CPU={} GPU={}",
            cpu[at],
            gpu.rgba[at]
        );
    }

    /// L'offset del ritaglio deve essere rispettato: un tratto in basso a destra
    /// resta in basso a destra.
    #[test]
    fn l_offset_del_bbox_e_rispettato() {
        let Some(renderer) = HeadlessRenderer::new() else {
            eprintln!("nessuna GPU disponibile: test saltato");
            return;
        };
        let (w, h) = (128u32, 96u32);
        let doc = documento_bianco(w, h);

        let mut stroke = Stroke::new(
            Brush::new(10.0)
                .hardness(1.0)
                .color([255, 0, 0])
                .blend(BlendMode::Normal),
        );
        stroke.add(Point::new(100.0, 70.0));

        let tile = stroke.render_bbox(w, h);
        assert!(
            tile.offset_x > 80,
            "offset_x atteso in basso a destra: {}",
            tile.offset_x
        );
        assert!(
            tile.offset_y > 50,
            "offset_y atteso in basso a destra: {}",
            tile.offset_y
        );

        let layer = renderer
            .upload_stroke(&tile, BlendMode::Normal, 1.0)
            .expect("upload");
        let img = renderer
            .render_document_with_strokes(&doc, std::slice::from_ref(&layer))
            .expect("render");

        // il centro del timbro è dove l'abbiamo messo
        let centro = img.pixel(100, 70).expect("pixel");
        assert_eq!(
            centro,
            [255, 0, 0, 255],
            "il tratto deve stare in basso a destra"
        );

        // e in alto a sinistra non c'è nulla
        assert_eq!(
            img.pixel(5, 5).expect("pixel"),
            [255, 255, 255, 255],
            "il resto della tela resta bianco"
        );
    }

    /// Gli stroke si disegnano nell'ordine dato: l'ultimo vince.
    #[test]
    fn gli_stroke_rispettano_l_ordine_di_disegno() {
        let Some(renderer) = HeadlessRenderer::new() else {
            eprintln!("nessuna GPU disponibile: test saltato");
            return;
        };
        let (w, h) = (16u32, 16u32);
        let doc = documento_bianco(w, h);

        let rosso = Stroke::new(
            Brush::new(12.0)
                .hardness(1.0)
                .color([255, 0, 0])
                .blend(BlendMode::Normal),
        );
        let mut rosso = rosso;
        rosso.add(Point::new(8.0, 8.0));

        let blu = Stroke::new(
            Brush::new(12.0)
                .hardness(1.0)
                .color([0, 0, 255])
                .blend(BlendMode::Normal),
        );
        let mut blu = blu;
        blu.add(Point::new(8.0, 8.0));

        let a = renderer
            .upload_stroke(&rosso.render_bbox(w, h), BlendMode::Normal, 1.0)
            .expect("upload");
        let b = renderer
            .upload_stroke(&blu.render_bbox(w, h), BlendMode::Normal, 1.0)
            .expect("upload");

        let img = renderer
            .render_document_with_strokes(&doc, &[a, b])
            .expect("render");
        let centro = img.pixel(8, 8).expect("pixel");
        assert_eq!(centro, [0, 0, 255, 255], "l'ultimo stroke deve stare sopra");
    }
    // ---------------------------------------------------------------------
    // Modulo A: effetti point-wise
    // ---------------------------------------------------------------------

    use crate::cpu;

    /// Tessera di prova con un motivo riconoscibile (gradiente + banda colorata).
    fn tile_di_prova(w: u32, h: u32) -> pizzart_core::PlacedImage {
        let mut rgba = vec![0u8; (w * h * 4) as usize];
        for y in 0..h {
            for x in 0..w {
                let i = ((y * w + x) * 4) as usize;
                let u = x as f32 / (w - 1).max(1) as f32;
                let v = y as f32 / (h - 1).max(1) as f32;
                rgba[i] = (40.0 + 200.0 * u) as u8;
                rgba[i + 1] = (30.0 + 210.0 * v) as u8;
                rgba[i + 2] = (160.0 - 120.0 * u * v) as u8;
                rgba[i + 3] = 255;
            }
        }
        pizzart_core::PlacedImage {
            rgba,
            width: w,
            height: h,
            offset_x: 0,
            offset_y: 0,
            canvas_w: w,
            canvas_h: h,
        }
    }

    fn grigio(valore: u8, lato: u32) -> pizzart_core::PlacedImage {
        let mut t = tile_di_prova(lato, lato);
        for px in t.rgba.chunks_exact_mut(4) {
            px[0] = valore;
            px[1] = valore;
            px[2] = valore;
            px[3] = 255;
        }
        t
    }

    fn canale(t: &pizzart_core::PlacedImage, canale: usize) -> u8 {
        t.rgba[canale]
    }

    #[test]
    fn brightness_aumenta_o_diminuisce_linearmente() {
        let Some(renderer) = HeadlessRenderer::new() else {
            eprintln!("nessuna GPU disponibile: test saltato");
            return;
        };
        let base = grigio(128, 4);

        let su = renderer
            .apply_effects(&base, &[Effect::brightness(0.2)])
            .expect("brightness +");
        let giu = renderer
            .apply_effects(&base, &[Effect::brightness(-0.2)])
            .expect("brightness -");

        // 128/255 = 0.502; +0.2 -> 0.702 -> 179; -0.2 -> 0.302 -> 77
        assert!(
            (canale(&su, 0) as i32 - 179).abs() <= 1,
            "atteso ~179, ottenuto {}",
            canale(&su, 0)
        );
        assert!(
            (canale(&giu, 0) as i32 - 77).abs() <= 1,
            "atteso ~77, ottenuto {}",
            canale(&giu, 0)
        );
    }

    #[test]
    fn contrast_0_e_identita() {
        let Some(renderer) = HeadlessRenderer::new() else {
            eprintln!("nessuna GPU disponibile: test saltato");
            return;
        };
        let base = tile_di_prova(32, 24);
        let out = renderer
            .apply_effects(&base, &[Effect::contrast(0.0)])
            .expect("contrasto 0");
        assert_eq!(out.rgba, base.rgba, "contrasto 0 deve essere l'identità");
    }

    #[test]
    fn saturate_meno_1_diventa_grayscale() {
        let Some(renderer) = HeadlessRenderer::new() else {
            eprintln!("nessuna GPU disponibile: test saltato");
            return;
        };
        let base = tile_di_prova(24, 24);
        let out = renderer
            .apply_effects(&base, &[Effect::saturate(-1.0)])
            .expect("desatura");

        for px in out.rgba.chunks_exact(4) {
            let (r, g, b) = (px[0] as i32, px[1] as i32, px[2] as i32);
            assert!(
                (r - g).abs() <= 1 && (g - b).abs() <= 1,
                "pixel non grigio: {r},{g},{b}"
            );
        }
        assert_eq!(out.rgba[3], 255, "l'alpha non cambia");
    }

    #[test]
    fn saturate_0_e_identita() {
        let Some(renderer) = HeadlessRenderer::new() else {
            eprintln!("nessuna GPU disponibile: test saltato");
            return;
        };
        let base = tile_di_prova(24, 16);
        let out = renderer
            .apply_effects(&base, &[Effect::saturate(0.0)])
            .expect("saturazione 0");
        assert_eq!(out.rgba, base.rgba);
    }

    #[test]
    fn catena_di_tre_effetti() {
        let Some(renderer) = HeadlessRenderer::new() else {
            eprintln!("nessuna GPU disponibile: test saltato");
            return;
        };
        let base = grigio(128, 4);

        let catena = [
            Effect::brightness(0.1),
            Effect::contrast(0.2),
            Effect::saturate(0.3),
        ];
        let avanti = renderer.apply_effects(&base, &catena).expect("catena");

        let inversa = [
            Effect::contrast(0.2),
            Effect::brightness(0.1),
            Effect::saturate(0.3),
        ];
        let indietro = renderer
            .apply_effects(&base, &inversa)
            .expect("catena inversa");

        // brightness poi contrast: 0.5 -> 0.6 -> 0.62
        // contrast poi brightness: 0.5 -> 0.5 -> 0.6
        assert!(
            avanti.rgba[0] > indietro.rgba[0] + 2,
            "l'ordine deve contare: {} vs {}",
            avanti.rgba[0],
            indietro.rgba[0]
        );
        assert!(
            (avanti.rgba[0] as i32 - 158).abs() <= 2,
            "atteso ~158, dato {}",
            avanti.rgba[0]
        );
    }

    #[test]
    fn parita_cpu_gpu_brightness() {
        let Some(renderer) = HeadlessRenderer::new() else {
            eprintln!("nessuna GPU disponibile: test saltato");
            return;
        };
        let base = tile_di_prova(100, 100);
        let fx = [Effect::brightness(0.3)];
        let cpu_out = cpu::apply_effects(&base, &fx);
        let gpu_out = renderer.apply_effects(&base, &fx).expect("gpu");
        let (diff, at) = max_diff(&cpu_out.rgba, &gpu_out.rgba);
        assert!(diff <= 2, "brightness: {diff}/255 al byte {at}");
    }

    #[test]
    fn parita_cpu_gpu_contrast() {
        let Some(renderer) = HeadlessRenderer::new() else {
            eprintln!("nessuna GPU disponibile: test saltato");
            return;
        };
        let base = tile_di_prova(100, 100);
        let fx = [Effect::contrast(0.35)];
        let cpu_out = cpu::apply_effects(&base, &fx);
        let gpu_out = renderer.apply_effects(&base, &fx).expect("gpu");
        let (diff, at) = max_diff(&cpu_out.rgba, &gpu_out.rgba);
        assert!(diff <= 2, "contrast: {diff}/255 al byte {at}");
    }

    #[test]
    fn parita_cpu_gpu_saturate() {
        let Some(renderer) = HeadlessRenderer::new() else {
            eprintln!("nessuna GPU disponibile: test saltato");
            return;
        };
        let base = tile_di_prova(100, 100);
        let fx = [Effect::saturate(-0.6)];
        let cpu_out = cpu::apply_effects(&base, &fx);
        let gpu_out = renderer.apply_effects(&base, &fx).expect("gpu");
        let (diff, at) = max_diff(&cpu_out.rgba, &gpu_out.rgba);
        assert!(diff <= 2, "saturate: {diff}/255 al byte {at}");
    }

    #[test]
    fn l_input_non_viene_modificato() {
        let Some(renderer) = HeadlessRenderer::new() else {
            eprintln!("nessuna GPU disponibile: test saltato");
            return;
        };
        let base = tile_di_prova(40, 30);
        let prima = base.clone();
        let _ = renderer
            .apply_effects(&base, &[Effect::brightness(0.4), Effect::contrast(0.3)])
            .expect("gpu");
        assert_eq!(base, prima, "l'ingresso deve restare intatto");
    }

    #[test]
    fn blend_e_opacity_dell_effetto() {
        let Some(renderer) = HeadlessRenderer::new() else {
            eprintln!("nessuna GPU disponibile: test saltato");
            return;
        };
        let base = grigio(128, 4);
        // brightness(-0.3): 0.502 -> 0.202; Multiply con 0.502 -> 0.101;
        // opacità 0.5 -> (0.502 + 0.101) / 2 = 0.302 -> 77
        let fx = [Effect::brightness(-0.3)
            .with_blend(BlendMode::Multiply)
            .with_opacity(0.5)];
        let out = renderer.apply_effects(&base, &fx).expect("gpu");
        assert!(
            (out.rgba[0] as i32 - 77).abs() <= 2,
            "atteso ~77, ottenuto {}",
            out.rgba[0]
        );
        assert_eq!(out.rgba[3], 255, "l'alpha resta quello dell'ingresso");
    }

    #[test]
    fn effetto_su_tile_vuota_non_va_in_panico() {
        let Some(renderer) = HeadlessRenderer::new() else {
            eprintln!("nessuna GPU disponibile: test saltato");
            return;
        };
        let vuota = pizzart_core::PlacedImage::empty(64, 64);
        let out = renderer
            .apply_effects(&vuota, &[Effect::brightness(0.5)])
            .expect("tessera vuota");
        assert!(out.is_empty());
        assert_eq!(out.canvas_w, 64);
    }

    /// Ogni pass quantizza a 8 bit: una catena accumula l'arrotondamento.
    /// Il limite sale (2 → 5) ma resta dichiarato, invece di essere ignorato.
    #[test]
    fn parita_cpu_gpu_su_una_catena_lunga() {
        let Some(renderer) = HeadlessRenderer::new() else {
            eprintln!("nessuna GPU disponibile: test saltato");
            return;
        };
        let base = tile_di_prova(80, 60);
        let catena = [
            Effect::brightness(0.12),
            Effect::contrast(0.35),
            Effect::saturate(0.6),
            Effect::brightness(-0.05),
        ];
        let cpu_out = cpu::apply_effects(&base, &catena);
        let gpu_out = renderer.apply_effects(&base, &catena).expect("gpu");
        let (diff, at) = max_diff(&cpu_out.rgba, &gpu_out.rgba);
        println!(
            "catena di {} effetti: differenza max {diff}/255",
            catena.len()
        );
        assert!(
            diff <= 5,
            "catena: {diff}/255 al byte {at} (atteso ≤5 per 4 pass)"
        );
    }

    // ---------------------------------------------------------------------
    // Blur separabile (kernel)
    // ---------------------------------------------------------------------

    /// Immagine nera con un solo pixel bianco al centro.
    fn delta(lato: u32) -> pizzart_core::PlacedImage {
        let mut t = grigio(0, lato);
        let c = lato / 2;
        let i = ((c * lato + c) * 4) as usize;
        t.rgba[i] = 255;
        t.rgba[i + 1] = 255;
        t.rgba[i + 2] = 255;
        t
    }

    fn luminanza(t: &pizzart_core::PlacedImage, x: u32, y: u32) -> i32 {
        let i = ((y * t.width + x) * 4) as usize;
        (t.rgba[i] as i32 + t.rgba[i + 1] as i32 + t.rgba[i + 2] as i32) / 3
    }

    #[test]
    fn blur_su_delta_produce_un_kernel_gaussiano() {
        let Some(renderer) = HeadlessRenderer::new() else {
            eprintln!("nessuna GPU disponibile: test saltato");
            return;
        };
        let lato = 33u32;
        let base = delta(lato);
        let out = renderer
            .apply_effects(&base, &[Effect::blur(3.0)])
            .expect("blur");
        let c = lato / 2;

        // il centro resta il punto più chiaro
        let centro = luminanza(&out, c, c);
        assert!(centro > 0, "il centro deve ricevere luce");
        for d in 1..=4 {
            assert!(
                luminanza(&out, c + d, c) < luminanza(&out, c + d - 1, c),
                "il kernel deve decrescere a distanza {d}"
            );
        }

        // simmetria orizzontale e verticale
        for d in 1..=5 {
            let destra = luminanza(&out, c + d, c);
            let sinistra = luminanza(&out, c - d, c);
            assert!(
                (destra - sinistra).abs() <= 1,
                "asimmetria a {d}: {destra} vs {sinistra}"
            );
            let sotto = luminanza(&out, c, c + d);
            let sopra = luminanza(&out, c, c - d);
            assert!((sotto - sopra).abs() <= 1, "asimmetria verticale a {d}");
        }

        // l'energia si conserva (kernel normalizzato), entro la quantizzazione
        let somma: f64 = out.rgba.chunks_exact(4).map(|p| p[0] as f64).sum();
        let attesa = (lato * lato) as f64 * 255.0 / (lato * lato) as f64; // 255 totale
        assert!(
            (somma - attesa).abs() < attesa * 0.15,
            "energia {somma}, attesa ~{attesa}"
        );
    }

    #[test]
    fn parita_cpu_gpu_blur() {
        let Some(renderer) = HeadlessRenderer::new() else {
            eprintln!("nessuna GPU disponibile: test saltato");
            return;
        };
        let base = tile_di_prova(64, 64);
        for raggio in [1.0f32, 2.0, 4.0, 8.0] {
            let fx = [Effect::blur(raggio)];
            let cpu_out = cpu::apply_effects(&base, &fx);
            let gpu_out = renderer.apply_effects(&base, &fx).expect("gpu");
            let (diff, at) = max_diff(&cpu_out.rgba, &gpu_out.rgba);
            println!("blur raggio {raggio}: differenza max {diff}/255");
            assert!(diff <= 3, "blur raggio {raggio}: {diff}/255 al byte {at}");
        }
    }

    #[test]
    fn blur_su_immagine_uniforme_e_identita() {
        let Some(renderer) = HeadlessRenderer::new() else {
            eprintln!("nessuna GPU disponibile: test saltato");
            return;
        };
        let base = grigio(128, 48);
        let out = renderer
            .apply_effects(&base, &[Effect::blur(6.0)])
            .expect("blur");
        for (i, px) in out.rgba.chunks_exact(4).enumerate() {
            assert!(
                (px[0] as i32 - 128).abs() <= 1,
                "pixel {i} è {} invece di 128",
                px[0]
            );
        }
    }

    #[test]
    fn blur_su_bordo_non_produce_artefatti() {
        let Some(renderer) = HeadlessRenderer::new() else {
            eprintln!("nessuna GPU disponibile: test saltato");
            return;
        };
        // Metà nera, metà bianca: è il caso che smaschera il padding a zero.
        let lato = 32u32;
        let mut base = grigio(0, lato);
        for y in 0..lato {
            for x in lato / 2..lato {
                let i = ((y * lato + x) * 4) as usize;
                base.rgba[i..i + 4].copy_from_slice(&[255, 255, 255, 255]);
            }
        }
        let out = renderer
            .apply_effects(&base, &[Effect::blur(6.0)])
            .expect("blur");

        // il bordo sinistro resta nero: i campioni fuori campo sono "clamp to edge"
        assert!(
            luminanza(&out, 0, lato / 2) <= 2,
            "il bordo nero non deve schiarirsi: {}",
            luminanza(&out, 0, lato / 2)
        );
        // e il bordo destro resta bianco, non si scurisce
        assert!(
            luminanza(&out, lato - 1, lato / 2) >= 250,
            "il bordo bianco non deve scurirsi: {}",
            luminanza(&out, lato - 1, lato / 2)
        );
        // i due estremi sono già verificati sopra: niente overshoot oltre il bianco
    }

    #[test]
    fn blur_raggio_zero_e_identita() {
        let Some(renderer) = HeadlessRenderer::new() else {
            eprintln!("nessuna GPU disponibile: test saltato");
            return;
        };
        let base = tile_di_prova(40, 32);
        let out = renderer
            .apply_effects(&base, &[Effect::blur(0.0)])
            .expect("blur");
        assert_eq!(out.rgba, base.rgba, "raggio 0 non deve cambiare nulla");
    }

    // ---------------------------------------------------------------------
    // Maschere: canale alpha applicato al layer
    // ---------------------------------------------------------------------

    use pizzart_core::{Mask, MaskSource};

    fn doc_con_maschera(mask: Mask) -> Document {
        let (w, h) = (32u32, 32u32);
        let mut doc = Document::new(w, h);
        doc.add_layer(Layer::filled("fondo", w, h, [30, 60, 180, 255]));
        let mut sopra = Layer::filled("rosso", w, h, [220, 40, 40, 255]);
        sopra.mask = Some(std::sync::Arc::new(mask));
        doc.add_layer(sopra);
        doc
    }

    #[test]
    fn parita_cpu_gpu_selezione_rettangolo() {
        let Some(renderer) = HeadlessRenderer::new() else {
            eprintln!("nessuna GPU disponibile: test saltato");
            return;
        };
        use pizzart_core::RectSelection;

        let (w, h) = (64u32, 48u32);
        let mut doc = Document::new(w, h);
        doc.add_layer(Layer::filled("fondo", w, h, [30, 60, 180, 255]));
        // layer con effetto applicato alla tessera, rivelato solo dalla selezione
        let mut sopra = Layer::filled("schiarito", w, h, [230, 200, 90, 255]);
        sopra.mask = Some(std::sync::Arc::new(
            RectSelection {
                x: 8.0,
                y: 6.0,
                w: 30.0,
                h: 20.0,
            }
            .to_mask(w, h),
        ));
        doc.add_layer(sopra);

        let cpu = doc.composite();
        let gpu = renderer.render(&doc).expect("render");
        let (diff, at) = max_diff(&cpu, &gpu.rgba);
        println!("selezione rettangolo: differenza max {diff}/255");
        assert!(diff <= 2, "selezione: {diff}/255 al byte {at}");
    }

    #[test]
    fn parita_cpu_gpu_mask_semplice() {
        let Some(renderer) = HeadlessRenderer::new() else {
            eprintln!("nessuna GPU disponibile: test saltato");
            return;
        };
        let mask = Mask::new(vec![128; 32 * 32], 32, 32, MaskSource::Procedural);
        let doc = doc_con_maschera(mask);

        let cpu = doc.composite();
        let gpu = renderer.render(&doc).expect("render");
        let (diff, at) = max_diff(&cpu, &gpu.rgba);
        println!("mask 50%: differenza max {diff}/255");
        assert!(diff <= 2, "mask semplice: {diff}/255 al byte {at}");
    }

    #[test]
    fn parita_cpu_gpu_mask_inverted() {
        let Some(renderer) = HeadlessRenderer::new() else {
            eprintln!("nessuna GPU disponibile: test saltato");
            return;
        };
        let mut mask = Mask::new(vec![200; 32 * 32], 32, 32, MaskSource::Procedural);
        mask.set_inverted(true);
        let doc = doc_con_maschera(mask);

        let cpu = doc.composite();
        let gpu = renderer.render(&doc).expect("render");
        let (diff, at) = max_diff(&cpu, &gpu.rgba);
        println!("mask invertita: differenza max {diff}/255");
        assert!(diff <= 2, "mask invertita: {diff}/255 al byte {at}");
    }

    // ---------------------------------------------------------------------
    // Albero: gruppi, fusione, appiattimento
    // ---------------------------------------------------------------------

    use pizzart_core::{Group, Node};

    fn doc_con_gruppi() -> Document {
        let (w, h) = (48u32, 40u32);
        let mut doc = Document::new(w, h);
        doc.add_layer(Layer::filled("fondo", w, h, [240, 240, 245, 255]));

        let mut esterno = Group::new("esterno");
        let mut interno = Group::new("interno");
        interno.add(Node::Layer(Layer::filled("r", w, h, [220, 60, 50, 200])));
        let mut giallo = Layer::filled("g", w, h, [240, 200, 60, 180]);
        giallo.opacity = 0.7;
        interno.add(Node::Layer(giallo));
        esterno.add(Node::Group(interno));

        let mut blu = Layer::filled("b", w, h, [40, 80, 200, 220]);
        blu.blend = BlendMode::Multiply;
        esterno.add(Node::Layer(blu));
        esterno.opacity = 0.85;
        doc.add_node(Node::Group(esterno));
        doc
    }

    #[test]
    fn parita_cpu_gpu_con_gruppi() {
        let Some(renderer) = HeadlessRenderer::new() else {
            eprintln!("nessuna GPU disponibile: test saltato");
            return;
        };
        let doc = doc_con_gruppi();
        let cpu = doc.composite();
        let gpu = renderer.render(&doc).expect("render");
        let (diff, at) = max_diff(&cpu, &gpu.rgba);
        println!("gruppi annidati: differenza max {diff}/255");
        assert!(diff <= 2, "gruppi: {diff}/255 al byte {at}");
    }

    #[test]
    fn parita_cpu_gpu_dopo_merge_down() {
        let Some(renderer) = HeadlessRenderer::new() else {
            eprintln!("nessuna GPU disponibile: test saltato");
            return;
        };
        let (w, h) = (32u32, 32u32);
        let mut doc = Document::new(w, h);
        doc.add_layer(Layer::filled("fondo", w, h, [250, 250, 250, 255]));
        let mut sopra = Layer::filled("sopra", w, h, [60, 120, 230, 200]);
        sopra.blend = BlendMode::Screen;
        let sopra_id = doc.add_layer(sopra);

        let prima_gpu = renderer.render(&doc).expect("render prima");
        doc.merge_down(sopra_id).expect("fusione");
        let dopo_gpu = renderer.render(&doc).expect("render dopo");

        let (diff, at) = max_diff(&prima_gpu.rgba, &dopo_gpu.rgba);
        println!("dopo merge: differenza max {diff}/255");
        assert!(diff <= 2, "merge: {diff}/255 al byte {at}");
        assert_eq!(
            doc.composite(),
            dopo_gpu.rgba,
            "CPU e GPU concordano dopo il merge"
        );
    }

    #[test]
    fn parita_cpu_gpu_dopo_flatten_group() {
        let Some(renderer) = HeadlessRenderer::new() else {
            eprintln!("nessuna GPU disponibile: test saltato");
            return;
        };
        let (w, h) = (40u32, 40u32);
        let mut doc = Document::new(w, h);
        doc.add_layer(Layer::filled("fondo", w, h, [20, 20, 30, 255]));

        let mut gruppo = Group::new("gruppo");
        gruppo.add(Node::Layer(Layer::filled("a", w, h, [200, 90, 40, 160])));
        gruppo.opacity = 0.6;
        let gid = doc.add_node(Node::Group(gruppo));

        let prima_gpu = renderer.render(&doc).expect("render prima");
        doc.flatten_group(gid).expect("appiattimento");
        let dopo_gpu = renderer.render(&doc).expect("render dopo");

        let (diff, at) = max_diff(&prima_gpu.rgba, &dopo_gpu.rgba);
        println!("dopo flatten: differenza max {diff}/255");
        assert!(diff <= 2, "flatten: {diff}/255 al byte {at}");
        assert_eq!(
            doc.composite(),
            dopo_gpu.rgba,
            "CPU e GPU concordano dopo il flatten"
        );
    }

    /// Un testo come maschera di un layer colorato: la CPU e la GPU devono
    /// concordare anche su un alpha che viene da un font, non da un rettangolo.
    #[test]
    fn parita_cpu_gpu_mask_from_tile() {
        let Some(renderer) = HeadlessRenderer::new() else {
            eprintln!("nessuna GPU disponibile: test saltato");
            return;
        };
        let Some(bytes) = pizzart_text::test_font_bytes() else {
            eprintln!("nessun font di sistema: test saltato");
            return;
        };
        let mut registry = FontRegistry::new();
        let Ok(id) = registry.add(bytes, FontSource::Memory) else {
            eprintln!("font non caricabile: test saltato");
            return;
        };
        let family = registry.get(id).expect("info").family.clone();

        let (w, h) = (240u32, 100u32);
        let testo = Text::new("Maschera", TextStyle::new(56.0).family(family)).at(10.0, 10.0);
        let tile = testo.render(&mut registry);
        let mask = Mask::from_tile_placed(&tile, w, h);

        let mut doc = Document::new(w, h);
        doc.add_layer(Layer::filled("fondo", w, h, [240, 240, 245, 255]));
        let mut sopra = Layer::filled("colore", w, h, [40, 170, 90, 255]);
        sopra.mask = Some(std::sync::Arc::new(mask));
        doc.add_layer(sopra);

        let cpu = doc.composite();
        let gpu = renderer.render(&doc).expect("render");
        let (diff, at) = max_diff(&cpu, &gpu.rgba);
        println!("mask da testo: differenza max {diff}/255");
        assert!(diff <= 3, "mask da testo: {diff}/255 al byte {at}");
    }

    /// La prova visiva: il testo rivela il layer verde, il resto mostra il fondo.
    #[test]
    fn mask_di_testo_su_layer_colorato() {
        let Some(renderer) = HeadlessRenderer::new() else {
            eprintln!("nessuna GPU disponibile: test saltato");
            return;
        };
        let Some(bytes) = pizzart_text::test_font_bytes() else {
            eprintln!("nessun font di sistema: test saltato");
            return;
        };
        let mut registry = FontRegistry::new();
        let Ok(id) = registry.add(bytes, FontSource::Memory) else {
            eprintln!("font non caricabile: test saltato");
            return;
        };
        let family = registry.get(id).expect("info").family.clone();

        let (w, h) = (240u32, 100u32);
        let tile = Text::new("ABC", TextStyle::new(72.0).family(family))
            .at(12.0, 8.0)
            .render(&mut registry);
        assert!(!tile.is_empty());
        let mask = Mask::from_tile_placed(&tile, w, h);

        let mut doc = Document::new(w, h);
        doc.add_layer(Layer::filled("fondo", w, h, [250, 250, 250, 255]));
        let mut verde = Layer::filled("verde", w, h, [20, 180, 60, 255]);
        verde.mask = Some(std::sync::Arc::new(mask));
        doc.add_layer(verde);

        let img = renderer.render(&doc).expect("render");

        // dentro una lettera: verde del layer
        let mut dentro = None;
        for y in 0..h {
            for x in 0..w {
                let p = img.pixel(x, y).expect("pixel");
                if p[1] > 150 && p[0] < 60 {
                    dentro = Some(p);
                    break;
                }
            }
            if dentro.is_some() {
                break;
            }
        }
        let dentro = dentro.expect("almeno un pixel verde (dentro una lettera)");
        assert_eq!(dentro[3], 255, "le lettere sono opache");

        // in un angolo lontano dal testo: il fondo bianco
        let angolo = img.pixel(w - 2, h - 2).expect("pixel");
        assert_eq!(
            angolo,
            [250, 250, 250, 255],
            "fuori dal testo resta il fondo"
        );
    }

    use pizzart_text::{FontRegistry, FontSource, Text, TextStyle};

    /// Il testo è una tessera come uno stroke: la GPU deve comporla identica.
    #[test]
    fn parita_cpu_gpu_text() {
        let Some(renderer) = HeadlessRenderer::new() else {
            eprintln!("nessuna GPU disponibile: test saltato");
            return;
        };
        let Some(bytes) = pizzart_text::test_font_bytes() else {
            eprintln!("nessun font di sistema: test saltato");
            return;
        };
        let mut registry = FontRegistry::new();
        let Ok(id) = registry.add(bytes, FontSource::Memory) else {
            eprintln!("font non caricabile: test saltato");
            return;
        };
        let family = registry.get(id).expect("info").family.clone();

        let testo = Text::new(
            "Pizzart 123",
            TextStyle::new(44.0)
                .family(family)
                .color([28, 88, 200, 255]),
        )
        .at(8.0, 6.0);
        let tile = testo.render(&mut registry);
        assert!(!tile.is_empty(), "il testo deve produrre pixel");

        let (w, h) = (320u32, 90u32);
        let mut doc = Document::new(w, h);
        doc.add_layer(Layer::filled("fondo", w, h, [250, 250, 250, 255]));

        // CPU: la tessera composta sopra il fondo, pixel per pixel.
        let mut cpu = doc.composite();
        for row in 0..tile.height {
            for col in 0..tile.width {
                let x = tile.offset_x + col as i32;
                let y = tile.offset_y + row as i32;
                if x < 0 || y < 0 || x >= w as i32 || y >= h as i32 {
                    continue;
                }
                let src = tile.pixel(col, row).expect("pixel della tessera");
                if src[3] == 0 {
                    continue;
                }
                let di = ((y as u32 * w + x as u32) * 4) as usize;
                pizzart_core::blend::blend_px(&mut cpu[di..di + 4], &src, BlendMode::Normal, 1.0);
            }
        }

        let layer = renderer
            .upload_tile(&tile, BlendMode::Normal, 1.0)
            .expect("upload");
        let gpu = renderer
            .render_document_with_tiles(&doc, std::slice::from_ref(&layer))
            .expect("render");

        let (diff, at) = max_diff(&cpu, &gpu.rgba);
        println!("testo: differenza max {diff}/255");
        assert!(
            diff <= 3,
            "testo: {diff}/255 al byte {at} (CPU={} GPU={})",
            cpu[at],
            gpu.rgba[at]
        );
    }

    // ---------------------------------------------------------------------
    // Sharpen e glow: kernel + combinazione a due ingressi
    // ---------------------------------------------------------------------

    /// Metà sinistra scura, metà destra chiara: un gradino netto.
    fn gradino(lato: u32, sinistra: u8, destra: u8) -> pizzart_core::PlacedImage {
        let mut t = grigio(sinistra, lato);
        for y in 0..lato {
            for x in lato / 2..lato {
                let i = ((y * lato + x) * 4) as usize;
                t.rgba[i..i + 4].copy_from_slice(&[destra, destra, destra, 255]);
            }
        }
        t
    }

    #[test]
    fn sharpen_aumenta_il_contrasto_sui_bordi() {
        let Some(renderer) = HeadlessRenderer::new() else {
            eprintln!("nessuna GPU disponibile: test saltato");
            return;
        };
        let lato = 40u32;
        let base = gradino(lato, 64, 192);
        let out = renderer
            .apply_effects(&base, &[Effect::sharpen(0.5, 4.0)])
            .expect("sharpen");

        let confine = lato / 2;
        let scuro = luminanza(&out, confine - 1, lato / 2);
        let chiaro = luminanza(&out, confine, lato / 2);
        assert!(
            scuro < 64,
            "sul lato scuro deve esserci un undershoot: {scuro} (atteso < 64)"
        );
        assert!(
            chiaro > 192,
            "sul lato chiaro deve esserci un overshoot: {chiaro} (atteso > 192)"
        );

        // lontano dal bordo non cambia nulla
        assert!(
            (luminanza(&out, 2, lato / 2) as i32 - 64).abs() <= 1,
            "il centro della zona scura deve restare 64"
        );
    }

    #[test]
    fn sharpen_su_immagine_uniforme_e_identita() {
        let Some(renderer) = HeadlessRenderer::new() else {
            eprintln!("nessuna GPU disponibile: test saltato");
            return;
        };
        let base = grigio(128, 48);
        let out = renderer
            .apply_effects(&base, &[Effect::sharpen(1.0, 6.0)])
            .expect("sharpen");
        for (i, px) in out.rgba.chunks_exact(4).enumerate() {
            assert!(
                (px[0] as i32 - 128).abs() <= 1,
                "pixel {i} è {} invece di 128: un'uniforme non deve cambiare",
                px[0]
            );
        }
    }

    #[test]
    fn sharpen_amount_0_e_identita() {
        let Some(renderer) = HeadlessRenderer::new() else {
            eprintln!("nessuna GPU disponibile: test saltato");
            return;
        };
        let base = gradino(32, 40, 200);
        let out = renderer
            .apply_effects(&base, &[Effect::sharpen(0.0, 4.0)])
            .expect("sharpen");
        assert_eq!(out.rgba, base.rgba, "amount 0 non deve cambiare nulla");
    }

    #[test]
    fn glow_su_pixel_bianco_su_nero_crea_alone() {
        let Some(renderer) = HeadlessRenderer::new() else {
            eprintln!("nessuna GPU disponibile: test saltato");
            return;
        };
        let lato = 41u32;
        let base = delta(lato);
        // Soglia 0: un singolo pixel bianco non supera soglie alte dopo lo
        // sfocato, quindi un alone misurabile richiede "tutto ciò che è > 0".
        // Il comportamento della soglia è coperto da `glow_sotto_soglia_non_fa_nulla`.
        let out = renderer
            .apply_effects(&base, &[Effect::glow(0.0, 2.0, 5.0)])
            .expect("glow");
        let c = lato / 2;

        assert_eq!(luminanza(&out, c, c), 255, "il centro resta bianco");
        // l'alone si estende oltre il pixel: è la prova che il kernel agisce
        assert!(
            luminanza(&out, c + 3, c) > 0,
            "atteso alone a 3 pixel dal centro, trovato {}",
            luminanza(&out, c + 3, c)
        );
        assert!(
            luminanza(&out, c + 3, c) < luminanza(&out, c, c),
            "l'alone deve essere più debole del centro"
        );
        // e si spegne lontano
        assert_eq!(luminanza(&out, 1, 1), 0, "lontano dal pixel resta nero");
    }

    #[test]
    fn glow_sotto_soglia_non_fa_nulla() {
        let Some(renderer) = HeadlessRenderer::new() else {
            eprintln!("nessuna GPU disponibile: test saltato");
            return;
        };
        // tutto scuro: lo sfocato resta sotto la soglia 0.5
        let base = grigio(20, 48);
        let out = renderer
            .apply_effects(&base, &[Effect::glow(0.5, 2.0, 6.0)])
            .expect("glow");
        for (i, px) in out.rgba.chunks_exact(4).enumerate() {
            assert!(
                (px[0] as i32 - 20).abs() <= 1,
                "pixel {i} è {} invece di 20: sotto soglia non deve cambiare",
                px[0]
            );
        }
    }

    #[test]
    fn parita_cpu_gpu_sharpen() {
        let Some(renderer) = HeadlessRenderer::new() else {
            eprintln!("nessuna GPU disponibile: test saltato");
            return;
        };
        let base = tile_di_prova(100, 100);
        let fx = [Effect::sharpen(0.8, 4.0)];
        let cpu_out = cpu::apply_effects(&base, &fx);
        let gpu_out = renderer.apply_effects(&base, &fx).expect("gpu");
        let (diff, at) = max_diff(&cpu_out.rgba, &gpu_out.rgba);
        println!("sharpen: differenza max {diff}/255");
        assert!(diff <= 2, "sharpen: {diff}/255 al byte {at}");
    }

    #[test]
    fn parita_cpu_gpu_glow() {
        let Some(renderer) = HeadlessRenderer::new() else {
            eprintln!("nessuna GPU disponibile: test saltato");
            return;
        };
        let base = tile_di_prova(100, 100);
        let fx = [Effect::glow(0.2, 1.5, 6.0)];
        let cpu_out = cpu::apply_effects(&base, &fx);
        let gpu_out = renderer.apply_effects(&base, &fx).expect("gpu");
        let (diff, at) = max_diff(&cpu_out.rgba, &gpu_out.rgba);
        println!("glow: differenza max {diff}/255");
        assert!(diff <= 2, "glow: {diff}/255 al byte {at}");
    }

    /// Il contratto del flag `is_last`: blend e opacità si applicano **una volta**.
    /// Con tre pass, comporre a ogni pass darebbe un risultato molto diverso.
    #[test]
    fn l_ultimo_pass_solo_compone_per_sharpen_e_glow() {
        let Some(renderer) = HeadlessRenderer::new() else {
            eprintln!("nessuna GPU disponibile: test saltato");
            return;
        };
        let base = tile_di_prova(64, 48);

        for (nome, fx) in [
            (
                "sharpen",
                Effect::sharpen(0.8, 3.0)
                    .with_blend(BlendMode::Multiply)
                    .with_opacity(0.5),
            ),
            (
                "glow",
                Effect::glow(0.15, 1.2, 4.0)
                    .with_blend(BlendMode::Multiply)
                    .with_opacity(0.5),
            ),
        ] {
            let cpu_out = cpu::apply_effects(&base, &[fx]);
            let gpu_out = renderer.apply_effects(&base, &[fx]).expect("gpu");
            let (diff, at) = max_diff(&cpu_out.rgba, &gpu_out.rgba);
            println!("{nome} con Multiply 0.5: differenza max {diff}/255");
            assert!(
                diff <= 3,
                "{nome}: composizione non una-sola-volta, {diff}/255 al byte {at}"
            );
        }
    }

    #[test]
    fn catena_blur_sharpen_glow() {
        let Some(renderer) = HeadlessRenderer::new() else {
            eprintln!("nessuna GPU disponibile: test saltato");
            return;
        };
        let base = tile_di_prova(64, 64);

        let solo_blur = renderer
            .apply_effects(&base, &[Effect::blur(3.0)])
            .expect("blur");
        let solo_sharpen = renderer
            .apply_effects(&base, &[Effect::sharpen(0.8, 3.0)])
            .expect("sharpen");
        let solo_glow = renderer
            .apply_effects(&base, &[Effect::glow(0.2, 1.2, 3.0)])
            .expect("glow");
        let catena_fx = [
            Effect::blur(3.0),
            Effect::sharpen(0.8, 3.0),
            Effect::glow(0.2, 1.2, 3.0),
        ];
        let catena = renderer.apply_effects(&base, &catena_fx).expect("catena");

        assert_ne!(catena.rgba, solo_blur.rgba);
        assert_ne!(catena.rgba, solo_sharpen.rgba);
        assert_ne!(catena.rgba, solo_glow.rgba);

        // e la catena deve combaciare con il riferimento CPU
        let cpu_out = cpu::apply_effects(&base, &catena_fx);
        let (diff, at) = max_diff(&cpu_out.rgba, &catena.rgba);
        println!("catena blur+sharpen+glow: differenza max {diff}/255");
        assert!(diff <= 5, "catena: {diff}/255 al byte {at}");
    }

    #[test]
    fn blur_di_raggio_8_su_512_e_veloce() {
        let Some(renderer) = HeadlessRenderer::new() else {
            eprintln!("nessuna GPU disponibile: test saltato");
            return;
        };
        let base = tile_di_prova(512, 512);
        let fx = [Effect::blur(8.0)];

        // La prima chiamata paga la compilazione pigra del pipeline su DX12:
        // si misura a caldo, e il costo a freddo si stampa come informazione.
        let freddo = std::time::Instant::now();
        renderer.apply_effects(&base, &fx).expect("warmup");
        let freddo_ms = freddo.elapsed().as_secs_f64() * 1000.0;

        // Il minimo di più esecuzioni è la misura robusta: la mediana, con i test
        // del workspace in esecuzione in parallelo, misura la contesa della GPU,
        // non il costo del kernel.
        let mut tempi = Vec::new();
        for _ in 0..5 {
            let inizio = std::time::Instant::now();
            let out = renderer.apply_effects(&base, &fx).expect("blur");
            tempi.push(inizio.elapsed().as_secs_f64() * 1000.0);
            assert!(!out.is_empty());
        }
        tempi.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let caldo = tempi[0];

        // Da dove viene il tempo? Un effetto a un pass misura il costo fisso
        // (upload + readback); la differenza è il costo vero del kernel.
        let mut base_tempi = Vec::new();
        for _ in 0..3 {
            let inizio = std::time::Instant::now();
            renderer
                .apply_effects(&base, &[Effect::brightness(0.1)])
                .expect("brightness");
            base_tempi.push(inizio.elapsed().as_secs_f64() * 1000.0);
        }
        base_tempi.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let fisso = base_tempi[0];

        println!(
            "512x512 — costo fisso (1 pass + readback): {fisso:.2} ms; \
             blur raggio 8: {caldo:.2} ms (minimo di 5); margine del kernel: {:.2} ms; prima chiamata: {freddo_ms:.0} ms",
            caldo - fisso
        );

        if cfg!(debug_assertions) {
            assert!(caldo < 60.0, "in debug il limite è morbido: {caldo:.2} ms");
        } else {
            assert!(caldo < 20.0, "obiettivo di progetto: {caldo:.2} ms");
        }
    }
}
