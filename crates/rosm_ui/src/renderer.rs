//! 2D Map Canvas rasterizer rendering OSM primitives and interaction highlights.

use rosm_core::geo::LatLon;
use rosm_core::id::{NodeId, PrimitiveId};
use rosm_core::primitive::Way;
use rosm_core::DataSet;
use slint::{Rgba8Pixel, SharedPixelBuffer};

use crate::viewport::Viewport;

/// Color definitions for JOSM-style vector rendering (RGBA).
pub mod colors {
    pub const BG: [u8; 4] = [28, 30, 36, 255];
    pub const GRID: [u8; 4] = [45, 48, 58, 255];
    pub const HIGHWAY_PRIMARY: [u8; 4] = [251, 146, 60, 255]; // Orange
    pub const HIGHWAY_RESIDENTIAL: [u8; 4] = [226, 232, 240, 255]; // Light slate
    pub const BUILDING: [u8; 4] = [168, 85, 247, 255]; // Purple
    pub const WATERWAY: [u8; 4] = [56, 189, 248, 255]; // Sky blue
    pub const DEFAULT_WAY: [u8; 4] = [148, 163, 184, 255]; // Grey
    pub const SELECTED: [u8; 4] = [250, 204, 21, 255]; // Bright Yellow
    pub const NODE_DEFAULT: [u8; 4] = [203, 213, 225, 255];
    pub const NODE_TAGGED: [u8; 4] = [52, 211, 153, 255]; // Emerald
    pub const DRAWING_RUBBERBAND: [u8; 4] = [244, 63, 94, 255]; // Rose
}

/// Renders the dataset into an RGBA pixel buffer.
pub fn render_map(
    dataset: &DataSet,
    viewport: &Viewport,
    selected_primitive: Option<PrimitiveId>,
    drawing_nodes: &[NodeId],
    mouse_geo: Option<LatLon>,
    mut tile_manager: Option<&mut rosm_imagery::TileManager>,
) -> SharedPixelBuffer<Rgba8Pixel> {
    let w = viewport.width;
    let h = viewport.height;
    let mut buffer = SharedPixelBuffer::<Rgba8Pixel>::new(w, h);
    let bytes = buffer.make_mut_bytes();

    // 1. Clear background
    for chunk in bytes.as_chunks_mut::<4>().0 {
        chunk.copy_from_slice(&colors::BG);
    }

    // 2. Render slippy map raster tiles if enabled
    if let Some(ref mut mgr) = tile_manager {
        if mgr.is_enabled() {
            if let Some(source) = mgr.source().cloned() {
                render_imagery_tiles(bytes, w, h, viewport, mgr, &source);
            }
        }
    }

    // 3. Render subtle geographic grid
    render_grid(bytes, w, h, viewport);

    // 3. Render all ways
    for way in dataset.ways() {
        let is_selected = selected_primitive == Some(PrimitiveId::Way(way.id));
        render_way(bytes, w, h, viewport, dataset, way, is_selected);
    }

    // 4. Render active drawing in-progress
    if !drawing_nodes.is_empty() {
        render_drawing_preview(bytes, w, h, viewport, dataset, drawing_nodes, mouse_geo);
    }

    // 5. Render nodes
    for node in dataset.nodes() {
        let is_selected = selected_primitive == Some(PrimitiveId::Node(node.id));
        let (sx, sy) = viewport.latlon_to_screen(node.position);

        if sx >= -10.0 && sx < (w as f32 + 10.0) && sy >= -10.0 && sy < (h as f32 + 10.0) {
            let color = if is_selected {
                colors::SELECTED
            } else if !node.tags.is_empty() {
                colors::NODE_TAGGED
            } else {
                colors::NODE_DEFAULT
            };

            let radius = if is_selected {
                5
            } else if !node.tags.is_empty() {
                4
            } else {
                2
            };

            draw_filled_circle(bytes, w, h, sx as i32, sy as i32, radius, color);

            if is_selected {
                draw_circle_outline(
                    bytes,
                    w,
                    h,
                    sx as i32,
                    sy as i32,
                    radius + 2,
                    colors::SELECTED,
                );
            }
        }
    }

    buffer
}

