use skia_safe::{self as skia, RRect};
use crate::math::Rect;

use super::{Corners, Stroke};

#[derive(Debug, Clone, PartialEq)]
pub struct RectShape {

}

impl RectShape {
  pub fn draw_stroke_on_rect(
      canvas: &skia::Canvas,
      stroke: &Stroke,
      rect: &Rect,
      selrect: &Rect,
      corners: &Option<Corners>,
  ) {
      // Draw the different kind of strokes for a rect is straightforward, we just need apply a stroke to:
      // - The same rect if it's a center stroke
      // - A bigger rect if it's an outer stroke
      // - A smaller rect if it's an outer stroke
      let stroke_rect = stroke.outer_rect(rect);
      let paint = stroke.to_paint(selrect);

      match corners {
          Some(radii) => {
              let radii = stroke.outer_corners(radii);
              let rrect = RRect::new_rect_radii(stroke_rect, &radii);
              canvas.draw_rrect(rrect, &paint);
          }
          None => {
              canvas.draw_rect(&stroke_rect, &paint);
          }
      }
  }
}
