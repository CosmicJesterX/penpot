use skia_safe as skia;
use skia::{Contains, Rect, RRect};
use std::collections::HashMap;
use uuid::Uuid;

use crate::math;
use crate::view::Viewbox;

mod blend;
mod gpu_state;
mod images;
mod options;

use crate::shapes::{CircleShape, Corners, Fill, Kind, Path, RectShape, Shape, Stroke, StrokeCap, StrokeKind};
use gpu_state::GpuState;
use options::RenderOptions;

pub use blend::BlendMode;
pub use images::*;

pub(crate) struct CachedSurfaceImage {
    pub image: Image,
    pub viewbox: Viewbox,
    has_all_shapes: bool,
}

impl CachedSurfaceImage {
    fn is_dirty_for_zooming(&mut self, viewbox: &Viewbox) -> bool {
        !self.has_all_shapes && !self.viewbox.area.contains(viewbox.area)
    }

    fn is_dirty_for_panning(&mut self, _viewbox: &Viewbox) -> bool {
        !self.has_all_shapes
    }
}

pub(crate) struct RenderState {
    gpu_state: GpuState,
    pub final_surface: skia::Surface,
    pub drawing_surface: skia::Surface,
    pub debug_surface: skia::Surface,
    pub font_provider: skia::textlayout::TypefaceFontProvider,
    pub cached_surface_image: Option<CachedSurfaceImage>,
    options: RenderOptions,
    pub viewbox: Viewbox,
    images: ImageStore,
    background_color: skia::Color,
}

impl RenderState {
    pub fn new(width: i32, height: i32) -> RenderState {
        // This needs to be done once per WebGL context.
        let mut gpu_state = GpuState::new();
        let mut final_surface = gpu_state.create_target_surface(width, height);
        let drawing_surface = final_surface
            .new_surface_with_dimensions((width, height))
            .unwrap();
        let debug_surface = final_surface
            .new_surface_with_dimensions((width, height))
            .unwrap();

        let mut font_provider = skia::textlayout::TypefaceFontProvider::new();
        let default_font = skia::FontMgr::default()
            .new_from_data(include_bytes!("fonts/RobotoMono-Regular.ttf"), None)
            .expect("Failed to load font");
        font_provider.register_typeface(default_font, "robotomono-regular");

        RenderState {
            gpu_state,
            final_surface,
            drawing_surface,
            debug_surface,
            cached_surface_image: None,
            font_provider,
            options: RenderOptions::default(),
            viewbox: Viewbox::new(width as f32, height as f32),
            images: ImageStore::new(),
            background_color: skia::Color::TRANSPARENT,
        }
    }

    pub fn add_font(&mut self, family_name: String, font_data: &[u8]) -> Result<(), String> {
        let typeface = skia::FontMgr::default()
            .new_from_data(font_data, None)
            .expect("Failed to add font");
        self.font_provider
            .register_typeface(typeface, family_name.as_ref());
        Ok(())
    }

    pub fn add_image(&mut self, id: Uuid, image_data: &[u8]) -> Result<(), String> {
        self.images.add(id, image_data)
    }

    pub fn has_image(&mut self, id: &Uuid) -> bool {
        self.images.contains(id)
    }

    pub fn set_debug_flags(&mut self, debug: u32) {
        self.options.debug_flags = debug;
    }

    pub fn set_dpr(&mut self, dpr: f32) {
        if Some(dpr) != self.options.dpr {
            self.options.dpr = Some(dpr);
            self.resize(
                self.viewbox.width.floor() as i32,
                self.viewbox.height.floor() as i32,
            );
        }
    }

    pub fn set_background_color(&mut self, color: skia::Color) {
        self.background_color = color;
        let _ = self.render_all_from_cache();
    }

    pub fn resize(&mut self, width: i32, height: i32) {
        let dpr_width = (width as f32 * self.options.dpr()).floor() as i32;
        let dpr_height = (height as f32 * self.options.dpr()).floor() as i32;

        let surface = self.gpu_state.create_target_surface(dpr_width, dpr_height);
        self.final_surface = surface;
        self.drawing_surface = self
            .final_surface
            .new_surface_with_dimensions((dpr_width, dpr_height))
            .unwrap();
        self.debug_surface = self
            .final_surface
            .new_surface_with_dimensions((dpr_width, dpr_height))
            .unwrap();

        self.viewbox.set_wh(width as f32, height as f32);
    }