fn render_grid(bytes: &mut [u8], w: u32, h: u32, vp: &Viewport) {
    // Dynamic grid step based on zoom
    let step_deg = if vp.zoom > 500_000.0 {
        0.0005
    } else if vp.zoom > 100_000.0 {
        0.002
    } else if vp.zoom > 20_000.0 {
        0.01
    } else if vp.zoom > 5_000.0 {
        0.05
    } else {
        0.2
    };

    let bbox = vp.visible_bbox();
    let min_lon = (bbox.min_lon / step_deg).floor() * step_deg;
    let max_lon = (bbox.max_lon / step_deg).ceil() * step_deg;
    let min_lat = (bbox.min_lat / step_deg).floor() * step_deg;
    let max_lat = (bbox.max_lat / step_deg).ceil() * step_deg;

    let mut lon = min_lon;
    while lon <= max_lon {
        let (sx, _) = vp.latlon_to_screen(LatLon::new(vp.center_lat, lon));
        let x = sx.round() as i32;
        if x >= 0 && (x as u32) < w {
            for y in 0..h {
                blend_pixel(bytes, w, h, x, y as i32, colors::GRID, 160);
            }
        }
        lon += step_deg;
    }

    let mut lat = min_lat;
    while lat <= max_lat {
        let (_, sy) = vp.latlon_to_screen(LatLon::new(lat, vp.center_lon));
        let y = sy.round() as i32;
        if y >= 0 && (y as u32) < h {
            for x in 0..w {
                blend_pixel(bytes, w, h, x as i32, y, colors::GRID, 160);
            }
        }
        lat += step_deg;
    }
}

fn render_way(
    bytes: &mut [u8],
    w: u32,
    h: u32,
    vp: &Viewport,
    dataset: &DataSet,
    way: &Way,
    is_selected: bool,
) {
    if way.nodes.len() < 2 {
        return;
    }

    let color = if is_selected {
        colors::SELECTED
    } else if way.tags.is_highway() {
        if way.tags.get("highway") == Some("primary") || way.tags.get("highway") == Some("trunk") {
            colors::HIGHWAY_PRIMARY
        } else {
            colors::HIGHWAY_RESIDENTIAL
        }
    } else if way.tags.is_building() {
        colors::BUILDING
    } else if way.tags.is_waterway() {
        colors::WATERWAY
    } else {
        colors::DEFAULT_WAY
    };

    let stroke_width = if is_selected { 3 } else { 2 };

    for pair in way.nodes.windows(2) {
        if let (Some(n1), Some(n2)) = (dataset.get_node(pair[0]), dataset.get_node(pair[1])) {
            let (x1, y1) = vp.latlon_to_screen(n1.position);
            let (x2, y2) = vp.latlon_to_screen(n2.position);
            draw_thick_line(
                bytes,
                w,
                h,
                x1 as i32,
                y1 as i32,
                x2 as i32,
                y2 as i32,
                stroke_width,
                color,
            );
        }
    }
}

