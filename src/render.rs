use wgpu::*;
use wgpu::util::*;
use bytemuck::{ Pod, Zeroable, cast_slice };
use raw_window_handle::{ HasRawWindowHandle, HasRawDisplayHandle };
use num_bigfloat::BigFloat;
use std::mem::size_of;



macro_rules! default_render_pipeline_descriptor {
    ($format:expr, $shader:expr, $layout:expr) => {
        RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: $layout,
            vertex: VertexState {
                module: $shader,
                entry_point: "vs_main",
                buffers: &[Vertex::LAYOUT],
            },
            fragment: Some(FragmentState {
                module: $shader,
                entry_point: "fs_main",
                targets: &[Some(ColorTargetState {
                    format: $format,
                    blend: Some(BlendState {
                        color: BlendComponent {
                            src_factor: BlendFactor::SrcAlpha,
                            dst_factor: BlendFactor::OneMinusSrcAlpha,
                            operation: BlendOperation::Add,
                        },
                        alpha: BlendComponent {
                            src_factor: BlendFactor::One,
                            dst_factor: BlendFactor::OneMinusSrcAlpha,
                            operation: BlendOperation::Add,
                        },
                    }),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        }
    };
}

/// 使用此宏以从一个`RenderContext`中创建一个Load模式的`RenderPass`
/// 使用此宏创建
macro_rules! load_render_pass_from_render_context {
    ($ctx:expr) => {{
        let mut render_pass = $ctx.encoder.as_mut().unwrap().begin_render_pass(
            &RenderPassDescriptor {
                label: Some("Render pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: $ctx.view,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Load,
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            }
        );

        render_pass.set_bind_group(0, &$ctx.renderer.basic_bind_group, &[]);

        render_pass
    }}
}



pub trait Drawable {
    fn draw(&self, ctx: RenderContext<'_>);
}



pub struct RenderContext<'a> {
    pub view: &'a TextureView,
    pub renderer: &'a Renderer,
    pub encoder: Option<CommandEncoder>,
}

#[repr(C)]
#[derive(Pod, Zeroable, Clone, Copy, Debug)]
pub struct BasicUniform {
    pub aspect_ratio: f32,
    pub scale: f32,
    pub _padding1: [f32; 2],
    pub camera_coord: [f32; 3],
    pub _padding2: [f32; 1],
}

pub struct Renderer {
    pub debug: bool,
    pub surface: Surface,
    pub device: Device,
    pub queue: Queue,
    pub config: SurfaceConfiguration,
    pub pipeline: RenderPipeline,
    pub shader: ShaderModule,
    pub circle_shader: ShaderModule,
    pub basic_bind_group: BindGroup,
    pub basic_bind_group_layout: BindGroupLayout,
    pub basic_bind_group_buffer: Buffer,
    pub basic_bind_group_data: BasicUniform,
    pub size: (u32, u32),
    pub timewrap: f64,
    pub scale: BigFloat,
    pub scale_base: BigFloat,
}

impl Renderer {
    pub async fn new<W>(win: &W, size: (u32, u32))-> Renderer
        where W: HasRawWindowHandle + HasRawDisplayHandle
    {
        // 此处的Instance是一个GPU实例
        let instance = Instance::new( InstanceDescriptor {
            backends: Backends::all(),
            ..Default::default()
        });

        let surface = unsafe {
            instance.create_surface(win)
                .expect("Failed to create sirface")
        };

        let adapter = instance.request_adapter(&RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }).await.unwrap();

        let (device, queue) = adapter.request_device(&DeviceDescriptor {
            label: None,
            limits: Limits::default(),
            features: Features::empty(),
        }, None).await.unwrap();

        let shader = device.create_shader_module(include_wgsl!("generic.wgsl"));
        let circle_shader = device.create_shader_module(include_wgsl!("circle.wgsl"));

        let caps = surface.get_capabilities(&adapter);
        let config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: caps.formats[0],
            width: size.0,
            height: size.1,
            present_mode: PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
        };

        let basic_bind_group_layout = device.create_bind_group_layout(
            &BindGroupLayoutDescriptor {
                label: Some("Basic bind group layout"),
                entries: &[
                    BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                        count: None,
                        ty: BindingType::Buffer {
                            min_binding_size: None,
                            has_dynamic_offset: false,
                            ty: BufferBindingType::Uniform,
                        },
                    },
                ],
            }
        );

        let basic_bind_group_buffer = device.create_buffer_init(
            &BufferInitDescriptor {
                label: Some("Basic bind group buffer"),
                contents: cast_slice(&[ BasicUniform {
                    aspect_ratio: size.0 as f32 / size.1 as f32,
                    scale: 1.0,
                    camera_coord: [0.0, 0.0, 0.0],
                    _padding1: [0.0, 0.0],
                    _padding2: [0.0],
                }]),
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            }
        );

        let basic_bind_group = device.create_bind_group(
            &BindGroupDescriptor {
                label: Some("Basic bind group"),
                layout: &basic_bind_group_layout,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: BindingResource::Buffer(BufferBinding {
                            buffer: &basic_bind_group_buffer,
                            offset: 0,
                            size: None,
                        }),
                    }
                ],
            }
        );

        let pipeline_layout = device.create_pipeline_layout(
            &PipelineLayoutDescriptor {
                label: Some("Pipeline layout"),
                bind_group_layouts: &[
                    &basic_bind_group_layout,
                ],
                push_constant_ranges: &[],
            }
        ); 

        let pipeline = device.create_render_pipeline(&default_render_pipeline_descriptor!(config.format, &shader, Some(&pipeline_layout)));

        surface.configure(&device, &config);

        Renderer {
            surface,
            device,
            queue,
            config,
            pipeline,
            size,
            shader,
            circle_shader,
            basic_bind_group,
            basic_bind_group_layout,
            basic_bind_group_buffer,
            basic_bind_group_data: BasicUniform {
                aspect_ratio: size.0 as f32 / size.1 as f32,
                scale: 1.0,
                camera_coord: [0.0, 0.0, 0.0],
                _padding1: [0.0, 0.0],
                _padding2: [0.0],
            },
            scale: "1.0".parse().unwrap(),
            scale_base: "4.0e8".parse().unwrap(),
            timewrap: 1.0,
            debug: false,
        }
    }

    /// 用`basic_bind_group_data`更新整个`BasicUniform`
    fn update_buffer(&self) {
        if self.debug {
            self.print_msg();
        }

        self.queue.write_buffer(
            &self.basic_bind_group_buffer,
            0,
            cast_slice(&[self.basic_bind_group_data]),
        );
    }

    /// 修改surface的大小，并同步`BasicUniform`
    pub fn resize(&mut self, new_size: (u32, u32)) {
        if new_size.0 > 0 && new_size.1 > 0 {
            self.size = new_size;
            self.config.width = new_size.0;
            self.config.height = new_size.1;
            self.surface.configure(&self.device, &self.config);

            self.basic_bind_group_data.aspect_ratio = new_size.0 as f32 / new_size.1 as f32;
            self.update_buffer();
        }
    }

    pub fn scale_from_array3(&self, s: [BigFloat; 3])-> [f32; 3] {
        let scale = self.scale_base / self.scale;
        [(s[0] / scale).to_f32(), (s[1] / scale).to_f32(), (s[2] / scale).to_f32()]
    }

    pub fn scale_from_point(&self, p: crate::physics::Point)-> [f32; 3] {
        self.scale_from_array3([p.x, p.y, p.z])
    }

    /// 缩放视图
    pub fn scale(&mut self, scale: BigFloat) {
        self.scale = scale;
        self.basic_bind_group_data.scale = scale.to_f32();
        self.update_buffer();
    }

    /// 移动相机到指定坐标
    pub fn move_camera(&mut self, new_coord: [f32; 3]) {
        self.basic_bind_group_data.camera_coord = new_coord;
        self.update_buffer();
    }

    pub fn print_msg(&self) {
        let data = &self.basic_bind_group_data;
        let cam = &data.camera_coord;
        print!("\x1bc");
        println!("Camera: ({},{},{})", cam[0], cam[1], cam[2]);
        println!("Scale:  {}", data.scale);
        println!("Timewrap ratio: {}", self.timewrap);
    }
}



