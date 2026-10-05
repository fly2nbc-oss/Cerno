//! Face detection (YuNet) and sharpness around the eyes.
//!
//! For portraits with shallow depth of field the tile-based sharpness can mislead: skin has
//! little texture and a small subject covers few tiles. What matters is whether the eyes are
//! in focus, so the Laplacian variance is measured in small squares around them.
//!
//! YuNet (OpenCV Zoo, MIT) is tiny and embedded. Input: 640 × 640, BGR, 0–255, no
//! normalisation; outputs per stride 8/16/32: class and objectness scores, box and five
//! landmarks (eyes, nose, mouth corners), all relative to the anchor cell.

use anyhow::{Context as _, Result, anyhow};
use ort::session::Session;
use ort::value::Tensor;

use crate::analysis::sharpness;
use crate::decode;

/// Stored with every value; bump when the detector or the eye measure changes.
pub const VERSION: i64 = 1;

static MODEL: &[u8] = include_bytes!("face_detection_yunet_2023mar.onnx");
const INPUT: u32 = 640;
const STRIDES: [u32; 3] = [8, 16, 32];
const SCORE_THRESHOLD: f32 = 0.6;
const NMS_IOU: f32 = 0.3;
/// Faces narrower than this (in the analysis image) are too small for a meaningful measure.
const MIN_FACE_WIDTH: f32 = 40.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Face {
    pub score: f32,
    /// x, y, width, height
    pub bbox: [f32; 4],
    /// Right eye, left eye, nose, right and left mouth corner.
    pub landmarks: [[f32; 2]; 5],
}

/// Raw network outputs of one stride.
pub struct StrideOutput<'a> {
    pub stride: u32,
    pub cls: &'a [f32],
    pub obj: &'a [f32],
    pub bbox: &'a [f32],
    pub kps: &'a [f32],
}

pub struct FaceDetector {
    session: Session,
}

impl FaceDetector {
    /// CPU on purpose: the model takes a few milliseconds, the GPU is busy with the big ones.
    pub fn load() -> Result<Self> {
        let session = Session::builder()
            .map_err(|e| anyhow!("{e}"))?
            .commit_from_memory(MODEL)
            .map_err(|e| anyhow!("{e}"))
            .context("cannot load the face detector")?;
        Ok(Self { session })
    }

    /// Faces in the coordinates of the given RGB8 image.
    pub fn detect(&mut self, rgb: &[u8], width: u32, height: u32) -> Result<Vec<Face>> {
        // Letterbox: scale the long side to 640, pad the rest with black.
        let scale = INPUT as f32 / width.max(height) as f32;
        let (w, h) = (
            ((width as f32 * scale).round() as u32).clamp(1, INPUT),
            ((height as f32 * scale).round() as u32).clamp(1, INPUT),
        );
        let small = decode::resize_rgb(rgb.to_vec(), width, height, w, h)?;
        let plane = (INPUT * INPUT) as usize;
        let mut input = vec![0.0f32; 3 * plane];
        for y in 0..h as usize {
            for x in 0..w as usize {
                let s = (y * w as usize + x) * 3;
                let d = y * INPUT as usize + x;
                // BGR order.
                input[d] = f32::from(small[s + 2]);
                input[plane + d] = f32::from(small[s + 1]);
                input[2 * plane + d] = f32::from(small[s]);
            }
        }
        let tensor = Tensor::from_array(([1usize, 3, INPUT as usize, INPUT as usize], input))
            .map_err(|e| anyhow!("{e}"))?;
        let outputs = self
            .session
            .run(ort::inputs!["input" => tensor])
            .map_err(|e| anyhow!("{e}"))?;
        let get = |name: String| -> Result<&[f32]> {
            Ok(outputs[name.as_str()]
                .try_extract_tensor::<f32>()
                .map_err(|e| anyhow!("{e}"))?
                .1)
        };
        let mut faces = Vec::new();
        for stride in STRIDES {
            faces.extend(decode_stride(
                &StrideOutput {
                    stride,
                    cls: get(format!("cls_{stride}"))?,
                    obj: get(format!("obj_{stride}"))?,
                    bbox: get(format!("bbox_{stride}"))?,
                    kps: get(format!("kps_{stride}"))?,
                },
                SCORE_THRESHOLD,
            ));
        }
        let back = 1.0 / scale;
        Ok(non_maximum_suppression(faces, NMS_IOU)
            .into_iter()
            .map(|f| Face {
                score: f.score,
                bbox: f.bbox.map(|v| v * back),
                landmarks: f.landmarks.map(|[x, y]| [x * back, y * back]),
            })
            .collect())
    }
}