fn render_drawing_preview(
    bytes: &mut [u8],
    w: u32,
    h: u32,
    vp: &Viewport,
    dataset: &DataSet,
    drawing_nodes: &[NodeId],
    mouse_geo: Option<LatLon>,
) {
    // Connect already placed drawing nodes
    for pair in drawing_nodes.windows(2) {
        if let (Some(n1), Some(n2)) = (dataset.get_node(pair[0]), dataset.get_node(pair[1])) {
            let (x1, y1) = vp.latlon_to_screen(n1.position);
            let (x2, y2) = vp.latlon_to_screen(n2.position);
            draw_thick_line(
                bytes,
                w,
                h,
                x1 as i32,
                y1 as i32,
                x2 as i32,
                y2 as i32,
                2,
                colors::DRAWING_RUBBERBAND,
            );
        }
    }

    // Rubberband line to mouse
    if let (Some(&last_id), Some(cur_pos)) = (drawing_nodes.last(), mouse_geo) {
        if let Some(last_node) = dataset.get_node(last_id) {
            let (x1, y1) = vp.latlon_to_screen(last_node.position);
            let (x2, y2) = vp.latlon_to_screen(cur_pos);
            draw_dashed_line(
                bytes,
                w,
                h,
                x1 as i32,
                y1 as i32,
                x2 as i32,
                y2 as i32,
                colors::DRAWING_RUBBERBAND,
            );
        }
    }
}

// --- Software Drawing Primitives ---

#[inline]
fn set_pixel(bytes: &mut [u8], w: u32, h: u32, x: i32, y: i32, color: [u8; 4]) {
    if x >= 0 && (x as u32) < w && y >= 0 && (y as u32) < h {
        let idx = ((y as u32 * w + x as u32) * 4) as usize;
        bytes[idx..idx + 4].copy_from_slice(&color);
    }
}

#[inline]
fn blend_pixel(bytes: &mut [u8], w: u32, h: u32, x: i32, y: i32, color: [u8; 4], alpha: u8) {
    if x >= 0 && (x as u32) < w && y >= 0 && (y as u32) < h {
        let idx = ((y as u32 * w + x as u32) * 4) as usize;
        let a = alpha as u32;
        let inv_a = 255 - a;
        bytes[idx] = ((color[0] as u32 * a + bytes[idx] as u32 * inv_a) / 255) as u8;
        bytes[idx + 1] = ((color[1] as u32 * a + bytes[idx + 1] as u32 * inv_a) / 255) as u8;
        bytes[idx + 2] = ((color[2] as u32 * a + bytes[idx + 2] as u32 * inv_a) / 255) as u8;
    }
}

fn draw_filled_circle(bytes: &mut [u8], w: u32, h: u32, cx: i32, cy: i32, r: i32, color: [u8; 4]) {
    let r2 = r * r;
    for dy in -r..=r {
        for dx in -r..=r {
            if dx * dx + dy * dy <= r2 {
                set_pixel(bytes, w, h, cx + dx, cy + dy, color);
            }
        }
    }
}

fn draw_circle_outline(bytes: &mut [u8], w: u32, h: u32, cx: i32, cy: i32, r: i32, color: [u8; 4]) {
    let r_inner2 = (r - 1) * (r - 1);
    let r_outer2 = r * r;
    for dy in -r..=r {
        for dx in -r..=r {
            let dist2 = dx * dx + dy * dy;
            if dist2 <= r_outer2 && dist2 >= r_inner2 {
                set_pixel(bytes, w, h, cx + dx, cy + dy, color);
            }
        }
    }
}

