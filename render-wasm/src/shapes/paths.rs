use skia_safe as skia;
use std::{array::TryFromSliceError, collections::HashMap};

use crate::math::{self, Point};

use super::{Stroke, StrokeKind};

fn stringify_slice_err(_: TryFromSliceError) -> String {
    format!("Error deserializing path")
}

#[derive(Debug)]
pub struct RawPathData {
    pub data: [u8; 28],
}

impl RawPathData {
    fn command(&self) -> Result<u16, String> {
        let cmd = u16::from_be_bytes(self.data[0..2].try_into().map_err(stringify_slice_err)?);
        Ok(cmd)
    }

    fn xy(&self) -> Result<Point, String> {
        let x = f32::from_be_bytes(self.data[20..24].try_into().map_err(stringify_slice_err)?);
        let y = f32::from_be_bytes(self.data[24..].try_into().map_err(stringify_slice_err)?);
        Ok((x, y))
    }

    fn c1(&self) -> Result<Point, String> {
        let c1_x = f32::from_be_bytes(self.data[4..8].try_into().map_err(stringify_slice_err)?);
        let c1_y = f32::from_be_bytes(self.data[8..12].try_into().map_err(stringify_slice_err)?);

        Ok((c1_x, c1_y))
    }

    fn c2(&self) -> Result<Point, String> {
        let c2_x = f32::from_be_bytes(self.data[12..16].try_into().map_err(stringify_slice_err)?);
        let c2_y = f32::from_be_bytes(self.data[16..20].try_into().map_err(stringify_slice_err)?);

        Ok((c2_x, c2_y))
    }
}

const MOVE_TO: u16 = 1;
const LINE_TO: u16 = 2;
const CURVE_TO: u16 = 3;
const CLOSE: u16 = 4;

#[derive(Debug, PartialEq, Copy, Clone)]
enum Segment {
    MoveTo(Point),
    LineTo(Point),
    CurveTo((Point, Point, Point)),
    Close,
}

impl TryFrom<RawPathData> for Segment {
    type Error = String;
    fn try_from(value: RawPathData) -> Result<Self, Self::Error> {
        let cmd = value.command()?;
        match cmd {
            MOVE_TO => Ok(Segment::MoveTo(value.xy()?)),
            LINE_TO => Ok(Segment::LineTo(value.xy()?)),
            CURVE_TO => Ok(Segment::CurveTo((value.c1()?, value.c2()?, value.xy()?))),
            CLOSE => Ok(Segment::Close),
            _ => Err(format!(
                "Error deserializing path. Unknown command/flags: {:#010x}",
                cmd
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Path {
    segments: Vec<Segment>,
    skia_path: skia::Path,
    open: bool,
}

fn starts_and_ends_at_same_point(path: &skia::Path) -> bool {
    if path.count_points() < 2 {
        return false; // A path with fewer than 2 points cannot be closed
    }

    let start_point = path.get_point(0); // First point of the path
    let end_point = path.get_point(path.count_points() - 1); // Last point of the path

    // Compare the start and end points
    start_point == end_point
}

impl Default for Path {
    fn default() -> Self {
        Path::try_from(Vec::new()).unwrap()
    }
}

impl TryFrom<Vec<RawPathData>> for Path {
    type Error = String;

    fn try_from(value: Vec<RawPathData>) -> Result<Self, Self::Error> {
        let mut open = true;
        let segments = value
            .into_iter()
            .map(|raw| Segment::try_from(raw))
            .collect::<Result<Vec<Segment>, String>>()?;

        let mut skia_path = skia::Path::new();
        for segment in segments.iter() {
            match *segment {
                Segment::MoveTo(xy) => {
                    skia_path.move_to(xy);
                }
                Segment::LineTo(xy) => {
                    skia_path.line_to(xy);
                }
                Segment::CurveTo((c1, c2, xy)) => {
                    skia_path.cubic_to(c1, c2, xy);
                }
                Segment::Close => {
                    skia_path.close();
                    open = false;
                }
            }
        }

        if !skia_path.is_last_contour_closed() && starts_and_ends_at_same_point(&skia_path) {
            skia_path.close();
            open = false;
        }

        Ok(Path {
            segments,
            skia_path,
            open,
        })
    }
}

impl Path {
    pub fn draw_stroke_on_path(
        canvas: &skia::Canvas,
        stroke: &Stroke,
        path: &Path,
        selrect: &math::Rect,
        path_transform: Option<&skia::Matrix>,
        svg_attrs: &HashMap<String, String>,
    ) {
        let mut skia_path = path.to_skia_path();
        skia_path.transform(path_transform.unwrap());

        let paint_stroke = stroke.to_stroked_paint(stroke.kind, selrect);
        // Draw the different kind of strokes for a path requires different strategies:
        match stroke.kind {
            // For inner stroke we draw a center stroke (with double width) and clip to the original path (that way the extra outer stroke is removed)
            StrokeKind::InnerStroke => {
                canvas.clip_path(&skia_path, skia::ClipOp::Intersect, true);
                canvas.draw_path(&skia_path, &paint_stroke);
            }
            // For center stroke we don't need to do anything extra
            StrokeKind::CenterStroke => {
                canvas.draw_path(&skia_path, &paint_stroke);
            }
            // For outer stroke we draw a center stroke (with double width) and use another path with blend mode clear to remove the inner stroke added
            StrokeKind::OuterStroke => {
                let mut paint = skia::Paint::default();
                paint.set_blend_mode(skia::BlendMode::SrcOver);
                paint.set_anti_alias(true);
                let layer_rec = skia::canvas::SaveLayerRec::default().paint(&paint);
                canvas.save_layer(&layer_rec);

                canvas.draw_path(&skia_path, &paint_stroke);

                let mut clear_paint = skia::Paint::default();
                clear_paint.set_blend_mode(skia::BlendMode::Clear);
                clear_paint.set_anti_alias(true);
                canvas.draw_path(&skia_path, &clear_paint);

                canvas.restore();
            }
        }
    }

    pub fn to_skia_path(&self) -> skia::Path {
        self.skia_path.snapshot()
    }

    pub fn is_open(&self) -> bool {
        self.open
    }
}