#[repr(C)]
#[derive(Debug, Pod, Zeroable, Clone, Copy)]
pub struct Vertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
}

impl Vertex {
    pub const LAYOUT: VertexBufferLayout<'_> = VertexBufferLayout {
        array_stride: size_of::<Self>() as u64,
        step_mode: VertexStepMode::Vertex,
        attributes: &[
            VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: VertexFormat::Float32x3,
            },

            VertexAttribute {
                offset: size_of::<[f32; 3]>() as u64,
                shader_location: 1,
                format: VertexFormat::Float32x4,
            },
        ],
    };
}

impl Drop for RenderContext<'_> {
    fn drop(&mut self) {
        self.renderer.queue.submit(std::iter::once(self.encoder.take().unwrap().finish()));
    }
}



/// 绘制一个矩形
pub struct Rectangle {
    pub vertices: [Vertex; 4],
}

impl Rectangle {
    pub const INDICES: [u16; 6] = [0, 1, 2, 2, 3, 0];
    
    pub fn new(p1: Vertex, p2: Vertex, p3: Vertex, p4: Vertex)-> Self {
        Self {
            vertices: [p1, p2, p3, p4],
        }
    }
}

impl Drawable for Rectangle {
    fn draw(&self, mut ctx: RenderContext) {
        let vertices = ctx.renderer.device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Rectangle vertex buffer"),
            contents: cast_slice(&self.vertices),
            usage: BufferUsages::VERTEX,
        });

        let indices = ctx.renderer.device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Rectangle index buffer"),
            contents: cast_slice(&Self::INDICES),
            usage: BufferUsages::INDEX,
        });

        let mut render_pass = load_render_pass_from_render_context!(ctx);

        render_pass.set_pipeline(&ctx.renderer.pipeline);
        render_pass.set_vertex_buffer(0, vertices.slice(..));
        render_pass.set_index_buffer(indices.slice(..), IndexFormat::Uint16);
        render_pass.draw_indexed(0..6, 0, 0..1);

        drop(render_pass);
        drop(ctx);
    }
}