    pub fn flush(&mut self) {
        self.gpu_state
            .context
            .flush_and_submit_surface(&mut self.final_surface, None)
    }

    pub fn translate(&mut self, dx: f32, dy: f32) {
        self.drawing_surface.canvas().translate((dx, dy));
    }

    pub fn scale(&mut self, sx: f32, sy: f32) {
        self.drawing_surface.canvas().scale((sx, sy));
    }

    pub fn reset_canvas(&mut self) {
        self.drawing_surface
            .canvas()
            .clear(skia::Color::TRANSPARENT)
            .reset_matrix();
        self.final_surface
            .canvas()
            .clear(self.background_color)
            .reset_matrix();
        self.debug_surface
            .canvas()
            .clear(skia::Color::TRANSPARENT)
            .reset_matrix();
    }

    pub fn render_single_element(&mut self, element: &mut Shape) {
        // element
        //     .render(&mut self.drawing_surface, &self.images, &self.font_provider)
        //     .unwrap();

        let transform = element.transform.to_skia_matrix();

        // Check transform-matrix code from common/src/app/common/geom/shapes/transforms.cljc
        let center = element.bounds().center();
        let mut matrix = skia::Matrix::new_identity();
        matrix.pre_translate(center);
        matrix.pre_concat(&transform);
        matrix.pre_translate(-center);

        self.drawing_surface.canvas().concat(&matrix);

        match &element.kind {
            Kind::SVGRaw(sr) => {
                if let Some(svg) = element.svg.as_ref() {
                    render_cached_svg(svg, &mut self.drawing_surface);
                } else {
                    if let Some(svg) = render_svg(
                        &sr.content.to_string(),
                        &mut self.drawing_surface,
                        &self.font_provider,
                    ) {
                        element.set_svg(svg);
                    }
                }
            }
            _ => {
                // let svg_canvas_required =
                //     matches!(&self.kind, Kind::Path(_)) && !self.svg_attrs.is_empty();
                // if svg_canvas_required {
                //     let svg_canvas = build_svg_canvas(self.selrect);
                //     render_fills_for_kind(
                //         self,
                //         &svg_canvas,
                //         images,
                //         self.to_path_transform().as_ref(),
                //     );
                //     render_svg_path_attrs(
                //         svg_canvas,
                //         &self.svg_attrs,
                //         self.selrect,
                //         surface,
                //         font_provider,
                //     );
                // } else {
                let canvas = self.drawing_surface.canvas();
                render_fills_for_kind(
                    element,
                    &canvas,
                    &self.images,
                    element.to_path_transform().as_ref(),
                    &element.svg_attrs,
                );
                // }
            }
        };

        self.drawing_surface.draw(
            &mut self.final_surface.canvas(),
            (0.0, 0.0),
            skia::SamplingOptions::new(skia::FilterMode::Linear, skia::MipmapMode::Nearest),
            Some(&skia::Paint::default()),
        );

        self.drawing_surface
            .canvas()
            .clear(skia::Color::TRANSPARENT);
    }

    pub fn zoom(&mut self, tree: &HashMap<Uuid, Shape>) -> Result<(), String> {
        if let Some(cached_surface_image) = self.cached_surface_image.as_mut() {
            let is_dirty = cached_surface_image.is_dirty_for_zooming(&self.viewbox);
            if is_dirty {
                self.render_all(tree, true);
            } else {
                self.render_all_from_cache()?;
            }
        }

        Ok(())
    }

    pub fn pan(&mut self, tree: &HashMap<Uuid, Shape>) -> Result<(), String> {
        if let Some(cached_surface_image) = self.cached_surface_image.as_mut() {
            let is_dirty = cached_surface_image.is_dirty_for_panning(&self.viewbox);
            if is_dirty {
                self.render_all(tree, true);
            } else {
                self.render_all_from_cache()?;
            }
        }

        Ok(())
    }

