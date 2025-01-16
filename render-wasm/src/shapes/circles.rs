use skia_safe as skia;
use crate::math::Rect;

use super::{Corners, Stroke};

#[derive(Debug, Clone, PartialEq)]
pub struct CircleShape {

}

impl CircleShape {
  pub fn draw_stroke_on_circle(canvas: &skia::Canvas, stroke: &Stroke, rect: &Rect, selrect: &Rect) {
      // Draw the different kind of strokes for an oval is straightforward, we just need apply a stroke to:
      // - The same oval if it's a center stroke
      // - A bigger oval if it's an outer stroke
      // - A smaller oval if it's an outer stroke
      let stroke_rect = stroke.outer_rect(rect);
      canvas.draw_oval(&stroke_rect, &stroke.to_paint(selrect));
  }
}
