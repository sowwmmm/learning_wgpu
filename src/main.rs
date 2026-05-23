use std::sync::Arc;

use anyhow::*;
use wgpu::{util::DeviceExt, *};
use winit::{
    application::ApplicationHandler,
    event::*,
    event_loop::{self, ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowAttributes},
};

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    color: [f32; 3],
}

const VERTICES: &[Vertex] = &[
    Vertex { position: [0.0, 0.5, 0.0], color: [1.0, 0.0, 0.0] },
    Vertex { position: [-0.5, -0.5, 0.0], color: [0.0, 1.0, 0.0] },
    Vertex { position: [0.5, -0.5, 0.0], color: [0.0, 0.0, 1.0] },
];

pub struct State {
    surface: Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    is_surface_configured: bool,
    window: Arc<Window>,
    render_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    num_vertices: u32,
}

impl State {
    async fn new(window: Arc<Window>) -> anyhow::Result<Self>
    {
        let num_vertices = VERTICES.len() as u32;

        let size = window.inner_size();
        // get instance
        let instance = Instance::new(&InstanceDescriptor { 
            backends: wgpu::Backends::PRIMARY, 
            flags: Default::default(), 
            memory_budget_thresholds: Default::default(), 
            backend_options: Default::default() 
        });
        // surface
        let surface = instance.create_surface(window.clone()).unwrap();
        // adapter (physical gpu)
        let adapter = instance.request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::default(),
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }).await?;
        // Logical GPU & Send Commands Queue
        let (device, queue) = adapter.request_device(&DeviceDescriptor {
            label: None,
            required_features: Features::empty(),
            experimental_features: ExperimentalFeatures::disabled(),
            required_limits: Limits::default(),
            memory_hints: Default::default(),
            trace: Trace::Off
        }).await?;
        // get surface capabilities of the gpu
        let surface_cap = surface.get_capabilities(&adapter);
        // return the first sRGB surface format
        let surface_format = surface_cap
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_cap.formats[0]);
        // configure the surface for that gpu
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: surface_cap.present_modes[0],
            alpha_mode: surface_cap.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2
        };

        let shader = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));

        let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Pipeline Layout"),
            bind_group_layouts: &[],
            immediate_size: 0,
        });
        let render_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor { 
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout), 
            vertex: VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Vertex::desc()],
                compilation_options: PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState { // 3.
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState { // 4.
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        }
    );

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let vertex_buffer_layout = VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        };
        // return the state
        Ok(Self {
            surface,
            device,
            queue,
            config,
            is_surface_configured: false,
            render_pipeline,
            vertex_buffer,
            window,
            num_vertices,
        })
    }
    pub fn resize(&mut self, width: u32, height: u32) 
    {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
            self.is_surface_configured = true;
        }
    }

    fn handle_key(&self, event_loop: &ActiveEventLoop, code: KeyCode, is_key_pressed: bool) {
        match (code, is_key_pressed) {
            (KeyCode::Escape, true) => {
                event_loop.exit();
            }
            _ => {}
        }
    }
    fn update(&mut self) { /* empty [ currenctly we do not do any contionous work ] */ }
    fn render(&mut self) -> Result<()> {
        self.window.request_redraw();

        if !self.is_surface_configured {
            return Ok(());
        } else {
            let output = match self.surface.get_current_texture() {
                std::result::Result::Ok(texture) => texture,
                std::result::Result::Err(SurfaceError::Lost) => {
                    self.surface.configure(&self.device, &self.config);
                    return Ok(());
                }
                std::result::Result::Err(SurfaceError::Outdated) => {
                    return Ok(());
                }
                std::result::Result::Err(e) => {
                    return Err(anyhow!("{e}"));
                }
            };

            let view = output
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());

            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Render Encoder"),
                });

            { 
                let mut _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Render Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        depth_slice: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::RED),
                            store: StoreOp::Store,
                        },
                    })],
                            depth_stencil_attachment: None,

                            occlusion_query_set: None,

                            timestamp_writes: None,

                            multiview_mask: None,
                });
                // render()
                _render_pass.set_pipeline(&self.render_pipeline);
                // NEW!
                _render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                // render
                _render_pass.draw(0..self.num_vertices, 0..1);

            }
                    self.queue.submit(std::iter::once(
                    encoder.finish(),
        ));

        // Present frame
        output.present();
                Ok(())
            }

        }

    }

#[derive(Default)]
struct App {
    state: Option<State>,
}

impl ApplicationHandler for App {

    fn resumed(
        &mut self,
        event_loop: &ActiveEventLoop
    ) {
        let window = Arc::new(
            event_loop
                .create_window(
                    WindowAttributes::default()
                        .with_title("WGPU Tutorial"),
                )
                .unwrap(),
        );

        let mut state =
            pollster::block_on(State::new(window))
                .unwrap();

        let size = state.window.inner_size();

        state.resize(size.width, size.height);

        self.state = Some(state);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    )
    {
        // extracts state
        let state = match &mut self.state {
            Some(canvas) => canvas,
            None => return,
        };

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                state.resize(size.width, size.height);
            }

            WindowEvent::RedrawRequested => {
                state.update();

                if let Err(e) = state.render() {
                    eprintln!("{e}");
                    event_loop.exit();
                }
            }

            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key:
                            PhysicalKey::Code(code),

                        state: key_state,

                        ..
                    },

                ..
            } => {
                state.handle_key(
                    event_loop,
                    code,
                    key_state.is_pressed(),
                );
            }

            _ => {}
        }
    }
}
impl Vertex {
    const ATTRIBS: [wgpu::VertexAttribute; 2] =
        wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3];

    fn desc() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;

        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}
fn main() -> Result<()>
{
    env_logger::init();

    let event_loop = EventLoop::new()?;
    

    let mut app = App::default();

    event_loop.run_app(&mut app)?;

    Ok(())
}
