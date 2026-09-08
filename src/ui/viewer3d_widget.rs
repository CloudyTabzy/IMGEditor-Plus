//! Iced integration for the embedded 3D viewer.
//!
//! Shares state with the surrounding [`App`](crate::ui::app::App) through
//! an [`Arc<SceneHandle>`] — the handle is mutated by message handling
//! in `app.rs`, read inside this widget's `update`/`draw`/`prepare`/
//! `render`.
//!
//! Three pieces implement three trait hierarchies:
//!
//! - [`Scene3dWidget`] — a custom `iced::advanced::Widget` that lays
//!   out inside the right-hand pane, handles pointer events for
//!   orbit/pan/zoom, and forwards the bounds to the renderer.
//! - [`ScenePrimitive`] — an `iced_wgpu::primitive::Primitive` whose
//!   `render` method issues the actual one-pass-with-depth draw on
//!   every frame.
//! - [`ScenePipeline`] — the per-device `iced_wgpu::primitive::Pipeline`,
//!   built once and shared across frames. Holds the WGSL pipelines +
//!   per-frame depth texture + mesh cache.
//!
//! The widget keeps its own drag state inside the `widget::Tree`
//! state so continuous mouse drags work correctly across frames.

use std::sync::{Arc, Mutex};

use iced::advanced::widget::tree::Tag;
use iced::advanced::widget::{Tree, tree};
use iced::{
    Element, Event, Length, Point, Rectangle, Size,
    advanced::Clipboard,
    advanced::Shell,
    advanced::Widget,
    advanced::graphics,
    advanced::layout::{self, Limits, Node},
    advanced::renderer,
    mouse::{self, Button as MouseButton, Cursor, Event as MouseEvent, ScrollDelta},
};

use iced_widget::renderer::wgpu::primitive::{self, Pipeline as PrimitivePipeline};

use crate::inspector::scene3d::camera::OrbitCamera;
use crate::inspector::scene3d::mesh::Aabb;
use crate::inspector::scene3d::pipeline::{
    GpuMesh, GpuTexture, RenderFlags, ScenePipelines, create_depth_texture, effective_texture_flag,
    register_gpu_error_handlers, validate_scene_for_device,
};
use crate::inspector::scene3d::scene::Scene;

const ORBIT_SENSITIVITY: f32 = 0.010;
const PAN_SENSITIVITY: f32 = 0.001;
const WHEEL_ZOOM_PER_LINE: f32 = 0.065;
const WHEEL_ZOOM_PER_PIXEL: f32 = WHEEL_ZOOM_PER_LINE / 40.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SceneOriginMode {
    #[default]
    Centered,
    World,
}

fn scene_display_offset(scene: &Scene, mode: SceneOriginMode) -> [f32; 3] {
    if mode == SceneOriginMode::World {
        return [0.0; 3];
    }
    let center = scene.aabb.center();
    if center.iter().all(|value| value.is_finite()) {
        [-center[0], -center[1], -center[2]]
    } else {
        [0.0; 3]
    }
}

fn translated_aabb(aabb: Aabb, offset: [f32; 3]) -> Aabb {
    Aabb {
        min: [
            aabb.min[0] + offset[0],
            aabb.min[1] + offset[1],
            aabb.min[2] + offset[2],
        ],
        max: [
            aabb.max[0] + offset[0],
            aabb.max[1] + offset[1],
            aabb.max[2] + offset[2],
        ],
    }
}

fn resource_cache_flags(flags: RenderFlags) -> u32 {
    flags.intersection(RenderFlags::HAS_TEXTURE).bits()
}

#[derive(Debug, Default)]
pub struct SceneHandle {
    inner: Mutex<SceneHandleInner>,
}

#[derive(Debug)]
pub struct SceneHandleInner {
    pub scene: Option<Arc<Scene>>,
    pub camera: OrbitCamera,
    pub flags: RenderFlags,
    pub(crate) origin_mode: SceneOriginMode,
    pub dirty: bool,
    pub gpu_error: Option<String>,
}

impl Default for SceneHandleInner {
    fn default() -> Self {
        Self {
            scene: None,
            camera: OrbitCamera::default(),
            // The reference floor is part of the established viewer
            // presentation; the toolbar lets users hide it per session.
            flags: RenderFlags::SHOW_GRID,
            origin_mode: SceneOriginMode::default(),
            dirty: false,
            gpu_error: None,
        }
    }
}

impl SceneHandle {
    pub fn new() -> Self {
        Self::default()
    }

    /// Install a scene. Accepts an `Arc<Scene>` so callers that hold a
    /// decoded scene in a cache can share it with the widget without a deep
    /// clone.
    pub fn set_scene(&self, scene: std::sync::Arc<Scene>) {
        let mut inner = self.inner.lock().expect("scene handle mutex");
        let offset = scene_display_offset(&scene, inner.origin_mode);
        inner
            .camera
            .reset_to_aabb(&translated_aabb(scene.aabb, offset));
        inner.scene = Some(scene);
        inner.gpu_error = None;
        inner.dirty = true;
    }

    pub fn clear(&self) {
        let mut inner = self.inner.lock().expect("scene handle mutex");
        inner.scene = None;
        inner.gpu_error = None;
        inner.dirty = true;
    }

    /// Reset the orbit camera to its default pose (fit the scene's AABB).
    /// The scene itself is preserved.
    pub fn reset_camera(&self) {
        let mut inner = self.inner.lock().expect("scene handle mutex");
        let display_aabb = inner.scene.as_ref().map(|scene| {
            translated_aabb(scene.aabb, scene_display_offset(scene, inner.origin_mode))
        });
        if let Some(aabb) = display_aabb {
            inner.camera.reset_to_aabb(&aabb);
        } else {
            inner.camera = OrbitCamera::default();
        }
        inner.dirty = true;
    }