    pub fn render_all(&mut self, tree: &HashMap<Uuid, Shape>, generate_cached_surface_image: bool) {
        self.reset_canvas();
        self.scale(
            self.viewbox.zoom * self.options.dpr(),
            self.viewbox.zoom * self.options.dpr(),
        );
        self.translate(self.viewbox.pan_x, self.viewbox.pan_y);

        // Reset shape tree
        let is_complete = self.render_shape_tree(&Uuid::nil(), tree);
        if generate_cached_surface_image || self.cached_surface_image.is_none() {
            self.cached_surface_image = Some(CachedSurfaceImage {
                image: self.final_surface.image_snapshot(),
                viewbox: self.viewbox,
                has_all_shapes: is_complete,
            });
        }

        if self.options.is_debug_visible() {
            self.render_debug();
        }

        self.flush();
    }

    fn render_all_from_cache(&mut self) -> Result<(), String> {
        self.reset_canvas();

        let cached = self
            .cached_surface_image
            .as_ref()
            .ok_or("Uninitialized cached surface image")?;

        let image = &cached.image;
        let paint = skia::Paint::default();
        self.final_surface.canvas().save();
        self.drawing_surface.canvas().save();

        let navigate_zoom = self.viewbox.zoom / cached.viewbox.zoom;
        let navigate_x = cached.viewbox.zoom * (self.viewbox.pan_x - cached.viewbox.pan_x);
        let navigate_y = cached.viewbox.zoom * (self.viewbox.pan_y - cached.viewbox.pan_y);

        self.final_surface
            .canvas()
            .scale((navigate_zoom, navigate_zoom));
        self.final_surface.canvas().translate((
            navigate_x * self.options.dpr(),
            navigate_y * self.options.dpr(),
        ));
        self.final_surface
            .canvas()
            .draw_image(image.clone(), (0, 0), Some(&paint));

        self.final_surface.canvas().restore();
        self.drawing_surface.canvas().restore();

        self.flush();

        Ok(())
    }

    fn render_debug_view(&mut self) {
        let mut paint = skia::Paint::default();
        paint.set_style(skia::PaintStyle::Stroke);
        paint.set_color(skia::Color::from_argb(255, 255, 0, 255));
        paint.set_stroke_width(1.);

        let mut scaled_rect = self.viewbox.area.clone();
        let x = 100. + scaled_rect.x() * 0.2;
        let y = 100. + scaled_rect.y() * 0.2;
        let width = scaled_rect.width() * 0.2;
        let height = scaled_rect.height() * 0.2;
        scaled_rect.set_xywh(x, y, width, height);

        self.debug_surface.canvas().draw_rect(scaled_rect, &paint);
    }

    fn render_debug_element(&mut self, element: &Shape, intersected: bool) {
        let mut paint = skia::Paint::default();
        paint.set_style(skia::PaintStyle::Stroke);
        paint.set_color(if intersected {
            skia::Color::from_argb(255, 255, 255, 0)
        } else {
            skia::Color::from_argb(255, 0, 255, 255)
        });
        paint.set_stroke_width(1.);

        let mut scaled_rect = element.bounds();
        let x = 100. + scaled_rect.x() * 0.2;
        let y = 100. + scaled_rect.y() * 0.2;
        let width = scaled_rect.width() * 0.2;
        let height = scaled_rect.height() * 0.2;
        scaled_rect.set_xywh(x, y, width, height);

        self.debug_surface.canvas().draw_rect(scaled_rect, &paint);
    }

    fn render_debug(&mut self) {
        let paint = skia::Paint::default();
        self.render_debug_view();
        self.debug_surface.draw(
            &mut self.final_surface.canvas(),
            (0.0, 0.0),
            skia::SamplingOptions::new(skia::FilterMode::Linear, skia::MipmapMode::Nearest),
            Some(&paint),
        );
    }