/// 绘制一个圆形
/// 顶点着色器默认，片段着色器使用`circle_fs`
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct Circle {
    pub center: [f32; 3],
    pub radius: f32,
    pub fill_color: [f32; 4],
}

impl Drawable for Circle {
    fn draw(&self, mut ctx: RenderContext<'_>) {
        let r = self.radius;
        let mut points = [self.center, self.center, self.center, self.center];

        points[0][0] -= r;
        points[0][1] += r;
        points[1][0] -= r;
        points[1][1] -= r;
        points[2][0] += r;
        points[2][1] -= r;
        points[3][0] += r;
        points[3][1] += r;

        let mut vertices_vec = Vec::new();
        points.into_iter()
            .for_each(|i| vertices_vec.push(Vertex {
                position: i,
                color: self.fill_color.clone()
        }));

        let vertices = ctx.renderer.device.create_buffer_init(
            &BufferInitDescriptor {
                label: Some("Circle vertex buffer"),
                contents: cast_slice(vertices_vec.as_slice()),
                usage: BufferUsages::VERTEX,
            }
        );

        let indices = ctx.renderer.device.create_buffer_init(
            &BufferInitDescriptor {
                label: Some("Circle index buffer"),
                contents: cast_slice(&Rectangle::INDICES),
                usage: BufferUsages::INDEX,
            }
        );

        let circle_bind_group_layout = ctx.renderer.device.create_bind_group_layout(
            &BindGroupLayoutDescriptor {
                label: Some("Circle bind group layout"),
                entries: &[
                    BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ShaderStages::FRAGMENT,
                        count: None,
                        ty: BindingType::Buffer {
                            min_binding_size: None,
                            has_dynamic_offset: false,
                            ty: BufferBindingType::Uniform,
                        },
                    },
                ],
            }
        );

        let circle_data = [self.center[0], self.center[1], self.center[2], self.radius];
        let circle_bind_group_buffer = ctx.renderer.device.create_buffer_init(
            &BufferInitDescriptor {
                label: Some("Circle bind group buffer"),
                contents: cast_slice(&circle_data),
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            }
        );

        let circle_bind_group = ctx.renderer.device.create_bind_group(
            &BindGroupDescriptor {
                label: Some("Circle bind group"),
                layout: &circle_bind_group_layout,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: BindingResource::Buffer(BufferBinding {
                            buffer: &circle_bind_group_buffer,
                            offset: 0,
                            size: None,
                        }),
                    },
                ],
            }
        );

        let circle_pipeline_layout = ctx.renderer.device.create_pipeline_layout(
            &PipelineLayoutDescriptor {
                label: Some("Circle pipeline layout"),
                bind_group_layouts: &[
                    &ctx.renderer.basic_bind_group_layout,
                    &circle_bind_group_layout,
                ],
                push_constant_ranges: &[],
            }
        );

        let circle_pipeline = ctx.renderer.device.create_render_pipeline(
            &RenderPipelineDescriptor {
                label: Some("Circle render pipeline"),
                layout: Some(&circle_pipeline_layout),
                vertex: VertexState {
                    module: &ctx.renderer.shader,
                    entry_point: "vs_main",
                    buffers: &[Vertex::LAYOUT],
                },
                fragment: Some(FragmentState {
                    module: &ctx.renderer.circle_shader,
                    entry_point: "circle_fs",
                    targets: &[Some(ColorTargetState {
                        format: ctx.renderer.config.format,
                        blend: Some(BlendState {
                            color: BlendComponent {
                                src_factor: BlendFactor::SrcAlpha,
                                dst_factor: BlendFactor::OneMinusSrcAlpha,
                                operation: BlendOperation::Add,
                            },
                            alpha: BlendComponent {
                                src_factor: BlendFactor::One,
                                dst_factor: BlendFactor::OneMinusSrcAlpha,
                                operation: BlendOperation::Add,
                            },
                        }),
                        write_mask: ColorWrites::ALL,
                    })],
                }),
                primitive: PrimitiveState {
                    topology: PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: FrontFace::Ccw,
                    cull_mode: None,
                    unclipped_depth: false,
                    polygon_mode: PolygonMode::Fill,
                    conservative: false,
                },
                multisample: MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                depth_stencil: None,
                multiview: None,
            }
        );

        let mut render_pass = load_render_pass_from_render_context!(ctx);

        render_pass.set_pipeline(&circle_pipeline);
        render_pass.set_vertex_buffer(0, vertices.slice(..));
        render_pass.set_index_buffer(indices.slice(..), IndexFormat::Uint16);
        render_pass.set_bind_group(1, &circle_bind_group, &[]);
        render_pass.draw_indexed(0..6, 0, 0..1);
    }
}
