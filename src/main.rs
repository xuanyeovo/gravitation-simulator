mod render;
mod physics;

use crate::physics::*;
use crate::render::*;
use anyhow::Result;
use winit::{
    window::{ Window, WindowBuilder },
    event_loop::{ EventLoop, ControlFlow },
    event::{ WindowEvent, Event },
    dpi::PhysicalPosition,
};
use pollster::FutureExt;
use uuid::Uuid;
use wgpu::*;
use num_bigfloat::{ BigFloat, ZERO };
use std::sync::{ Arc, Mutex, atomic::{ AtomicBool, Ordering::* } };
use std::time::{ Instant, Duration };

type Context = WinitContext;

trait World {
    /// 返回可绘制的所有物体
    fn get_drawable_items<'items, 'this: 'items>(&'this self)-> Vec<&'items dyn Drawable>;

    /// 执行物理计算
    fn execute(&mut self, time: Duration);

    /// 获取默认显示比例的底
    fn get_default_scale_base(&self)-> BigFloat {
        "4.0e8".parse().unwrap()
    }
}



struct WinitContext {
    pub event_loop: EventLoop<()>,
    pub window: Window,
}

struct Earth {
    uid: Uuid,
    phyattr: PhysicalAttributes,
}

struct Moon {
    uid: Uuid,
    phyattr: PhysicalAttributes,
}

struct Application {
    renderer: Renderer,
    ctx: Context,
}

struct EarthMoonWorld {
    executor: SpaceExecutor,
    earth: Earth,
    moon: Moon,
}



impl WinitContext {
    pub fn new()-> Result<Self> {
        let event_loop = EventLoop::new();
        let window = WindowBuilder::new()
            .with_title("Gravitation Simulator")
            .with_visible(false)
            .build(&event_loop)?;

        #[cfg(not(any(target_os = "android", target_arch = "wasm")))]
        {
            let monitor = window.current_monitor().unwrap();
            let mut size = monitor.size();
            size.width /= 2;
            size.height /= 2;
            window.set_inner_size(size);
        }

        window.set_visible(true);

        Ok(Self {
            event_loop,
            window,
        })
    }
}

impl Application {
    pub async fn new()-> Self {
        let ctx = WinitContext::new().expect("Unable to build a window");
        let wsize = ctx.window.inner_size();
        Self {
            renderer: Renderer::new(&ctx.window, (wsize.width, wsize.height)).await,
            ctx,
        }
    }

