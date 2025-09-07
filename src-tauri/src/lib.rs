use data_url::DataUrl;
use image::ImageReader;
use imageproc::geometric_transformations;
use imageproc::geometric_transformations::Projection;
use imageproc::point::Point;
use serde::{Deserialize, Serialize};
use tauri::ipc::Response;
use thiserror::Error;

use std::fmt;
use std::io::Cursor;

#[derive(Debug, Serialize, Deserialize)]
struct ControlPoint {
    x: i32,
    y: i32,
}

#[derive(Debug)]
struct ImageSquaringError {
    message: String,
}

impl fmt::Display for ImageSquaringError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{0}", self.message)
    }
}

impl std::error::Error for ImageSquaringError {}

#[derive(Debug, Error)]
pub enum ErrorWrapper {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Image(#[from] image::ImageError),
    #[error(transparent)]
    DataUrl(#[from] data_url::DataUrlError),
    #[error(transparent)]
    Base64(#[from] data_url::forgiving_base64::InvalidBase64),
    #[error(transparent)]
    Squaring(#[from] ImageSquaringError),
}

impl Serialize for ErrorWrapper {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

fn scaled_control_points_to_projection(points: &Vec<(f32, f32)>) -> Option<Projection> {
    // From Oleksandr Kaleniuk's "Geometry for Programmers", pp. 118 - 119.
    match points.as_slice() {
        [(xt1, yt1), (xt2, yt2), (xt3, yt3), (xt4, yt4)] => {
            let g =
                (xt1 * yt3 - xt1 * yt4 - xt2 * yt3 + xt2 * yt4 - xt3 * yt1 + xt3 * yt2 + xt4 * yt1
                    - xt4 * yt2)
                    / (xt2 * yt3 - xt2 * yt4 - xt3 * yt2 + xt3 * yt4 + xt4 * yt2 - xt4 * yt3);
            let h =
                (xt1 * yt2 - xt1 * yt3 - xt2 * yt1 + xt2 * yt4 + xt3 * yt1 - xt3 * yt4 - xt4 * yt2
                    + xt4 * yt3)
                    / (xt2 * yt3 - xt2 * yt4 - xt3 * yt2 + xt3 * yt4 + xt4 * yt2 - xt4 * yt3);
            let e = h * yt4 - yt1 + yt4;
            let d = g * yt2 - yt1 + yt2;
            let b = h * xt4 - xt1 + xt4;
            let a = g * xt2 - xt1 + xt2;
            let c = 0.0 + xt1;
            let f = 0.0 + yt1;
            let i: f32 = 1.0;
            Projection::from_matrix([a, b, c, d, e, f, g, h, i])
        }
        _ => None,
    }
}

#[tauri::command]
fn process_image(
    image_data_uri: &str,
    control_points: Vec<ControlPoint>,
) -> Result<Response, ErrorWrapper> {
    assert!(control_points.len() == 4);
    let points: Vec<Point<i32>> = control_points
        .into_iter()
        .map(|cp| Point::<i32>::new(cp.x, cp.y))
        .collect();
    let mut convex_hull: Vec<Point<i32>> = imageproc::geometry::convex_hull(points);
    if convex_hull.len() != 4 {
        return Err(ErrorWrapper::Squaring(ImageSquaringError {
            message: String::from("Non-convex quadrilateral"),
        }));
    }
    let url = DataUrl::process(image_data_uri)?;
    let (body, _) = url.decode_to_vec()?;
    let image = ImageReader::new(Cursor::new(body))
        .with_guessed_format()?
        .decode()?;
    let mut first_point = 0;
    // Both in JavaScript and these Rust image packages, (0, 0) = top-left corner
    // and increasing y goes *down* the page.
    let mut min_mid_y = image.height() as i32;
    let mut min_x = image.width() as i32;
    let mut max_x = -1 as i32;
    let mut min_y = image.height() as i32;
    let mut max_y = -1 as i32;
    for i in 0..4 {
        let x = convex_hull[i].x;
        let y = convex_hull[i].y;
        min_x = std::cmp::min(x, min_x);
        max_x = std::cmp::max(x, max_x);
        min_y = std::cmp::min(y, min_y);
        max_y = std::cmp::max(y, max_y);
        let mid_y = (y + convex_hull[(i + 1) % 4].y) / 2;
        if mid_y < min_mid_y {
            min_mid_y = mid_y;
            first_point = if x < convex_hull[(i + 1) % 4].x {
                i
            } else {
                (i + 1) % 4
            };
        }
    }
    let new_width = (max_x - min_x) as f32;
    let new_height = (max_y - min_y) as f32;
    convex_hull.rotate_left(first_point);
    let image = image.crop_imm(
        min_x as u32,
        min_y as u32,
        new_width as u32,
        new_height as u32,
    );
    let scaled_hull_vec: Vec<(f32, f32)> = convex_hull
        .into_iter()
        .map(|p: Point<i32>| -> (f32, f32) {
            (
                ((p.x - min_x) as f32) / new_width,
                ((p.y - min_y) as f32) / new_height,
            )
        })
        .collect();
    let projection = scaled_control_points_to_projection(&scaled_hull_vec).unwrap();
    let projection = Projection::scale(1.0 / new_width, 1.0 / new_height)
        .and_then(projection.invert())
        .and_then(Projection::scale(new_width, new_height));
    let mut bytes: Vec<u8> = Vec::new();
    let squared = geometric_transformations::warp(
        &image.to_rgba8(),
        &projection,
        geometric_transformations::Interpolation::Nearest,
        image::Rgba([0, 0, 0, 0]),
    );
    squared.write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)?;
    Ok(tauri::ipc::Response::new(bytes))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![process_image])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