    // Returns a boolean indicating if the viewbox contains the rendered shapes
    fn render_shape_tree(&mut self, root_id: &Uuid, tree: &HashMap<Uuid, Shape>) -> bool {
        if let Some(element) = tree.get(&root_id) {
            let mut is_complete = self.viewbox.area.contains(element.bounds());

            if !root_id.is_nil() {
                if !element.bounds().intersects(self.viewbox.area) || element.hidden() {
                    self.render_debug_element(element, false);
                    // TODO: This means that not all the shapes are rendered so we
                    // need to call a render_all on the zoom out.
                    return is_complete; // TODO return is_complete or return false??
                } else {
                    self.render_debug_element(element, true);
                }
            }

            let mut paint = skia::Paint::default();
            paint.set_blend_mode(element.blend_mode().into());
            paint.set_alpha_f(element.opacity());
            let filter = element.image_filter(self.viewbox.zoom * self.options.dpr());
            if let Some(image_filter) = filter {
                paint.set_image_filter(image_filter);
            }

            let layer_rec = skia::canvas::SaveLayerRec::default().paint(&paint);
            // This is needed so the next non-children shape does not carry this shape's transform
            self.final_surface.canvas().save_layer(&layer_rec);
            self.drawing_surface.canvas().save();

            if !root_id.is_nil() {
                self.render_single_element(&mut element.clone());
                if element.clip() {
                    self.drawing_surface.canvas().clip_rect(
                        element.bounds(),
                        skia::ClipOp::Intersect,
                        true,
                    );
                }
            }

            // draw all the children shapes
            if element.is_recursive() {
                for id in element.children_ids() {
                    is_complete = self.render_shape_tree(&id, tree) && is_complete;
                }
            }

            self.final_surface.canvas().restore();
            self.drawing_surface.canvas().restore();

            return is_complete;
        } else {
            eprintln!("Error: Element with root_id {root_id} not found in the tree.");
            return false;
        }

    }
}

pub fn render_fills_for_kind(
    shape: &Shape,
    canvas: &skia::Canvas,
    images: &ImageStore,
    path_transform: Option<&skia::Matrix>,
    svg_attrs: &HashMap<String, String>,
) {
    for fill in shape.fills().rev() {
        render_fill(
            canvas,
            images,
            fill,
            shape.selrect,
            &shape.kind,
            path_transform,
            svg_attrs,
        );
    }

    for stroke in shape.strokes().rev() {
        render_stroke(
            canvas,
            images,
            stroke,
            shape.selrect,
            &shape.kind,
            shape.to_path_transform().as_ref(),
            svg_attrs,
        );
    }
}

pub fn render_fill(
    canvas: &skia::Canvas,
    images: &ImageStore,
    fill: &Fill,
    selrect: math::Rect,
    kind: &Kind,
    path_transform: Option<&skia::Matrix>,
    svg_attrs: &HashMap<String, String>,
) {
    match (fill, kind) {
        (Fill::Image(image_fill), kind) => {
            let image = images.get(&image_fill.id());
            if let Some(image) = image {
                draw_image_fill_in_container(
                    canvas,
                    &image,
                    image_fill.size(),
                    kind,
                    &fill.to_paint(&selrect),
                    &selrect,
                    path_transform,
                );
            }
        }
        (_, Kind::Rect(rect, None)) => {
            canvas.draw_rect(rect, &fill.to_paint(&selrect));
        }
        (_, Kind::Rect(rect, Some(corners))) => {
            let rrect = RRect::new_rect_radii(rect, corners);
            canvas.draw_rrect(rrect, &fill.to_paint(&selrect));
        }
        (_, Kind::Circle(rect)) => {
            canvas.draw_oval(rect, &fill.to_paint(&selrect));
        }
        (_, Kind::Path(path)) | (_, Kind::Bool(_, path)) => {
            let mut skia_path = &mut path.to_skia_path();
            skia_path = skia_path.transform(path_transform.unwrap());
            if let Some("evenodd") = svg_attrs.get("fill-rule").map(String::as_str) {
                skia_path.set_fill_type(skia::PathFillType::EvenOdd);
            }

            canvas.draw_path(&skia_path, &fill.to_paint(&selrect));
        }
        (_, _) => todo!()
    }
}

fn render_stroke(
    canvas: &skia::Canvas,
    images: &ImageStore,
    stroke: &Stroke,
    selrect: math::Rect,
    kind: &Kind,
    path_transform: Option<&skia::Matrix>,
    svg_attrs: &HashMap<String, String>,
) {
    if let Fill::Image(image_fill) = &stroke.fill {
        if let Some(image) = images.get(&image_fill.id()) {
            draw_image_stroke_in_container(
                canvas,
                &image,
                stroke,
                image_fill.size(),
                kind,
                &selrect,
                path_transform,
                svg_attrs,
            );
        }
    } else {
        match kind {
            Kind::Rect(rect, corners) => {
                RectShape::draw_stroke_on_rect(canvas, stroke, rect, &selrect, corners)
            }
            Kind::Circle(rect) => CircleShape::draw_stroke_on_circle(canvas, stroke, rect, &selrect),
            Kind::Path(path) | Kind::Bool(_, path) => {
                Path::draw_stroke_on_path(canvas, stroke, path, &selrect, path_transform, svg_attrs);
            }
            Kind::SVGRaw(_) => todo!()
        }
    }
}