    pub async fn run(mut self) {
        const FRAME_TIME: Duration = Duration::from_micros(33333);

        let world_factory = || {
            EarthMoonWorld::default()
        };

        let world = Arc::new(Mutex::new(world_factory()));
        let run_flag = Arc::new(AtomicBool::new(true));
        let timewrap = Arc::new(Mutex::new(1.0f64));
        let mut y_accumulate = 0.0;
        let mut last_pos = PhysicalPosition::<f64> {
            x: 0.0,
            y: 0.0,
        };
        let mut drag = None::<(PhysicalPosition<f64>, [f32; 3])>;

        self.renderer.scale_base = world.lock().unwrap().get_default_scale_base();
        self.renderer.debug = true;

        std::thread::Builder::new()
            .name("Physics Executor".to_owned())
            .spawn({
                let world = Arc::clone(&world);
                let run_flag = Arc::clone(&run_flag);
                let timewrap = Arc::clone(&timewrap);
                move || {
                    while run_flag.load(Acquire) {
                        let t1 = Instant::now();

                        world.lock().unwrap().execute(
                            Duration::from_millis(
                                (30.0 * (*timewrap.lock().unwrap())) as u64
                            )
                        );

                        let t = t1.elapsed();

                        if t < FRAME_TIME {
                            //std::thread::sleep(FRAME_TIME - t);
                        }
                    }
                }
            }).unwrap();

        self.ctx.event_loop.run(move |event, _target, control_flow| {
            match event {
                // 重新绘制窗口
                Event::RedrawRequested(id)
                    if id == self.ctx.window.id()
                => {
                    match self.renderer.surface.get_current_texture() {
                        Ok(surface_texture) => {
                            let view = surface_texture.texture.create_view(&TextureViewDescriptor::default());
                            let mut encoder = self.renderer.device.create_command_encoder(&Default::default());
                            let _render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                                label: Some("Earth render pass"),
                                color_attachments: &[Some(RenderPassColorAttachment {
                                    view: &view,
                                    resolve_target: None,
                                    ops: Operations {
                                        load: LoadOp::Clear(Color {
                                            r: 0.05,
                                            g: 0.05,
                                            b: 0.05,
                                            a: 1.00,
                                        }),
                                        store: true,
                                    },
                                })],
                                depth_stencil_attachment: None,
                            });

                            drop(_render_pass);
                            self.renderer.queue.submit(std::iter::once(encoder.finish()));

                            world
                                .lock()
                                .unwrap()
                                .get_drawable_items()
                                .into_iter()
                                .for_each(|i| {
                                    i.draw(RenderContext {
                                        view: &view,
                                        renderer: &self.renderer,
                                        encoder: Some(self.renderer.device.create_command_encoder(&CommandEncoderDescriptor::default())),
                                    });
                                });

                            surface_texture.present();
                        },

                        Err(SurfaceError::Lost) => {
                            self.renderer.resize(self.renderer.size);
                        },

                        Err(SurfaceError::OutOfMemory) => {
                            *control_flow = ControlFlow::Exit;
                        },

                        Err(_) => (),
                    }
                },

                Event::WindowEvent {
                    window_id: id,
                    event: window_event,
                } if id == self.ctx.window.id() => {
                    use winit::event::{
                        MouseScrollDelta,
                        MouseButton,
                        ElementState,
                        VirtualKeyCode,
                        KeyboardInput,
                    };
                    match window_event {
                        WindowEvent::Resized(size) => {
                            self.renderer.resize((size.width, size.height));
                        },

                        WindowEvent::ScaleFactorChanged {
                            new_inner_size: size,
                            ..
                        } => {
                            self.renderer.resize((size.width, size.height));
                        },

                        WindowEvent::CloseRequested | WindowEvent::Destroyed => {
                            *control_flow = ControlFlow::Exit;
                        },

                        // 鼠标滚轮调整缩放比例
                        WindowEvent::MouseWheel { delta, .. } => {
                            match delta {
                                MouseScrollDelta::LineDelta(_, y) => {
                                    y_accumulate += (y as f64) / 0.01;
                                },

                                MouseScrollDelta::PixelDelta(pos) => {
                                    y_accumulate += pos.y;
                                },
                            }

                            loop {
                                if y_accumulate >= 1. {
                                    y_accumulate -= 1.;
                                    self.renderer.scale(self.renderer.scale * BigFloat::from(1.01010101f64));
                                } else if y_accumulate <= -1. {
                                    y_accumulate += 1.;
                                    self.renderer.scale(self.renderer.scale * BigFloat::from(0.98f64));
                                } else {
                                    break;
                                }
                            }
                        },

                        // 
                        WindowEvent::MouseInput { state, button, .. }
                            if button == MouseButton::Left
                        => {
                            match state {
                                ElementState::Pressed => drag = Some((last_pos, self.renderer.basic_bind_group_data.camera_coord)),
                                ElementState::Released => drag = None,
                            }
                        },

                        WindowEvent::CursorLeft {..} => {
                            drag = None;
                        },

                        WindowEvent::CursorMoved { position, .. } => {
                            // 这里检测按下拖动时调整视角
                            if let Some((drag, cc)) = drag.as_ref() {
                                let x = ((position.x - drag.x) / 100.0 / self.renderer.scale.to_f64()) as f32;
                                let y = ((position.y - drag.y) / 100.0 / self.renderer.scale.to_f64()) as f32;
                                self.renderer.move_camera([cc[0] + x, cc[1] + y, 0.0]);
                            }
                            last_pos = position;
                        },

                        WindowEvent::Touch(touch) => {
                            // TODO 添加对触摸的支持
                            //dbg!(touch);
                        },

                        WindowEvent::KeyboardInput {
                            input: KeyboardInput {
                                state,
                                virtual_keycode,
                                ..
                            },
                            ..
                        } if state == ElementState::Pressed => {
                            if let Some(k) = virtual_keycode {
                                match k {
                                    // 按下上键提高时间流逝速度(每次乘2)
                                    VirtualKeyCode::Up => {
                                        let mut tw = timewrap.lock().unwrap();
                                        *tw *= 2.0;
                                        self.renderer.timewrap = *tw;
                                        self.renderer.print_msg();
                                    },

                                    // 按下下键降低时间流逝速度(每次除以2)
                                    VirtualKeyCode::Down => {
                                        let mut tw = timewrap.lock().unwrap();
                                        *tw /= 2.0;
                                        self.renderer.timewrap = *tw;
                                        self.renderer.print_msg();
                                    },

                                    // 按下R重置世界
                                    VirtualKeyCode::R => {
                                        let mut world_ref = world.lock().unwrap();
                                        *world_ref = world_factory();
                                        self.renderer.scale(BigFloat::from(1.0));
                                        self.renderer.move_camera([0.0, 0.0, 0.0]);
                                        self.renderer.scale_base = world_ref.get_default_scale_base();
                                    },

                                    _ => {},
                                }
                            }
                        },

                        _ => {},
                    }
                },

                Event::MainEventsCleared => {
                    self.ctx.window.request_redraw();
                },

                _ => {},
            }

            if *control_flow == ControlFlow::Exit {
                run_flag.store(false, Release);
            }
        });
    }
}

