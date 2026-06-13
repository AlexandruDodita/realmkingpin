use std::sync::Arc;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::NamedKey,
    window::{Window, WindowId},
};

pub struct State {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pub pipeline: wgpu::RenderPipeline,
    texture_bind_group_layout: wgpu::BindGroupLayout,
    mesh: Mesh,
    texture: Texture,
    material: Material,
}

pub struct Mesh {
    pub vertex_buffer: wgpu::Buffer,
    // pub vertex_count: u32,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
    // pub pipeline: wgpu::RenderPipeline,
    // pub bind_group: wgpu::BindGroup,
}

pub struct Texture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    // pub bind_group: wgpu::BindGroup,
}

pub struct Material {
    pub bind_group: wgpu::BindGroup,
    //later: uniform buffer for params, handles to textures it references
}

impl Mesh {
    pub fn new(device: &wgpu::Device) -> Self {
        
        //position x,y, uv u,v
        let vertices: [f32; 16] = [
            -0.5, 0.5, 0.0, 0.0, // top left
            0.5, 0.5, 1.0, 0.0,  // top right
            -0.5, -0.5, 0.0, 1.0, // bottom left
            0.5, -0.5, 1.0, 1.0,  // bottom right
        ];

        let indices: [u16; 6] = [
            0, 1, 2, // first triangle
            2, 1, 3, // second triangle
        ];

        let vertex_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        // let vertex_count = vertices.len() as u32 / 2; // unused since we switched to indexed drawing, but might be useful later if we add non-indexed meshes
        let index_count = indices.len() as u32;
      

        Self { vertex_buffer, /*vertex_count,*/ index_buffer, index_count, }
    }
}

impl Texture {
    pub fn from_bytes(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bytes: &[u8],
    ) -> anyhow::Result<Self> {
        let img = image::load_from_memory(bytes)?.to_rgba8();
        let (tex_w, tex_h) = img.dimensions();
        let tex_size = wgpu::Extent3d { width: tex_w, height: tex_h, depth_or_array_layers: 1 };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Texture"),
            size: tex_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &img,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * tex_w),
                rows_per_image: Some(tex_h),
            },
            tex_size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        //     label: Some("Texture Bind Group"),
        //     layout: bind_group_layout,
        //     entries: &[
        //         wgpu::BindGroupEntry {
        //             binding: 0,
        //             resource: wgpu::BindingResource::TextureView(&view),
        //         },
        //         wgpu::BindGroupEntry {
        //             binding: 1,
        //             resource: wgpu::BindingResource::Sampler(&sampler),
        //         },
        //     ],
        // });

        Ok(Self { texture, view, sampler /*, bind_group*/ })
    }
}

impl Material {
    pub fn new(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, texture: &Texture) -> Self {
        // For now the material just wraps a bind group, but later it might also own a uniform buffer for params, handles to textures it references, etc.
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Material Bind Group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&texture.sampler),
                },
            ],
        });
        Self { bind_group }
    }
}

impl State {
    pub fn new(window: Arc<Window>) -> anyhow::Result<Self> {
        let size = window.inner_size();
        let width = size.width.max(1);
        let height = size.height.max(1);


        let instance = wgpu::Instance::default();

        // The Arc<Window> is owned by the surface, so this is Surface<'static>.
        let surface = instance.create_surface(window.clone())?;

        // request_adapter / request_device are async; block on them with pollster.
        let adapter = pollster::block_on(instance.request_adapter(
            &wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            },
        ))?;

        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))?;

        let config = surface
            .get_default_config(&adapter, width, height)
            .ok_or_else(|| anyhow::anyhow!("surface not supported by this adapter"))?;
        surface.configure(&device, &config);

        let mesh = Mesh::new(&device);
        let texture_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Texture Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Mesh Pipeline Layout"),
            bind_group_layouts: &[Some(&texture_bind_group_layout)],
            immediate_size: 0, //replaces push_constant_ranges
        });
        
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Mesh Shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!("shader.wgsl"))),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Mesh Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: 4 * std::mem::size_of::<f32>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: 
                    &[wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x2,
                        offset: 0,
                        shader_location: 0,
                    },
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x2,
                        offset: 2 * std::mem::size_of::<f32>() as wgpu::BufferAddress,
                        shader_location: 1,
                    }],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format, // match surface config
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let texture = Texture::from_bytes(&device, &queue, include_bytes!("test.png"))?;
        let material = Material::new(&device, &texture_bind_group_layout, &texture);

        println!("State created: {width}x{height}, format {:?}", config.format);



        Ok(Self { window, surface, device, queue, config, mesh, pipeline, texture_bind_group_layout, texture, material })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    pub fn render(&mut self) {
        // wgpu 29: get_current_texture returns an enum, not a Result.
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f)
            | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                return;
            }
            // Timeout / Occluded / Validation: skip this frame.
            _ => return,
        };

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        

        

        

        {
            // This clear is what actually draws to the window. Without it the
            // compositor never gets a committed buffer, so nothing shows.
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None, // new field in wgpu 29
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.2,
                            b: 0.3,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, &self.material.bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.mesh.vertex_buffer.slice(..));
            render_pass.set_index_buffer(self.mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..self.mesh.index_count, 0, 0..1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();

        // Continuous game loop. Delete this line if you want on-demand rendering
        // (then also use ControlFlow::Wait in main).
        self.window.request_redraw();
    }
}

#[derive(Default)]
pub struct App {
    state: Option<State>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let attrs = Window::default_attributes().with_title("Realm Kingpin");
        match event_loop.create_window(attrs) {
            Ok(window) => {
                let window = Arc::new(window);
                match State::new(window.clone()) {
                    Ok(state) => {
                        self.state = Some(state);
                        window.request_redraw();
                    }
                    Err(e) => {
                        eprintln!("Failed to init wgpu: {e:?}");
                        event_loop.exit();
                    }
                }
            }
            Err(e) => {
                eprintln!("Failed to create window: {e}");
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(state) = &mut self.state else { return };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => state.resize(size.width, size.height),
            WindowEvent::RedrawRequested => state.render(),
            WindowEvent::KeyboardInput { event: key_event, .. } => {
                if key_event.logical_key == NamedKey::Escape {
                    event_loop.exit();
                }
            }
            _ => {}
        }
    }
}

fn main() -> anyhow::Result<()> {
    env_logger::init();
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::default();
    event_loop.run_app(&mut app)?;
    Ok(())
}