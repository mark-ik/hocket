//! Strophe's serval desktop host (serval-host refactor S0: scaffold).
//!
//! A winit window presenting a serval view tree: `ServalAppRunner` diffs the
//! views into a `ScriptedDom`, a retained `IncrementalLayout` lays it out, the
//! paint list lowers to a `netrender::Scene`, and `serval-winit-host`'s
//! `SurfaceHost` rasterizes and composites onto the backbuffer. This is the
//! same host skeleton woodshed-serval runs; S1+ grows the real view layer
//! (`strophe-views`) and theme over `AppState`. For S0 it renders a themed
//! placeholder with a clickable counter, proving the render + input + update
//! loop end to end.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use layout_dom_api::{DomMutation, LayoutDomMut as _};
use netrender::{ColorLoad, ExternalTexturePlacement, NetrenderOptions};
use paint_list_api::{DeviceIntSize, PaintList as _};
use serval_layout::{IncrementalLayout, ScrollOffsets};
use serval_scripted_dom::{NodeId, ScriptedDom};
use serval_winit_host::SurfaceHost;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};
use xilem_serval::{
    clickable, el, text, AnyView, PointerClick, Propagation, ServalAppRunner, ServalCtx,
    ServalElement,
};

/// Boxed heterogeneous child view over [`Ui`].
type Child = Box<dyn AnyView<Ui, (), ServalCtx, ServalElement>>;
type Runner = ServalAppRunner<Ui, fn(&Ui) -> Child, Child>;

/// S0 placeholder state. S1 replaces this with Strophe's `AppState`.
#[derive(Default)]
struct Ui {
    clicks: u32,
}

/// S0 placeholder view: a title, a subtitle, and one clickable counter that
/// exercises the full host loop (hit-test -> dispatch -> update -> repaint).
fn root(ui: &Ui) -> Child {
    Box::new(
        el(
            "div",
            (
                el("div", text("Strophe")).attr("class", "title"),
                el("div", text("serval host — S0 scaffold"))
                    .attr("class", "subtitle"),
                clickable(
                    el(
                        "div",
                        text(format!("clicked {} times — the host loop works", ui.clicks)),
                    )
                    .attr("class", "btn"),
                    |ui: &mut Ui, _| ui.clicks += 1,
                ),
            ),
        )
        .attr("class", "root"),
    )
}

/// Hardcoded dark placeholder sheet. S1 wires the real tinct-derived theme
/// (ported from `strophe-widgets::theme`).
fn placeholder_sheet() -> String {
    r#"
.root { width: 100%; height: 100%; box-sizing: border-box; background-color: #17151b;
        color: #d8d4dc; font-family: sans-serif; font-size: 14px; padding: 28px; }
.title { font-size: 30px; color: #f0ead6; margin-bottom: 6px; }
.subtitle { font-size: 14px; color: #8a8391; margin-bottom: 28px; }
.btn { display: inline-block; background-color: #2a2630; color: #d8d4dc;
       padding: 10px 18px; border-radius: 8px; }
.btn:hover { background-color: #3a3442; }
"#
    .to_string()
}

struct App {
    window: Option<Arc<Window>>,
    host: Option<SurfaceHost>,
    runner: Option<Runner>,
    /// Retained layout session (logical coords), hit-test target.
    layout: Option<IncrementalLayout<NodeId>>,
    layout_size: (f32, f32),
    sheet: String,
    /// Cursor in logical coordinates.
    cursor: (f32, f32),
}

impl App {
    fn redraw(&mut self) {
        let (Some(window), Some(host), Some(runner)) =
            (self.window.as_ref(), self.host.as_ref(), self.runner.as_ref())
        else {
            return;
        };
        let size = window.inner_size();
        let (pw, ph) = (size.width.max(1), size.height.max(1));
        let scale = window.scale_factor() as f32;
        let (lw, lh) = (pw as f32 / scale, ph as f32 / scale);

        let scene = {
            let dom = runner.dom();
            let mut muts: Vec<DomMutation<NodeId>> = Vec::new();
            dom.borrow_mut().drain_mutations(&mut muts);
            let dom_ref = dom.borrow();
            let sheets: Vec<&str> = vec![self.sheet.as_str()];
            let structural = muts
                .iter()
                .any(|m| !matches!(m, DomMutation::AttributeChanged { .. }));
            let size_changed = self.layout_size != (lw, lh);
            match self.layout.as_mut() {
                Some(layout) if !structural && !size_changed => {
                    if !muts.is_empty() {
                        let _ = layout.apply(&*dom_ref, &sheets, &muts);
                    }
                }
                _ => {
                    self.layout = Some(IncrementalLayout::new(&*dom_ref, &sheets, lw, lh));
                    self.layout_size = (lw, lh);
                }
            }
            let layout = self.layout.as_ref().expect("layout just ensured");
            let list = layout.emit_paint_list(
                &*dom_ref,
                &ScrollOffsets::default(),
                DeviceIntSize::new(lw as i32, lh as i32),
            );
            let translated = paint_list_render::translate_paint_cmd_stream(
                list.viewport(),
                list.commands(),
                list.fonts(),
                list.images(),
            );
            translated.scene
        };

        let (_tex, view) = host.core().rasterize_scaled(
            &scene,
            pw,
            ph,
            ColorLoad::Clear(wgpu::Color::BLACK),
            scale,
        );
        let Some(frame) = host.acquire() else { return };
        let target = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        host.renderer().compose_external_texture(
            &view,
            &target,
            host.format(),
            pw,
            ph,
            ExternalTexturePlacement::new([0.0, 0.0, pw as f32, ph as f32]),
        );
        frame.present();
    }

    fn click(&mut self) {
        let (Some(runner), Some(layout)) = (self.runner.as_mut(), self.layout.as_ref()) else {
            return;
        };
        let (x, y) = self.cursor;
        let hit = {
            let dom = runner.dom();
            let dom_ref = dom.borrow();
            layout.hit_test(&*dom_ref, x, y, &ScrollOffsets::default())
        };
        let Some(node) = hit else { return };
        runner.dispatch_click(
            node,
            PointerClick {
                local: (0.0, 0.0),
                prop: Propagation::new(),
            },
        );
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("Strophe")
                        .with_inner_size(winit::dpi::LogicalSize::new(1000.0, 640.0)),
                )
                .expect("create window"),
        );
        let size = window.inner_size();
        let host = SurfaceHost::boot(
            window.clone(),
            size.width.max(1),
            size.height.max(1),
            NetrenderOptions {
                tile_cache_size: Some(1024),
                enable_vello: true,
                ..Default::default()
            },
        )
        .expect("boot serval host");
        let dom = Rc::new(RefCell::new(ScriptedDom::new()));
        let runner = Runner::new(dom, root as fn(&Ui) -> Child, Ui::default());
        self.window = Some(window);
        self.host = Some(host);
        self.runner = Some(runner);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(host) = self.host.as_mut() {
                    host.resize(size.width.max(1), size.height.max(1));
                }
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let scale = self.window.as_ref().map_or(1.0, |w| w.scale_factor());
                self.cursor = ((position.x / scale) as f32, (position.y / scale) as f32);
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => self.click(),
            WindowEvent::RedrawRequested => self.redraw(),
            _ => {}
        }
    }
}

fn main() {
    let event_loop = EventLoop::new().expect("event loop");
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App {
        window: None,
        host: None,
        runner: None,
        layout: None,
        layout_size: (0.0, 0.0),
        sheet: placeholder_sheet(),
        cursor: (0.0, 0.0),
    };
    event_loop.run_app(&mut app).expect("run app");
}