impl Earth {
    pub fn new(center: Point, velocity: Vector)-> Self {
        let uid = Uuid::new_v4();
        Self {
            phyattr: PhysicalAttributes {
                center,
                velocity,
                force: Vector::ZERO,
                mass: "5.965e24".parse().unwrap(),
            },
            uid,
        }
    }
}

impl PhysicalObject for Earth {
    fn get_uid(&self)-> Uuid {
        self.uid
    }

    fn get_physical_attributes(&self)-> &PhysicalAttributes {
        &self.phyattr
    }

    fn get_physical_attributes_mut(&mut self)-> &mut PhysicalAttributes {
        &mut self.phyattr
    }
}

impl Drawable for Earth {
    fn draw(&self, ctx: RenderContext) {
        Circle {
            center: ctx.renderer.scale_from_point(self.phyattr.center.clone()),
            radius: 0.2 * ctx.renderer.scale.to_f32(),
            fill_color: [0.1, 0.1, 0.95, 1.0],
        }.draw(ctx)
    }
}

impl Moon {
    pub fn new(center: Point, velocity: Vector)-> Self {
        let uid = Uuid::new_v4();
        Self {
            phyattr: PhysicalAttributes {
                center,
                velocity,
                force: Vector::ZERO,
                mass: "7.35e22".parse().unwrap(),
            },
            uid,
        }
    }
}

impl PhysicalObject for Moon {
    fn get_uid(&self)-> Uuid {
        self.uid
    }

    fn get_physical_attributes(&self)-> &PhysicalAttributes {
        &self.phyattr
    }

    fn get_physical_attributes_mut(&mut self)-> &mut PhysicalAttributes {
        &mut self.phyattr
    }
}

impl Drawable for Moon {
    fn draw(&self, ctx: RenderContext) {
        Circle {
            center: ctx.renderer.scale_from_point(self.phyattr.center.clone()),
            radius: 0.12 * ctx.renderer.scale.to_f32(),
            fill_color: [0.25, 0.25, 0.25, 1.0],
        }.draw(ctx)
    }
}

impl Default for EarthMoonWorld {
    fn default()-> Self {
        Self {
            executor: SpaceExecutor::default(),

            earth: Earth::new(
                Point { x: ZERO, y: ZERO, z: ZERO },

                Vector::ZERO
            ),

            // 月球以近地点为起点
            moon: Moon::new(
                Point {
                    x: ZERO,
                    y: "3.57e8".parse().unwrap(),
                    z: ZERO
                },

                Vector {
                    x: BigFloat::from(1022),
                    y: ZERO,
                    z: ZERO,
                }
            ),
        }
    }
}

impl World for EarthMoonWorld {
    fn get_drawable_items<'items, 'this: 'items>(&'this self)-> Vec<&'items dyn Drawable> {
        vec![&self.earth, &self.moon]
    }

    fn execute(&mut self, time: Duration) {
        let mut objects = Objects::new(vec![&mut self.earth, &mut self.moon]);
        self.executor.execute_force(&mut objects, time);
        self.executor.execute_displacement(&mut objects, time);

        drop(objects);
    }

    fn get_default_scale_base(&self)-> BigFloat {
        "3.80e8".parse().unwrap()
    }
}



fn main() {
    env_logger::init();

    let app = Application::new().block_on();

    app.run().block_on();
}

#[cfg(target_arch = "wasm32")]
#[cfg_attr(target_arch="wasm32", wasm_bindgen::prelude::wasm_bindgen(start))]
async fn wasm_main() {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(log::Level::Info);

    let app = Application::new().await;

    app.run().await;
}