    pub fn toggle_center_origin(&self) {
        let mut inner = self.inner.lock().expect("scene handle mutex");
        inner.origin_mode = match inner.origin_mode {
            SceneOriginMode::Centered => SceneOriginMode::World,
            SceneOriginMode::World => SceneOriginMode::Centered,
        };
        let display_aabb = inner.scene.as_ref().map(|scene| {
            translated_aabb(scene.aabb, scene_display_offset(scene, inner.origin_mode))
        });
        if let Some(aabb) = display_aabb {
            inner.camera.reset_to_aabb(&aabb);
        }
        inner.dirty = true;
    }

    pub fn toggle_wireframe(&self) {
        let mut inner = self.inner.lock().expect("scene handle mutex");
        inner.flags ^= crate::inspector::scene3d::pipeline::RenderFlags::WIREFRAME;
        inner.dirty = true;
    }

    pub fn toggle_grid(&self) {
        let mut inner = self.inner.lock().expect("scene handle mutex");
        inner.flags ^= crate::inspector::scene3d::pipeline::RenderFlags::SHOW_GRID;
        inner.dirty = true;
    }

    pub fn toggle_cull_back(&self) {
        let mut inner = self.inner.lock().expect("scene handle mutex");
        inner.flags ^= crate::inspector::scene3d::pipeline::RenderFlags::CULL_BACK;
        inner.dirty = true;
    }

    pub fn toggle_textured(&self) {
        let mut inner = self.inner.lock().expect("scene handle mutex");
        inner.flags ^= crate::inspector::scene3d::pipeline::RenderFlags::HAS_TEXTURE;
        inner.dirty = true;
    }

    pub fn with<R>(&self, f: impl FnOnce(&SceneHandleInner) -> R) -> R {
        let inner = self.inner.lock().expect("scene handle mutex");
        f(&inner)
    }

    pub(crate) fn with_mut<R>(&self, f: impl FnOnce(&mut SceneHandleInner) -> R) -> R {
        let mut inner = self.inner.lock().expect("scene handle mutex");
        f(&mut inner)
    }

    pub(crate) fn set_gpu_error(&self, message: String) {
        self.with_mut(|inner| inner.gpu_error = Some(message));
    }