/// Bresenham's line algorithm with thickness
#[allow(clippy::too_many_arguments)]
fn draw_thick_line(
    bytes: &mut [u8],
    w: u32,
    h: u32,
    mut x0: i32,
    mut y0: i32,
    x1: i32,
    y1: i32,
    thickness: i32,
    color: [u8; 4],
) {
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    let rad = thickness / 2;

    loop {
        if thickness <= 1 {
            set_pixel(bytes, w, h, x0, y0, color);
        } else {
            for ty in -rad..=rad {
                for tx in -rad..=rad {
                    set_pixel(bytes, w, h, x0 + tx, y0 + ty, color);
                }
            }
        }

        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_dashed_line(
    bytes: &mut [u8],
    w: u32,
    h: u32,
    mut x0: i32,
    mut y0: i32,
    x1: i32,
    y1: i32,
    color: [u8; 4],
) {
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    let mut counter = 0;

    loop {
        if counter % 8 < 5 {
            set_pixel(bytes, w, h, x0, y0, color);
        }
        counter += 1;

        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

#[derive(Clone, Copy)]
struct ScreenRect {
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
}

fn render_imagery_tiles(
    bytes: &mut [u8],
    w: u32,
    h: u32,
    vp: &Viewport,
    mgr: &mut rosm_imagery::TileManager,
    source: &rosm_imagery::ImagerySource,
) {
    let zoom = rosm_imagery::zoom_level_from_pixels_per_deg(vp.zoom)
        .clamp(source.min_zoom, source.max_zoom);

    let bbox = vp.visible_bbox();
    let tiles = rosm_imagery::TileCoordinate::tiles_in_bbox(&bbox, zoom);

    // Limit visible tiles to at most 64 to maintain high FPS when zoomed out
    let tiles_to_render = if tiles.len() > 64 {
        &tiles[..64]
    } else {
        &tiles[..]
    };

    let opacity = mgr.opacity();

    for &tile in tiles_to_render {
        let bounds = tile.bounds();
        let nw = LatLon::new(bounds.max_lat, bounds.min_lon);
        let se = LatLon::new(bounds.min_lat, bounds.max_lon);

        let (x0, y0) = vp.latlon_to_screen(nw);
        let (x1, y1) = vp.latlon_to_screen(se);

        let rect = ScreenRect { x0, y0, x1, y1 };
        if let Some(resolved) = mgr.get_tile_or_ancestor(source, tile) {
            blit_tile(bytes, w, h, &resolved, tile, rect, opacity);
        } else if mgr.is_pending(source.id, tile) {
            draw_tile_placeholder(bytes, w, h, rect);
        }
    }
}

fn blit_tile(
    dest: &mut [u8],
    w: u32,
    h: u32,
    resolved: &rosm_imagery::ResolvedTile,
    requested_coord: rosm_imagery::TileCoordinate,
    rect: ScreenRect,
    opacity: f32,
) {
    let (x0, y0, x1, y1) = (rect.x0, rect.y0, rect.x1, rect.y1);
    let img = &resolved.image;
    let (img_w, img_h) = (img.width(), img.height());
    if img_w == 0 || img_h == 0 {
        return;
    }

    let (src_x0, src_y0, src_x1, src_y1) = if resolved.is_exact {
        (0.0_f32, 0.0_f32, img_w as f32, img_h as f32)
    } else {
        let dz = requested_coord.z.saturating_sub(resolved.actual_coord.z);
        let scale = (1u32 << dz) as f32;
        let dx = (requested_coord.x - (resolved.actual_coord.x << dz)) as f32;
        let dy = (requested_coord.y - (resolved.actual_coord.y << dz)) as f32;
        let u0 = (dx / scale) * img_w as f32;
        let u1 = ((dx + 1.0) / scale) * img_w as f32;
        let v0 = (dy / scale) * img_h as f32;
        let v1 = ((dy + 1.0) / scale) * img_h as f32;
        (u0, v0, u1, v1)
    };

    let dx_span = x1 - x0;
    let dy_span = y1 - y0;
    if dx_span.abs() < 0.5 || dy_span.abs() < 0.5 {
        return;
    }

    let min_x = (x0.round() as i32).max(0).min(w as i32);
    let max_x = (x1.round() as i32).max(0).min(w as i32);
    let min_y = (y0.round() as i32).max(0).min(h as i32);
    let max_y = (y1.round() as i32).max(0).min(h as i32);

    if min_x >= max_x || min_y >= max_y {
        return;
    }

    let src_w = src_x1 - src_x0;
    let src_h = src_y1 - src_y0;

    let alpha_int = (opacity.clamp(0.0, 1.0) * 255.0) as u32;

    for sy in min_y..max_y {
        let v = (sy as f32 - y0) / dy_span;
        let src_y = ((src_y0 + v * src_h) as u32).min(img_h - 1);
        let row_offset = (sy as usize * w as usize) * 4;

        for sx in min_x..max_x {
            let u = (sx as f32 - x0) / dx_span;
            let src_x = ((src_x0 + u * src_w) as u32).min(img_w - 1);

            let pixel = img.get_pixel(src_x, src_y);
            let pixel_alpha = (pixel[3] as u32 * alpha_int) / 255;
            let pixel_inv_alpha = 255 - pixel_alpha;

            let idx = row_offset + sx as usize * 4;
            if pixel_alpha == 255 {
                dest[idx] = pixel[0];
                dest[idx + 1] = pixel[1];
                dest[idx + 2] = pixel[2];
                dest[idx + 3] = 255;
            } else if pixel_alpha > 0 {
                dest[idx] = (((pixel[0] as u32 * pixel_alpha)
                    + (dest[idx] as u32 * pixel_inv_alpha))
                    / 255) as u8;
                dest[idx + 1] = (((pixel[1] as u32 * pixel_alpha)
                    + (dest[idx + 1] as u32 * pixel_inv_alpha))
                    / 255) as u8;
                dest[idx + 2] = (((pixel[2] as u32 * pixel_alpha)
                    + (dest[idx + 2] as u32 * pixel_inv_alpha))
                    / 255) as u8;
                dest[idx + 3] = 255;
            }
        }
    }
}

fn draw_tile_placeholder(dest: &mut [u8], w: u32, h: u32, rect: ScreenRect) {
    let (x0, y0, x1, y1) = (rect.x0, rect.y0, rect.x1, rect.y1);
    let min_x = (x0.round() as i32).max(0).min(w as i32 - 1);
    let max_x = (x1.round() as i32).max(0).min(w as i32 - 1);
    let min_y = (y0.round() as i32).max(0).min(h as i32 - 1);
    let max_y = (y1.round() as i32).max(0).min(h as i32 - 1);

    if min_x >= max_x || min_y >= max_y {
        return;
    }

    let color = [38, 42, 52, 255];
    for x in min_x..=max_x {
        let idx_top = (min_y as usize * w as usize + x as usize) * 4;
        let idx_bot = (max_y as usize * w as usize + x as usize) * 4;
        dest[idx_top..idx_top + 4].copy_from_slice(&color);
        dest[idx_bot..idx_bot + 4].copy_from_slice(&color);
    }
    for y in min_y..=max_y {
        let idx_left = (y as usize * w as usize + min_x as usize) * 4;
        let idx_right = (y as usize * w as usize + max_x as usize) * 4;
        dest[idx_left..idx_left + 4].copy_from_slice(&color);
        dest[idx_right..idx_right + 4].copy_from_slice(&color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::RgbaImage;
    use rosm_core::primitive::Node;
    use rosm_imagery::{TileCache, TileCoordinate, TileManager, OSM_CARTO};

    #[test]
    fn test_render_map_with_imagery() {
        let mut ds = DataSet::new();
        ds.insert_node(Node::new(1, LatLon::new(51.5074, -0.1278)));

        let vp = Viewport::new(400, 300);
        let cache = TileCache::new(None, 10);

        // Prepopulate cache with a mock tile
        let center_tile = TileCoordinate::from_latlon(LatLon::new(51.5074, -0.1278), 16);
        let mut test_img = RgbaImage::new(256, 256);
        for p in test_img.pixels_mut() {
            *p = image::Rgba([100, 150, 200, 255]);
        }
        cache.insert(OSM_CARTO.id, center_tile, &[], test_img);

        let mut mgr = TileManager::new(cache);
        mgr.set_opacity(0.8);

        let buffer = render_map(&ds, &vp, None, &[], None, Some(&mut mgr));
        assert_eq!(buffer.width(), 400);
        assert_eq!(buffer.height(), 300);
    }
}