fn calculate_scaled_rect(size: (i32, i32), container: &math::Rect, delta: f32) -> math::Rect {
    let (width, height) = (size.0 as f32, size.1 as f32);
    let image_aspect_ratio = width / height;

    // Container size
    let container_width = container.width();
    let container_height = container.height();
    let container_aspect_ratio = container_width / container_height;

    let scale = if image_aspect_ratio > container_aspect_ratio {
        container_height / height
    } else {
        container_width / width
    };

    let scaled_width = width * scale;
    let scaled_height = height * scale;

    math::Rect::from_xywh(
        container.left - delta - (scaled_width - container_width) / 2.0,
        container.top - delta - (scaled_height - container_height) / 2.0,
        scaled_width + (2. * delta) + (scaled_width - container_width),
        scaled_height + (2. * delta) + (scaled_width - container_width),
    )
}


fn handle_stroke_cap(
    canvas: &skia::Canvas,
    cap: StrokeCap,
    width: f32,
    paint: &mut skia::Paint,
    p1: &skia::Point,
    p2: &skia::Point,
) {
    paint.set_style(skia::PaintStyle::Fill);
    paint.set_blend_mode(skia::BlendMode::Src);
    match cap {
        StrokeCap::None => {}
        StrokeCap::Line => {
            paint.set_style(skia::PaintStyle::Stroke);
            draw_arrow_cap(canvas, &paint, p1, p2, width * 4.);
        }
        StrokeCap::Triangle => {
            draw_triangle_cap(canvas, &paint, p1, p2, width * 4.);
        }
        StrokeCap::Rectangle => {
            draw_square_cap(canvas, &paint, p1, p2, width * 4., 0.);
        }
        StrokeCap::Circle => {
            canvas.draw_circle((p1.x, p1.y), width * 2., &paint);
        }
        StrokeCap::Diamond => {
            draw_square_cap(canvas, &paint, p1, p2, width * 4., 45.);
        }
        StrokeCap::Round => {
            canvas.draw_circle((p1.x, p1.y), width / 2.0, &paint);
        }
        StrokeCap::Square => {
            draw_square_cap(canvas, &paint, p1, p2, width, 0.);
        }
    }
}

fn handle_stroke_caps(
    path: &mut skia::Path,
    stroke: &Stroke,
    selrect: &Rect,
    canvas: &skia::Canvas,
    is_open: bool,
) {
    let points_count = path.count_points();
    let mut points = vec![skia::Point::default(); points_count];
    let c_points = path.get_points(&mut points);

    // Closed shapes don't have caps
    if c_points >= 2 && is_open {
        let first_point = points.first().unwrap();
        let last_point = points.last().unwrap();

        let kind = stroke.render_kind(is_open);
        let mut paint_stroke = stroke.to_stroked_paint(kind.clone(), selrect);

        handle_stroke_cap(
            canvas,
            stroke.cap_start,
            stroke.width,
            &mut paint_stroke,
            first_point,
            &points[1],
        );
        handle_stroke_cap(
            canvas,
            stroke.cap_end,
            stroke.width,
            &mut paint_stroke,
            last_point,
            &points[points_count - 2],
        );
    }
}

fn draw_square_cap(
    canvas: &skia::Canvas,
    paint: &skia::Paint,
    center: &skia::Point,
    direction: &skia::Point,
    size: f32,
    extra_rotation: f32,
) {
    let dx = direction.x - center.x;
    let dy = direction.y - center.y;
    let angle = dy.atan2(dx);

    let mut matrix = skia::Matrix::new_identity();
    matrix.pre_rotate(
        angle.to_degrees() + extra_rotation,
        skia::Point::new(center.x, center.y),
    );

    let half_size = size / 2.0;
    let rect = skia::Rect::from_xywh(center.x - half_size, center.y - half_size, size, size);

    let points = [
        skia::Point::new(rect.left(), rect.top()),
        skia::Point::new(rect.right(), rect.top()),
        skia::Point::new(rect.right(), rect.bottom()),
        skia::Point::new(rect.left(), rect.bottom()),
    ];

    let mut transformed_points = points.clone();
    matrix.map_points(&mut transformed_points, &points);

    let mut path = skia::Path::new();
    path.move_to(skia::Point::new(center.x, center.y));
    path.move_to(transformed_points[0]);
    path.line_to(transformed_points[1]);
    path.line_to(transformed_points[2]);
    path.line_to(transformed_points[3]);
    path.close();
    canvas.draw_path(&path, paint);
}