    pub(crate) fn clear_gpu_error(&self) {
        self.with_mut(|inner| inner.gpu_error = None);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DragMode {
    Orbit,
    Pan,
}

impl DragMode {
    fn from_button(button: MouseButton) -> Option<Self> {
        match button {
            MouseButton::Left => Some(Self::Orbit),
            MouseButton::Middle => Some(Self::Pan),
            _ => None,
        }
    }
}

#[derive(Default)]
struct DragState {
    mode: Option<DragMode>,
    last: Option<Point>,
    cursor_inside: bool,
}

impl DragState {
    fn begin(&mut self, button: MouseButton) -> bool {
        let Some(mode) = DragMode::from_button(button) else {
            return false;
        };
        self.mode = Some(mode);
        self.last = None;
        true
    }

    fn end(&mut self, button: MouseButton) -> bool {
        let Some(mode) = DragMode::from_button(button) else {
            return false;
        };
        if self.mode == Some(mode) {
            self.mode = None;
            self.last = None;
            true
        } else {
            false
        }
    }

    fn is_dragging(&self) -> bool {
        self.mode.is_some()
    }
}

pub struct Scene3dWidget {
    handle: Arc<SceneHandle>,
    width: Length,
    height: Length,
}

impl Scene3dWidget {
    pub fn new(handle: Arc<SceneHandle>) -> Self {
        Self {
            handle,
            width: Length::Fill,
            height: Length::Fill,
        }
    }

    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }
}

fn handle_event(
    camera: &mut OrbitCamera,
    state: &mut DragState,
    event: &Event,
    cursor_inside: bool,
) -> bool {
    match event {
        Event::Mouse(MouseEvent::ButtonPressed(button)) if cursor_inside => state.begin(*button),
        Event::Mouse(MouseEvent::ButtonReleased(button)) => state.end(*button),
        Event::Mouse(MouseEvent::CursorMoved { position }) => {
            if let Some(mode) = state.mode {
                if let Some(last) = state.last {
                    let dx = position.x - last.x;
                    let dy = position.y - last.y;
                    match mode {
                        DragMode::Orbit => camera.orbit(dx, dy, ORBIT_SENSITIVITY),
                        DragMode::Pan => camera.pan(dx, dy, PAN_SENSITIVITY),
                    }
                    state.last = Some(*position);
                    true
                } else {
                    state.last = Some(*position);
                    false
                }
            } else {
                false
            }
        }
        Event::Mouse(MouseEvent::WheelScrolled { delta, .. }) => {
            if !cursor_inside {
                return false;
            }
            camera.dolly(wheel_zoom_factor(delta));
            true
        }
        Event::Keyboard(_) => true,
        _ => false,
    }
}

/// Convert a wheel delta into a symmetric exponential dolly factor.
/// Positive wheel deltas zoom in, negative deltas zoom out. Exponential
/// scaling keeps a one-line step consistent in either direction and makes
/// high-resolution pixel scrolling feel continuous rather than stepped.
fn wheel_zoom_factor(delta: &ScrollDelta) -> f32 {
    let (amount, sensitivity) = match delta {
        ScrollDelta::Lines { y, .. } => (*y, WHEEL_ZOOM_PER_LINE),
        ScrollDelta::Pixels { y, .. } => (*y, WHEEL_ZOOM_PER_PIXEL),
    };
    (-amount * sensitivity).exp()
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for Scene3dWidget
where
    Renderer: primitive::Renderer,
{
    fn tag(&self) -> Tag {
        struct Marker;
        Tag::of::<Marker>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(DragState::default())
    }

    fn size(&self) -> Size<Length> {
        Size {
            width: self.width,
            height: self.height,
        }
    }

    fn layout(&mut self, _tree: &mut Tree, _renderer: &Renderer, limits: &Limits) -> Node {
        layout::atomic(limits, self.width, self.height)
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: layout::Layout<'_>,
        cursor: Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let state = tree.state.downcast_mut::<DragState>();
        let cursor_inside = cursor.position_in(bounds).is_some();
        let prev_inside = state.cursor_inside;
        state.cursor_inside = cursor_inside;
        let mut dirty = false;
        self.handle.with_mut(|inner| {
            let mut needs_redraw = handle_event(&mut inner.camera, state, event, cursor_inside);
            if !state.is_dragging() {
                state.last = None;
            }
            if !cursor_inside
                && !state.is_dragging()
                && matches!(event, Event::Mouse(MouseEvent::CursorMoved { .. }))
            {
                needs_redraw = false;
            }
            if needs_redraw {
                inner.dirty = true;
                dirty = true;
            }
            let _ = bounds;
        });
        if prev_inside != cursor_inside {
            dirty = true;
        }
        if dirty {
            shell.request_redraw();
        }
    }

    fn draw(
        &self,
        _tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: layout::Layout<'_>,
        _cursor: Cursor,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let primitive = ScenePrimitive {
            handle: self.handle.clone(),
            bounds,
        };
        renderer.draw_primitive(bounds, primitive);
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: layout::Layout<'_>,
        cursor: Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        let bounds = layout.bounds();
        let state = tree.state.downcast_ref::<DragState>();
        if cursor.position_in(bounds).is_none() {
            return mouse::Interaction::Idle;
        }
        let has_scene = self.handle.with(|i| i.scene.is_some());
        if !has_scene {
            return mouse::Interaction::Idle;
        }
        if state.is_dragging() {
            mouse::Interaction::Grabbing
        } else {
            mouse::Interaction::Grab
        }
    }
}

impl<'a, Message, Theme, Renderer> From<Scene3dWidget> for Element<'a, Message, Theme, Renderer>
where
    Renderer: primitive::Renderer,
    Theme: 'a,
    Message: 'a,
{
    fn from(w: Scene3dWidget) -> Self {
        Element::new(w)
    }
}

#[derive(Debug)]
pub struct ScenePrimitive {
    pub handle: Arc<SceneHandle>,
    pub bounds: Rectangle,
}

impl primitive::Primitive for ScenePrimitive {
    type Pipeline = ScenePipeline;

    fn prepare(
        &self,
        pipeline: &mut Self::Pipeline,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bounds: &Rectangle,
        viewport: &graphics::Viewport,
    ) {
        // Size the offscreen targets and the camera frustum to the
        // widget's physical rect, not the whole window — the composite
        // pass blits the offscreen texture 1:1 into the pane, so the
        // model is centred and undistorted.
        let scale = viewport.scale_factor();
        let width = ((bounds.width * scale).round() as u32).max(1);
        let height = ((bounds.height * scale).round() as u32).max(1);
        pipeline.prepared_this_frame = true;
        pipeline.record_last_viewport(width, height);
        pipeline.record_widget_rect(
            bounds.x * scale,
            bounds.y * scale,
            width as f32,
            height as f32,
        );
        let (Some(scene), origin_offset) = self.handle.with_mut(|inner| {
            if inner.scene.is_none() {
                return (None, [0.0; 3]);
            }
            inner
                .camera
                .set_viewport(crate::inspector::scene3d::camera::Viewport { width, height });
            let scene = inner.scene.clone();
            let offset = scene
                .as_ref()
                .map(|scene| scene_display_offset(scene, inner.origin_mode))
                .unwrap_or([0.0; 3]);
            (scene, offset)
        }) else {
            pipeline.release_scene_resources();
            return;
        };
        if let Some(error) = pipeline.gpu_error() {
            self.handle.set_gpu_error(error);
            pipeline.release_scene_resources();
            return;
        }
        if let Err(error) = validate_scene_for_device(device, &scene, width, height) {
            self.handle.set_gpu_error(error);
            pipeline.release_scene_resources();
            return;
        }
        self.handle.clear_gpu_error();
        pipeline.ensure_size(device, width, height);
        pipeline.ensure_offscreen(device, width, height);
        let (camera, flags) = self.handle.with(|i| (i.camera.clone(), i.flags));
        pipeline.upload_if_changed(device, queue, &scene, &camera, flags, origin_offset);
        let _ = device.poll(wgpu::PollType::Poll);
        if let Some(error) = pipeline.gpu_error() {
            self.handle.set_gpu_error(error);
            pipeline.release_scene_resources();
        }
    }

    fn draw(&self, _pipeline: &Self::Pipeline, _render_pass: &mut wgpu::RenderPass<'_>) -> bool {
        // Force Iced to route us through `render` instead — that's
        // where we have an encoder and can issue the offscreen render
        // pass. `draw` only gives us the compositor's ongoing render
        // pass, which has no depth attachment and so cannot rasterize
        // the lit 3D model.
        false
    }

    fn render(
        &self,
        pipeline: &Self::Pipeline,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        clip_bounds: &Rectangle<u32>,
    ) {
        let (scene, camera, flags, gpu_error) = self.handle.with(|i| {
            (
                i.scene.clone(),
                i.camera.clone(),
                i.flags,
                i.gpu_error.is_some(),
            )
        });
        if gpu_error || pipeline.gpu_error().is_some() {
            return;
        }
        let Some(scene) = scene else { return };
        // First pass: render the model into the offscreen color target.
        pipeline.render_to_offscreen(encoder, &scene, &camera, flags);
        // Second pass: composite the offscreen color into the frame.
        // Iced's compositor ended its main pass before calling us, so
        // `target` is the frame texture view we should bind. We open a
        // fresh pass with `LoadOp::Load` so everything Iced already drew
        // (text, icons, the tab bar) is preserved underneath.
        pipeline.composite_to_frame(encoder, target, clip_bounds);
    }
}

pub struct ScenePipeline {
    pub render_pipelines: ScenePipelines,
    pub depth_tex: Option<wgpu::Texture>,
    pub depth_view: Option<wgpu::TextureView>,
    pub width: u32,
    pub height: u32,
    /// Cached from the most recent `prepare` call so `render` can build a
    /// frustum camera with the same viewport. Public, mutated via
    /// `set_last_viewport`.
    pub last_viewport: (u32, u32),
    /// Physical on-screen rect of the widget `(x, y, w, h)`, recorded in
    /// `prepare`. The composite pass uses it as its viewport so the blit
    /// lands exactly over the pane (scissored to `clip_bounds`).
    pub widget_rect: [f32; 4],
    pub cached_scene_ptr: usize,
    pub cached_signature: u64,
    pub cached_flags_bits: u32,
    pub cached_origin_offset: [f32; 3],
    pub mesh_cache: Vec<(GpuMesh, Option<GpuTexture>)>,
    pub prepared_this_frame: bool,
    gpu_error: Arc<Mutex<Option<String>>>,
}

impl ScenePipeline {
    fn ensure_size(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if self.depth_tex.is_some() && self.width == width && self.height == height {
            return;
        }
        let (tex, view) = create_depth_texture(device, width, height, 1);
        self.depth_tex = Some(tex);
        self.depth_view = Some(view);
        self.width = width;
        self.height = height;
    }

    fn ensure_offscreen(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        self.render_pipelines
            .ensure_scene_color(device, width, height);
    }

    fn record_last_viewport(&mut self, width: u32, height: u32) {
        self.last_viewport = (width.max(1), height.max(1));
    }

    fn record_widget_rect(&mut self, x: f32, y: f32, width: f32, height: f32) {
        self.widget_rect = [x, y, width.max(1.0), height.max(1.0)];
    }

    fn upload_if_changed(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        scene: &Scene,
        camera: &OrbitCamera,
        flags: RenderFlags,
        origin_offset: [f32; 3],
    ) {
        let scene_ptr = scene as *const Scene as usize;
        let eff_flags = effective_texture_flag(scene, flags);
        let signature = if scene_ptr == self.cached_scene_ptr {
            self.cached_signature
        } else {
            scene_signature(scene)
        };
        let resource_flags = resource_cache_flags(eff_flags);
        if scene_ptr == self.cached_scene_ptr
            && signature == self.cached_signature
            && resource_flags == self.cached_flags_bits
            && origin_offset == self.cached_origin_offset
        {
            self.render_pipelines.update_camera(
                queue,
                camera,
                scene.key_light,
                scene.ambient,
                eff_flags,
            );
            return;
        }
        self.cached_scene_ptr = scene_ptr;
        self.cached_signature = signature;
        self.cached_flags_bits = resource_flags;
        self.cached_origin_offset = origin_offset;
        self.mesh_cache.clear();
        for mesh in &scene.meshes {
            let gpu = GpuMesh::from_scene_mesh_at_offset(device, queue, mesh, origin_offset);
            let tex = mesh.diffuse.as_ref().map(|t| {
                GpuTexture::from_scene_texture(
                    device,
                    queue,
                    t,
                    &self.render_pipelines.texture_layout,
                    &self.render_pipelines.texture_sampler,
                )
            });
            self.mesh_cache.push((gpu, tex));
        }
        self.render_pipelines.update_camera(
            queue,
            camera,
            scene.key_light,
            scene.ambient,
            eff_flags,
        );
    }

    fn release_scene_resources(&mut self) {
        self.mesh_cache.clear();
        self.cached_signature = 0;
        self.cached_scene_ptr = 0;
        self.cached_flags_bits = u32::MAX;
        self.cached_origin_offset = [0.0; 3];
        self.depth_view = None;
        self.depth_tex = None;
        self.width = 0;
        self.height = 0;
        self.render_pipelines.release_scene_color();
    }

    fn gpu_error(&self) -> Option<String> {
        self.gpu_error.lock().ok().and_then(|error| error.clone())
    }

    /// Render the 3D model into the offscreen color target + depth target.
    /// Called from `Primitive::render`; opens its own render pass.
    /// Sequence:
    ///   1. clear color to the F3D-style dark backdrop
    ///   2. draw the optional procedural infinity grid as a depth-neutral backdrop
    ///   3. draw the model meshes (lit shader, depth-tested and depth-owning)
    ///   4. draw optional polygon edges over the solid model
    ///   5. draw the XYZ axis gizmo in the bottom-right (overlay, no depth)
    pub fn render_to_offscreen(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        scene: &Scene,
        camera: &OrbitCamera,
        flags: RenderFlags,
    ) {
        let _ = scene;
        let _ = camera;
        let (Some(depth_view), Some(scene_color_view)) = (
            self.depth_view.as_ref(),
            self.render_pipelines.scene_color_view.as_ref(),
        ) else {
            return;
        };
        let flags = effective_texture_flag(scene, flags);
        log::trace!(
            target: "imgeditor.scene3d",
            "render_to_offscreen: {w}x{h}, mesh_count={mc}, textured_count={tc}, flags={flags:?}",
            w = self.width,
            h = self.height,
            mc = self.mesh_cache.len(),
            tc = self.mesh_cache.iter().filter(|(_, t)| t.is_some()).count(),
            flags = flags
        );
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("imgeditor-scene3d/render_offscreen"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: scene_color_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.06,
                        g: 0.07,
                        b: 0.09,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        // 2. optional procedural grid floor — fills the cleared color with
        // grid lines. It deliberately does not write depth: the floor is a
        // reference backdrop, so it must never hide model geometry below
        // or intersecting the world Y=0 plane.
        if flags.contains(RenderFlags::SHOW_GRID) {
            pass.set_pipeline(&self.render_pipelines.grid);
            pass.set_bind_group(0, &self.render_pipelines.camera_bind_group, &[]);
            pass.set_vertex_buffer(0, self.render_pipelines.quad_vertex_buffer.slice(..));
            pass.set_index_buffer(
                self.render_pipelines.quad_index_buffer.slice(..),
                wgpu::IndexFormat::Uint32,
            );
            pass.draw_indexed(0..6, 0, 0..1);
        }

        // 3. the model
        let lit_pipeline = if flags.contains(RenderFlags::CULL_BACK) {
            &self.render_pipelines.lit_cull_back
        } else {
            &self.render_pipelines.lit
        };
        pass.set_pipeline(lit_pipeline);
        pass.set_bind_group(0, &self.render_pipelines.camera_bind_group, &[]);

        for (gpu_mesh, tex) in &self.mesh_cache {
            let bg: &wgpu::BindGroup = match tex {
                Some(t) => &t.bind_group,
                None => &self.render_pipelines.default_diffuse.bind_group,
            };
            pass.set_bind_group(1, bg, &[]);
            pass.set_vertex_buffer(0, gpu_mesh.vertex_buffer.slice(..));
            pass.set_index_buffer(gpu_mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..gpu_mesh.index_count, 0, 0..1);
        }

        // 4. optional wire overlay. The overlay pipeline reads the solid
        // model's depth without writing it, so hidden edges stay hidden and
        // visible edges remain legible without z-fighting the surface.
        if flags.contains(RenderFlags::WIREFRAME) {
            pass.set_pipeline(&self.render_pipelines.wireframe);
            pass.set_bind_group(0, &self.render_pipelines.camera_bind_group, &[]);
            for (gpu_mesh, tex) in &self.mesh_cache {
                let bg: &wgpu::BindGroup = match tex {
                    Some(t) => &t.bind_group,
                    None => &self.render_pipelines.default_diffuse.bind_group,
                };
                pass.set_bind_group(1, bg, &[]);
                pass.set_vertex_buffer(0, gpu_mesh.vertex_buffer.slice(..));
                pass.set_index_buffer(
                    gpu_mesh.wire_index_buffer.slice(..),
                    wgpu::IndexFormat::Uint32,
                );
                pass.draw_indexed(0..gpu_mesh.wire_index_count, 0, 0..1);
            }
        }

        // 5. the XYZ axis gizmo in the bottom-right of the pane.
        pass.set_pipeline(&self.render_pipelines.gizmo);
        pass.set_bind_group(0, &self.render_pipelines.camera_bind_group, &[]);
        pass.set_vertex_buffer(0, self.render_pipelines.quad_vertex_buffer.slice(..));
        pass.set_index_buffer(
            self.render_pipelines.quad_index_buffer.slice(..),
            wgpu::IndexFormat::Uint32,
        );
        pass.draw_indexed(0..6, 0, 0..1);
    }

    /// Composite the offscreen color texture into the existing render pass
    /// that Iced's compositor is running. Called from `Primitive::draw`
    /// after Iced has set the viewport + scissor for the widget.
    pub fn composite(&self, pass: &mut wgpu::RenderPass<'_>) {
        let Some(bg) = self.render_pipelines.compositor_bind_group.as_ref() else {
            return;
        };
        pass.set_pipeline(&self.render_pipelines.compositor);
        pass.set_bind_group(0, bg, &[]);
        pass.set_vertex_buffer(0, self.render_pipelines.quad_vertex_buffer.slice(..));
        pass.set_index_buffer(
            self.render_pipelines.quad_index_buffer.slice(..),
            wgpu::IndexFormat::Uint32,
        );
        pass.draw_indexed(0..6, 0, 0..1);
    }

    /// Composite the offscreen color into the frame texture by opening a
    /// fresh render pass via the supplied encoder. Used by `render`
    /// after `render_to_offscreen` because Iced's compositor only calls
    /// `render` when `draw` returned `false` (which is our only path to
    /// reach the encoder with the frame texture view still available).
    pub fn composite_to_frame(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        clip_bounds: &Rectangle<u32>,
    ) {
        let Some(bg) = self.render_pipelines.compositor_bind_group.as_ref() else {
            return;
        };
        if clip_bounds.width == 0 || clip_bounds.height == 0 {
            return;
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("imgeditor-scene3d/composite"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        // The offscreen texture is sized to the widget, so the blit is
        // 1:1: place the fullscreen quad over the widget's physical rect
        // and let the scissor (clip bounds handed to us by Iced, already
        // snapped to physical pixels) cut it down when the pane is
        // partially occluded.
        let [bx, by, bw, bh] = self.widget_rect;
        pass.set_viewport(bx, by, bw, bh, 0.0, 1.0);
        pass.set_scissor_rect(
            clip_bounds.x,
            clip_bounds.y,
            clip_bounds.width,
            clip_bounds.height,
        );
        pass.set_pipeline(&self.render_pipelines.compositor);
        pass.set_bind_group(0, bg, &[]);
        pass.set_vertex_buffer(0, self.render_pipelines.quad_vertex_buffer.slice(..));
        pass.set_index_buffer(
            self.render_pipelines.quad_index_buffer.slice(..),
            wgpu::IndexFormat::Uint32,
        );
        pass.draw_indexed(0..6, 0, 0..1);
    }
}

impl PrimitivePipeline for ScenePipeline {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let render_pipelines = ScenePipelines::new(device, queue, format);
        Self {
            render_pipelines,
            depth_tex: None,
            depth_view: None,
            width: 0,
            height: 0,
            last_viewport: (1, 1),
            widget_rect: [0.0, 0.0, 1.0, 1.0],
            cached_scene_ptr: 0,
            cached_signature: 0,
            cached_flags_bits: 0,
            cached_origin_offset: [0.0; 3],
            mesh_cache: Vec::new(),
            prepared_this_frame: false,
            gpu_error: register_gpu_error_handlers(device),
        }
    }

    fn trim(&mut self) {
        // Iced calls `trim` at the end of every frame. It is a primitive
        // storage lifecycle hook, not a request to discard resources that
        // are still used by the next frame. Rebuilding here caused all NIF
        // buffers and textures to churn every frame and was the primary OOM
        // trigger for large scenes.
        if !self.prepared_this_frame {
            self.release_scene_resources();
        }
        self.prepared_this_frame = false;
    }
}

fn scene_signature(scene: &Scene) -> u64 {
    let mut sig: u64 = 0xcbf29ce484222325;
    for mesh in &scene.meshes {
        for v in &mesh.vertices {
            sig = (sig ^ v.position[0].to_bits() as u64).wrapping_mul(0x100000001b3);
            sig = (sig ^ v.position[1].to_bits() as u64).wrapping_mul(0x100000001b3);
            sig = (sig ^ v.position[2].to_bits() as u64).wrapping_mul(0x100000001b3);
        }
        for &i in &mesh.indices {
            sig = (sig ^ i as u64).wrapping_mul(0x100000001b3);
        }
        if let Some(t) = &mesh.diffuse {
            sig = (sig ^ t.width as u64).wrapping_mul(0x100000001b3);
            sig = (sig ^ t.height as u64).wrapping_mul(0x100000001b3);
            for chunk in t.rgba.chunks(64) {
                for &b in chunk {
                    sig = (sig ^ b as u64).wrapping_mul(0x100000001b3);
                }
            }
        }
    }
    sig
}

#[allow(dead_code)]
fn _type_asserts() {
    fn _ensure_send<T: Send>() {}
    _ensure_send::<SceneHandle>();
    _ensure_send::<ScenePrimitive>();
    _ensure_send::<ScenePipeline>();
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn handle_creates_with_no_scene() {
        let h = SceneHandle::new();
        assert!(h.with(|i| i.scene.is_none()));
    }

    #[test]
    fn grid_floor_is_enabled_by_default_and_toggleable() {
        let h = SceneHandle::new();
        assert!(h.with(|i| i.flags.contains(RenderFlags::SHOW_GRID)));

        h.toggle_grid();
        assert!(!h.with(|i| i.flags.contains(RenderFlags::SHOW_GRID)));

        h.toggle_grid();
        assert!(h.with(|i| i.flags.contains(RenderFlags::SHOW_GRID)));
    }

    #[test]
    fn set_scene_centres_camera_on_origin_by_default() {
        let h = SceneHandle::new();
        let scene = Scene {
            meshes: vec![crate::inspector::scene3d::mesh::SceneMesh {
                name: "unit".into(),
                texture_name: None,
                vertices: vec![],
                indices: vec![],
                diffuse: None,
                aabb: crate::inspector::scene3d::mesh::Aabb {
                    min: [-1.0, -2.0, -3.0],
                    max: [4.0, 5.0, 6.0],
                },
            }],
            aabb: crate::inspector::scene3d::mesh::Aabb {
                min: [-1.0, -2.0, -3.0],
                max: [4.0, 5.0, 6.0],
            },
            ambient: [0.2, 0.2, 0.22],
            key_light: [0.5, 0.7, 0.5],
            base_orientation: crate::inspector::scene3d::camera::BaseOrientation::Yup,
        };
        h.set_scene(std::sync::Arc::new(scene));
        h.with(|i| {
            assert!(i.scene.is_some());
            let c = i.camera.target;
            assert_eq!(i.origin_mode, SceneOriginMode::Centered);
            assert_eq!(c, [0.0, 0.0, 0.0]);
        });
    }

    #[test]
    fn origin_toggle_restores_authentic_world_position() {
        let h = SceneHandle::new();
        let scene = Scene {
            meshes: vec![],
            aabb: crate::inspector::scene3d::mesh::Aabb {
                min: [99.0, 198.0, 297.0],
                max: [105.0, 204.0, 303.0],
            },
            ..Scene::empty(crate::inspector::scene3d::camera::BaseOrientation::Zup)
        };
        h.set_scene(std::sync::Arc::new(scene));
        h.toggle_center_origin();
        h.with(|i| {
            assert_eq!(i.origin_mode, SceneOriginMode::World);
            assert_eq!(i.camera.target, [102.0, 201.0, 300.0]);
        });
        h.toggle_center_origin();
        h.with(|i| {
            assert_eq!(i.origin_mode, SceneOriginMode::Centered);
            assert_eq!(i.camera.target, [0.0, 0.0, 0.0]);
        });
    }

    #[test]
    fn clear_resets_handle() {
        let h = SceneHandle::new();
        h.clear();
        assert!(h.with(|i| i.scene.is_none()));
    }

    #[test]
    fn gpu_error_can_be_cleared_for_retry() {
        let h = SceneHandle::new();
        h.set_gpu_error("test GPU failure".into());
        assert_eq!(
            h.with(|i| i.gpu_error.clone()),
            Some("test GPU failure".to_string())
        );
        h.clear_gpu_error();
        assert!(h.with(|i| i.gpu_error.is_none()));
    }

    #[test]
    fn scene_signature_changes_with_geometry() {
        let a = Scene {
            meshes: vec![],
            aabb: crate::inspector::scene3d::mesh::Aabb::default(),
            ambient: [0.0, 0.0, 0.0],
            key_light: [0.0, 1.0, 0.0],
            base_orientation: crate::inspector::scene3d::camera::BaseOrientation::Yup,
        };
        let mut b = a.clone();
        b.meshes.push(crate::inspector::scene3d::mesh::SceneMesh {
            name: "x".into(),
            texture_name: None,
            vertices: vec![crate::inspector::scene3d::mesh::Vertex {
                position: [0.0, 1.0, 2.0],
                normal: [0.0, 1.0, 0.0],
                uv: [0.0, 0.0],
            }],
            indices: vec![0],
            diffuse: None,
            aabb: crate::inspector::scene3d::mesh::Aabb::default(),
        });
        assert_ne!(scene_signature(&a), scene_signature(&b));
    }

    #[test]
    fn raster_flags_do_not_invalidate_gpu_resource_cache() {
        let textured = RenderFlags::HAS_TEXTURE;
        assert_eq!(
            resource_cache_flags(textured),
            resource_cache_flags(textured | RenderFlags::WIREFRAME | RenderFlags::CULL_BACK)
        );
        assert_ne!(
            resource_cache_flags(RenderFlags::empty()),
            resource_cache_flags(RenderFlags::HAS_TEXTURE)
        );
    }

    #[test]
    fn wheel_zoom_uses_symmetric_exponential_steps() {
        let zoom_in = wheel_zoom_factor(&ScrollDelta::Lines { x: 0.0, y: 1.0 });
        let zoom_out = wheel_zoom_factor(&ScrollDelta::Lines { x: 0.0, y: -1.0 });

        assert!((0.93..1.0).contains(&zoom_in));
        assert!((1.0..1.08).contains(&zoom_out));
        assert!(
            (zoom_in * zoom_out - 1.0).abs() < 1e-5,
            "zoom steps should undo each other: in={zoom_in}, out={zoom_out}"
        );
    }

    #[test]
    fn wheel_zoom_pixel_and_line_rates_match() {
        let line = wheel_zoom_factor(&ScrollDelta::Lines { x: 0.0, y: 1.0 });
        let pixels = wheel_zoom_factor(&ScrollDelta::Pixels { x: 0.0, y: 40.0 });

        assert!((line - pixels).abs() < 1e-6);
    }

    #[test]
    fn scene_handle_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<SceneHandle>();
    }

    struct MockRenderer;

    impl renderer::Renderer for MockRenderer {
        fn start_layer(&mut self, _bounds: Rectangle) {}
        fn end_layer(&mut self) {}
        fn start_transformation(&mut self, _transformation: iced::Transformation) {}
        fn end_transformation(&mut self) {}
        fn fill_quad(&mut self, _quad: renderer::Quad, _background: impl Into<iced::Background>) {}
        fn reset(&mut self, _new_bounds: Rectangle) {}
        fn allocate_image(
            &mut self,
            _handle: &iced::advanced::image::Handle,
            _callback: impl FnOnce(
                Result<iced::advanced::image::Allocation, iced::advanced::image::Error>,
            ) + Send
            + 'static,
        ) {
        }
    }

    impl primitive::Renderer for MockRenderer {
        fn draw_primitive(&mut self, _bounds: Rectangle, _primitive: impl primitive::Primitive) {}
    }

    // Regression: `update` used to pre-seed `state.last` with the current
    // cursor position on every move, zeroing every drag delta so orbit/pan
    // never moved the camera.
    #[test]
    fn left_drag_orbits_camera() {
        type TestWidget = dyn Widget<(), iced::Theme, MockRenderer>;

        let handle = Arc::new(SceneHandle::new());
        let mut widget = Scene3dWidget::new(handle.clone());
        let mut tree = Tree {
            tag: <Scene3dWidget as Widget<(), iced::Theme, MockRenderer>>::tag(&widget),
            state: <Scene3dWidget as Widget<(), iced::Theme, MockRenderer>>::state(&widget),
            children: Vec::new(),
        };
        let node = Node::new(Size::new(200.0, 200.0));
        let viewport = Rectangle::new(Point::ORIGIN, Size::new(200.0, 200.0));
        let renderer = MockRenderer;
        let mut clipboard = iced::advanced::clipboard::Null;
        let mut messages = Vec::<()>::new();

        let mut drive = |widget: &mut Scene3dWidget, event: Event, cursor: Cursor| {
            TestWidget::update(
                widget,
                &mut tree,
                &event,
                layout::Layout::new(&node),
                cursor,
                &renderer,
                &mut clipboard,
                &mut Shell::new(&mut messages),
                &viewport,
            );
        };

        drive(
            &mut widget,
            Event::Mouse(MouseEvent::ButtonPressed(MouseButton::Left)),
            Cursor::Available(Point::new(50.0, 50.0)),
        );
        drive(
            &mut widget,
            Event::Mouse(MouseEvent::CursorMoved {
                position: Point::new(50.0, 50.0),
            }),
            Cursor::Available(Point::new(50.0, 50.0)),
        );
        drive(
            &mut widget,
            Event::Mouse(MouseEvent::CursorMoved {
                position: Point::new(80.0, 50.0),
            }),
            Cursor::Available(Point::new(80.0, 50.0)),
        );

        let yaw = handle.with(|i| i.camera.yaw);
        assert!(
            yaw.abs() > 1e-4,
            "expected left-drag to orbit the camera, yaw = {yaw}"
        );
    }

    #[test]
    fn middle_drag_pans_camera_without_orbiting() {
        type TestWidget = dyn Widget<(), iced::Theme, MockRenderer>;

        let handle = Arc::new(SceneHandle::new());
        let mut widget = Scene3dWidget::new(handle.clone());
        let mut tree = Tree {
            tag: <Scene3dWidget as Widget<(), iced::Theme, MockRenderer>>::tag(&widget),
            state: <Scene3dWidget as Widget<(), iced::Theme, MockRenderer>>::state(&widget),
            children: Vec::new(),
        };
        let node = Node::new(Size::new(200.0, 200.0));
        let viewport = Rectangle::new(Point::ORIGIN, Size::new(200.0, 200.0));
        let renderer = MockRenderer;
        let mut clipboard = iced::advanced::clipboard::Null;
        let mut messages = Vec::<()>::new();

        let mut drive = |widget: &mut Scene3dWidget, event: Event, cursor: Cursor| {
            TestWidget::update(
                widget,
                &mut tree,
                &event,
                layout::Layout::new(&node),
                cursor,
                &renderer,
                &mut clipboard,
                &mut Shell::new(&mut messages),
                &viewport,
            );
        };

        drive(
            &mut widget,
            Event::Mouse(MouseEvent::ButtonPressed(MouseButton::Middle)),
            Cursor::Available(Point::new(50.0, 50.0)),
        );
        drive(
            &mut widget,
            Event::Mouse(MouseEvent::CursorMoved {
                position: Point::new(50.0, 50.0),
            }),
            Cursor::Available(Point::new(50.0, 50.0)),
        );
        drive(
            &mut widget,
            Event::Mouse(MouseEvent::CursorMoved {
                position: Point::new(80.0, 70.0),
            }),
            Cursor::Available(Point::new(80.0, 70.0)),
        );

        let (target, yaw) = handle.with(|inner| (inner.camera.target, inner.camera.yaw));
        assert!(
            target.iter().any(|value| value.abs() > 1e-4),
            "expected middle-drag to pan the camera, target = {target:?}"
        );
        assert!(yaw.abs() < 1e-6, "middle-drag must not orbit, yaw = {yaw}");

        drive(
            &mut widget,
            Event::Mouse(MouseEvent::ButtonReleased(MouseButton::Middle)),
            Cursor::Available(Point::new(80.0, 70.0)),
        );
        let stopped = handle.with(|inner| inner.camera.target);
        drive(
            &mut widget,
            Event::Mouse(MouseEvent::CursorMoved {
                position: Point::new(120.0, 110.0),
            }),
            Cursor::Available(Point::new(120.0, 110.0)),
        );
        assert_eq!(
            handle.with(|inner| inner.camera.target),
            stopped,
            "releasing middle button must stop panning"
        );
    }

    #[test]
    fn wheel_zoom_is_scoped_to_viewer_bounds() {
        type TestWidget = dyn Widget<(), iced::Theme, MockRenderer>;

        let handle = Arc::new(SceneHandle::new());
        let mut widget = Scene3dWidget::new(handle.clone());
        let mut tree = Tree {
            tag: <Scene3dWidget as Widget<(), iced::Theme, MockRenderer>>::tag(&widget),
            state: <Scene3dWidget as Widget<(), iced::Theme, MockRenderer>>::state(&widget),
            children: Vec::new(),
        };
        let node = Node::new(Size::new(200.0, 200.0));
        let viewport = Rectangle::new(Point::ORIGIN, Size::new(200.0, 200.0));
        let renderer = MockRenderer;
        let mut clipboard = iced::advanced::clipboard::Null;
        let mut messages = Vec::<()>::new();
        let mut drive = |event: Event, cursor: Cursor| {
            TestWidget::update(
                &mut widget,
                &mut tree,
                &event,
                layout::Layout::new(&node),
                cursor,
                &renderer,
                &mut clipboard,
                &mut Shell::new(&mut messages),
                &viewport,
            );
        };

        let before = handle.with(|inner| inner.camera.distance);
        drive(
            Event::Mouse(MouseEvent::WheelScrolled {
                delta: ScrollDelta::Lines { x: 0.0, y: 1.0 },
            }),
            Cursor::Available(Point::new(300.0, 50.0)),
        );
        let outside = handle.with(|inner| inner.camera.distance);
        assert_eq!(outside, before, "outside scroll must not zoom the viewer");

        drive(
            Event::Mouse(MouseEvent::WheelScrolled {
                delta: ScrollDelta::Lines { x: 0.0, y: 1.0 },
            }),
            Cursor::Available(Point::new(100.0, 100.0)),
        );
        let inside = handle.with(|inner| inner.camera.distance);
        assert_ne!(inside, outside, "inside scroll should zoom the viewer");
    }
}