/// Decodes one stride's anchors (OpenCV's FaceDetectorYN post-processing).
pub fn decode_stride(out: &StrideOutput<'_>, threshold: f32) -> Vec<Face> {
    let cols = (INPUT / out.stride) as usize;
    let s = out.stride as f32;
    let mut faces = Vec::new();
    for (idx, (&cls, &obj)) in out.cls.iter().zip(out.obj).enumerate() {
        let score = (cls.clamp(0.0, 1.0) * obj.clamp(0.0, 1.0)).sqrt();
        if score < threshold {
            continue;
        }
        let (row, col) = ((idx / cols) as f32, (idx % cols) as f32);
        let b = &out.bbox[idx * 4..idx * 4 + 4];
        let (cx, cy) = ((col + b[0]) * s, (row + b[1]) * s);
        let (w, h) = (b[2].exp() * s, b[3].exp() * s);
        let k = &out.kps[idx * 10..idx * 10 + 10];
        faces.push(Face {
            score,
            bbox: [cx - w / 2.0, cy - h / 2.0, w, h],
            landmarks: std::array::from_fn(|n| [(k[2 * n] + col) * s, (k[2 * n + 1] + row) * s]),
        });
    }
    faces
}

fn iou(a: &[f32; 4], b: &[f32; 4]) -> f32 {
    let x1 = a[0].max(b[0]);
    let y1 = a[1].max(b[1]);
    let x2 = (a[0] + a[2]).min(b[0] + b[2]);
    let y2 = (a[1] + a[3]).min(b[1] + b[3]);
    let inter = (x2 - x1).max(0.0) * (y2 - y1).max(0.0);
    let union = a[2] * a[3] + b[2] * b[3] - inter;
    if union <= 0.0 { 0.0 } else { inter / union }
}

pub fn non_maximum_suppression(mut faces: Vec<Face>, threshold: f32) -> Vec<Face> {
    faces.sort_by(|a, b| b.score.total_cmp(&a.score));
    let mut kept: Vec<Face> = Vec::new();
    for face in faces {
        if kept.iter().all(|k| iou(&k.bbox, &face.bbox) < threshold) {
            kept.push(face);
        }
    }
    kept
}