fn draw_arrow_cap(
    canvas: &skia::Canvas,
    paint: &skia::Paint,
    center: &skia::Point,
    direction: &skia::Point,
    size: f32,
) {
    let dx = direction.x - center.x;
    let dy = direction.y - center.y;
    let angle = dy.atan2(dx);

    let mut matrix = skia::Matrix::new_identity();
    matrix.pre_rotate(
        angle.to_degrees() - 90.,
        skia::Point::new(center.x, center.y),
    );

    let half_height = size / 2.;
    let points = [
        skia::Point::new(center.x, center.y - half_height),
        skia::Point::new(center.x - size, center.y + half_height),
        skia::Point::new(center.x + size, center.y + half_height),
    ];

    let mut transformed_points = points.clone();
    matrix.map_points(&mut transformed_points, &points);

    let mut path = skia::Path::new();
    path.move_to(transformed_points[1]);
    path.line_to(transformed_points[0]);
    path.line_to(transformed_points[2]);
    path.move_to(skia::Point::new(center.x, center.y));
    path.line_to(transformed_points[0]);

    canvas.draw_path(&path, paint);
}

fn draw_triangle_cap(
    canvas: &skia::Canvas,
    paint: &skia::Paint,
    center: &skia::Point,
    direction: &skia::Point,
    size: f32,
) {
    let dx = direction.x - center.x;
    let dy = direction.y - center.y;
    let angle = dy.atan2(dx);

    let mut matrix = skia::Matrix::new_identity();
    matrix.pre_rotate(
        angle.to_degrees() - 90.,
        skia::Point::new(center.x, center.y),
    );

    let half_height = size / 2.;
    let points = [
        skia::Point::new(center.x, center.y - half_height),
        skia::Point::new(center.x - size, center.y + half_height),
        skia::Point::new(center.x + size, center.y + half_height),
    ];

    let mut transformed_points = points.clone();
    matrix.map_points(&mut transformed_points, &points);

    let mut path = skia::Path::new();
    path.move_to(transformed_points[0]);
    path.line_to(transformed_points[1]);
    path.line_to(transformed_points[2]);
    path.close();

    canvas.draw_path(&path, paint);
}



pub fn draw_image_stroke_in_container(
    canvas: &skia::Canvas,
    image: &Image,
    stroke: &Stroke,
    size: (i32, i32),
    kind: &Kind,
    container: &Rect,
    path_transform: Option<&skia::Matrix>,
    svg_attrs: &HashMap<String, String>
) {
    // Helper to handle drawing based on kind
    fn draw_kind(
        canvas: &skia::Canvas,
        kind: &Kind,
        stroke: &Stroke,
        container: &Rect,
        path_transform: Option<&skia::Matrix>,
    ) {
        let outer_rect = stroke.outer_rect(container);
        match kind {
            Kind::Rect(rect, corners) => {
                RectShape::draw_stroke_on_rect(canvas, stroke, rect, &outer_rect, corners)
            }
            Kind::Circle(rect) => CircleShape::draw_stroke_on_circle(canvas, stroke, rect, &outer_rect),
            Kind::SVGRaw(_) => todo!(),
            Kind::Path(p) | Kind::Bool(_, p) => {
                let mut path = p.to_skia_path();
                path.transform(path_transform.unwrap());
                let stroke_kind = stroke.render_kind(p.is_open());
                if stroke_kind == StrokeKind::InnerStroke {
                    canvas.clip_path(&path, skia::ClipOp::Intersect, true);
                }
                let paint = stroke.to_stroked_paint(stroke_kind, &outer_rect);
                canvas.draw_path(&path, &paint);
                handle_stroke_caps(&mut path, stroke, &outer_rect, canvas, p.is_open());
            }
        }
    }

    // Save canvas and layer state
    let mut pb = skia::Paint::default();
    pb.set_blend_mode(skia::BlendMode::SrcOver);
    pb.set_anti_alias(true);
    let layer_rec = skia::canvas::SaveLayerRec::default().paint(&pb);
    canvas.save_layer(&layer_rec);

    // Draw the stroke based on the kind, we are using this stroke as a "selector" of the area of the image we want to show.
    draw_kind(canvas, kind, stroke, container, path_transform);

    // Draw the image. We are using now the SrcIn blend mode, so the rendered piece of image will the area of the stroke over the image.
    let mut image_paint = skia::Paint::default();
    image_paint.set_blend_mode(skia::BlendMode::SrcIn);
    image_paint.set_anti_alias(true);
    // Compute scaled rect and clip to it
    let dest_rect = calculate_scaled_rect(size, container, stroke.delta());
    canvas.clip_rect(dest_rect, skia::ClipOp::Intersect, true);
    canvas.draw_image_rect(image, None, dest_rect, &image_paint);

    // Clear outer stroke for paths if necessary. When adding an outer stroke we need to empty the stroke added too in the inner area.
    if let Kind::Path(p) = kind {
        if stroke.render_kind(p.is_open()) == StrokeKind::OuterStroke {
            let mut path = p.to_skia_path();
            path.transform(path_transform.unwrap());
            let mut clear_paint = skia::Paint::default();
            clear_paint.set_blend_mode(skia::BlendMode::Clear);
            clear_paint.set_anti_alias(true);
            canvas.draw_path(&path, &clear_paint);
        }
    }

    // Restore canvas state
    canvas.restore();
}