/// Laplacian variance around the eyes of the largest face (the sharper eye counts – the
/// other may be turned away or covered). `None` if there is no face big enough.
pub fn eye_sharpness(rgb: &[u8], width: u32, height: u32, faces: &[Face]) -> Option<f32> {
    let face = faces
        .iter()
        .filter(|f| f.bbox[2] >= MIN_FACE_WIDTH)
        .max_by(|a, b| (a.bbox[2] * a.bbox[3]).total_cmp(&(b.bbox[2] * b.bbox[3])))?;
    let half = (face.bbox[2] * 0.14).max(6.0);
    let luma = sharpness::luma(rgb);
    face.landmarks[..2]
        .iter()
        .filter_map(|&[x, y]| {
            let region = [
                (x - half).max(1.0) as usize,
                (y - half).max(1.0) as usize,
                ((x + half) as usize).min(width as usize - 1),
                ((y + half) as usize).min(height as usize - 1),
            ];
            sharpness::region_variance(&luma, width as usize, height as usize, region)
        })
        .reduce(f32::max)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Paints a face's box and its five points into an RGB image.
    fn mark(rgb: &mut [u8], width: u32, height: u32, face: &Face) {
        let mut dot = |x: f32, y: f32, colour: [u8; 3], radius: i32| {
            for dy in -radius..=radius {
                for dx in -radius..=radius {
                    let (px, py) = (x as i32 + dx, y as i32 + dy);
                    if px >= 0 && py >= 0 && (px as u32) < width && (py as u32) < height {
                        let at = ((py as u32 * width + px as u32) * 3) as usize;
                        rgb[at..at + 3].copy_from_slice(&colour);
                    }
                }
            }
        };
        let [x, y, w, h] = face.bbox;
        let steps = (2.0 * (w + h)) as i32;
        for i in 0..=steps {
            let t = i as f32 / steps.max(1) as f32;
            for (px, py) in [
                (x + t * w, y),
                (x + t * w, y + h),
                (x, y + t * h),
                (x + w, y + t * h),
            ] {
                dot(px, py, [40, 220, 90], 1);
            }
        }
        for (i, [lx, ly]) in face.landmarks.iter().enumerate() {
            let colour = if i < 2 { [240, 40, 40] } else { [60, 120, 255] };
            dot(*lx, *ly, colour, 3);
        }
    }

    /// Roadmap 6: the detector on real photos. `CERNO_FACES_CHECK=<n>` takes n photos the index
    /// saw faces on – half with one face, half with several – and n / 4 with none, from the live
    /// index opened **read-only**, detects again on the analysis decode and writes each photo with
    /// its boxes (green) and points (eyes red) into `CERNO_FACES_OUT`, a folder outside the
    /// photos. One line per photo on stdout (`--nocapture`). Writes nothing else.
    #[test]
    #[ignore]
    fn faces_on_real_photos() {
        use rusqlite::{Connection, OpenFlags};
        let Ok(count) = std::env::var("CERNO_FACES_CHECK") else {
            return;
        };
        let count: usize = count.parse().expect("CERNO_FACES_CHECK is a number");
        let out =
            std::path::PathBuf::from(std::env::var("CERNO_FACES_OUT").expect("CERNO_FACES_OUT"));
        std::fs::create_dir_all(&out).expect("output folder");
        let db = crate::paths::database_path().expect("index path");
        let conn = Connection::open_with_flags(&db, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .expect("index, read-only");
        let pick = |filter: &str, n: usize| -> Vec<(String, i64)> {
            let mut stmt = conn
                .prepare(&format!(
                    "SELECT MIN(f.path), i.faces FROM files f JOIN images i ON i.fingerprint = f.fingerprint
                     WHERE {filter} AND f.path NOT LIKE '%.originals%'
                     GROUP BY f.fingerprint ORDER BY random() LIMIT {}",
                    n * 3
                ))
                .expect("query");
            stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                .expect("rows")
                .filter_map(Result::ok)
                .filter(|(path, _): &(String, i64)| std::path::Path::new(path).is_file())
                .take(n)
                .collect()
        };
        let mut photos = pick("i.faces = 1", count / 2);
        photos.extend(pick("i.faces > 1", count - count / 2));
        photos.extend(pick("i.faces = 0", count / 4));
        let mut detector = FaceDetector::load().expect("detector");
        for (i, (path, stored)) in photos.iter().enumerate() {
            let path = std::path::Path::new(path);
            let Ok(bytes) = std::fs::read(path) else {
                continue;
            };
            let Some(format) = crate::library::format_of(path) else {
                continue;
            };
            let meta = crate::metadata::read_for(path, &bytes);
            let Ok(mut image) =
                decode::decode_for_display(&bytes, format, meta.orientation, [2048, 2048])
            else {
                println!("{path:?}: cannot decode");
                continue;
            };
            let (w, h) = (image.width, image.height);
            let found = detector.detect(&image.rgb, w, h).expect("detect");
            let eyes = eye_sharpness(&image.rgb, w, h, &found);
            let sizes: Vec<String> = found
                .iter()
                .map(|f| format!("{:.0}px/{:.2}", f.bbox[2], f.score))
                .collect();
            println!(
                "{i:02} stored {stored} found {} eyes {:?} [{}] {}",
                found.len(),
                eyes.map(|e| e.round()),
                sizes.join(" "),
                path.display()
            );
            for face in &found {
                mark(&mut image.rgb, w, h, face);
            }
            let jpeg = crate::edit::encode_jpeg(w, h, &image.rgb).expect("encode");
            std::fs::write(out.join(format!("{i:02}-{}f.jpg", found.len())), jpeg).expect("write");
        }
    }

    /// A single confident anchor in the stride-32 grid decodes to the expected box and eyes.
    #[test]
    fn decodes_one_anchor() {
        let cells = (INPUT / 32 * INPUT / 32) as usize;
        let idx = 3 * 20 + 5; // row 3, column 5
        let mut cls = vec![0.0; cells];
        let mut obj = vec![0.0; cells];
        let mut bbox = vec![0.0; cells * 4];
        let mut kps = vec![0.0; cells * 10];
        cls[idx] = 0.9;
        obj[idx] = 0.9;
        // Centre at the cell origin + 0.5 cell, size e^0 · 32 = 32 px.
        bbox[idx * 4..idx * 4 + 4].copy_from_slice(&[0.5, 0.5, 0.0, 0.0]);
        kps[idx * 10] = 0.25; // right eye x offset
        kps[idx * 10 + 1] = 0.25;
        let faces = decode_stride(
            &StrideOutput {
                stride: 32,
                cls: &cls,
                obj: &obj,
                bbox: &bbox,
                kps: &kps,
            },
            SCORE_THRESHOLD,
        );
        assert_eq!(faces.len(), 1);
        let face = faces[0];
        assert!((face.score - 0.9).abs() < 1e-6);
        assert_eq!(face.bbox, [160.0, 96.0, 32.0, 32.0]);
        assert_eq!(face.landmarks[0], [168.0, 104.0]);
    }

    #[test]
    fn suppresses_overlapping_boxes() {
        let face = |score, x| Face {
            score,
            bbox: [x, 0.0, 100.0, 100.0],
            landmarks: [[0.0; 2]; 5],
        };
        let kept = non_maximum_suppression(
            vec![face(0.7, 10.0), face(0.9, 0.0), face(0.8, 500.0)],
            NMS_IOU,
        );
        assert_eq!(kept.len(), 2);
        assert_eq!(kept[0].score, 0.9);
        assert_eq!(kept[1].score, 0.8);
    }

    #[test]
    fn detector_runs_and_finds_nothing_in_noise_free_grey() {
        let mut detector = FaceDetector::load().unwrap();
        let faces = detector
            .detect(&vec![128u8; 300 * 200 * 3], 300, 200)
            .unwrap();
        assert!(faces.is_empty());
    }

    #[test]
    fn eyes_measure_needs_a_big_enough_face() {
        let rgb = vec![128u8; 200 * 200 * 3];
        let small = Face {
            score: 0.9,
            bbox: [10.0, 10.0, 20.0, 20.0],
            landmarks: [[15.0, 15.0]; 5],
        };
        assert_eq!(eye_sharpness(&rgb, 200, 200, &[small]), None);
        let big = Face {
            bbox: [20.0, 20.0, 120.0, 120.0],
            landmarks: [
                [60.0, 70.0],
                [110.0, 70.0],
                [85.0, 100.0],
                [65.0, 120.0],
                [105.0, 120.0],
            ],
            ..small
        };
        assert_eq!(eye_sharpness(&rgb, 200, 200, &[big]), Some(0.0));
    }
}