pub fn draw_image_fill_in_container(
    canvas: &skia::Canvas,
    image: &Image,
    size: (i32, i32),
    kind: &Kind,
    paint: &skia::Paint,
    container: &math::Rect,
    path_transform: Option<&skia::Matrix>,
) {
    let width = size.0 as f32;
    let height = size.1 as f32;
    let image_aspect_ratio = width / height;

    // Container size
    let container_width = container.width();
    let container_height = container.height();
    let container_aspect_ratio = container_width / container_height;

    // Calculate scale to ensure the image covers the container
    let scale = if image_aspect_ratio > container_aspect_ratio {
        // Image is wider, scale based on height to cover container
        container_height / height
    } else {
        // Image is taller, scale based on width to cover container
        container_width / width
    };

    // Scaled size of the image
    let scaled_width = width * scale;
    let scaled_height = height * scale;

    let dest_rect = math::Rect::from_xywh(
        container.left - (scaled_width - container_width) / 2.0,
        container.top - (scaled_height - container_height) / 2.0,
        scaled_width,
        scaled_height,
    );

    // Save the current canvas state
    canvas.save();

    // Set the clipping rectangle to the container bounds
    match kind {
        Kind::Rect(_, _) => {
            canvas.clip_rect(container, skia::ClipOp::Intersect, true);
        }
        Kind::Circle(_) => {
            let mut oval_path = skia::Path::new();
            oval_path.add_oval(container, None);
            canvas.clip_path(&oval_path, skia::ClipOp::Intersect, true);
        }
        Kind::Path(p) => {
            canvas.clip_path(
                &p.to_skia_path().transform(path_transform.unwrap()),
                skia::ClipOp::Intersect,
                true,
            );
        }
        Kind::SVGRaw(_) => {
            canvas.clip_rect(container, skia::ClipOp::Intersect, true);
        },
        Kind::Bool(_, _) => todo!()
    }

    // Draw the image with the calculated destination rectangle
    canvas.draw_image_rect(image, None, dest_rect, &paint);

    // Restore the canvas to remove the clipping
    canvas.restore();
}

fn render_cached_svg(dom: &skia::svg::Dom, surface: &mut skia::Surface) {
    dom.render(surface.canvas());
}

fn render_svg(
    svg: &str,
    surface: &mut skia::Surface,
    font_provider: &skia::textlayout::TypefaceFontProvider,
) -> Option<skia::svg::Dom> {
    let font_manager = skia::FontMgr::from(font_provider.clone());
    let dom_result = skia::svg::Dom::from_str(svg, font_manager);
    match dom_result {
        Ok(dom) => {
            dom.render(surface.canvas());
            Some(dom)
        }
        Err(e) => {
            eprintln!("Error parsing SVG. Error: {}", e);
            None
        }
    }
}
