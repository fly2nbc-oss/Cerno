//! The camera filter: one camera of the folder – a model, or one body of it – and the list the
//! filter bar's camera box offers.

use super::*;

/// The camera a view is narrowed to. `model` is `metadata::camera_id` of the model (as series
/// and camera offsets count cameras); `serial`, when set, also the body's serial number – for
/// two bodies of one model. Without it every body of the model counts, also photos whose
/// serial number is unknown (not read yet, or never written).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CameraPick {
    pub model: u64,
    pub serial: Option<u64>,
}

impl CameraPick {
    /// Whether a photo of this camera model and body stays visible. Videos and photos without
    /// camera data never do.
    pub fn accepts(self, camera: Option<u64>, serial: Option<u64>) -> bool {
        camera == Some(self.model) && self.serial.is_none_or(|wanted| serial == Some(wanted))
    }
}

/// One entry of the camera box: a model with all its photos, or one body of a model that came
/// with two or more (by their serial numbers).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CameraEntry {
    pub pick: CameraPick,
    /// The model as the file names it (`ScoreBoard::camera_name`, like the camera time card).
    pub name: String,
    /// The end of the body's serial number that tells it apart from the model's other bodies.
    pub suffix: Option<String>,
    pub count: usize,
}

impl CameraEntry {
    /// `ILCE-7M4`, or `ILCE-7M4 ·4821` for one body.
    pub fn label(&self) -> String {
        match &self.suffix {
            Some(suffix) => format!("{} ·{suffix}", self.name),
            None => self.name.clone(),
        }
    }
}

/// The digits a serial number keeps in a label: its last four, more when two bodies end alike.
const SUFFIX: usize = 4;

/// The folder's cameras, the most photos first: one entry per model, and below a model with two
/// or more bodies (serial numbers) one entry per body. `photos` gives each photo's model and
/// serial id (`metadata::camera_id`); `name` and `serial_number` turn them back into text.
/// Photos without a camera (videos, screenshots) are in no entry.
pub fn camera_entries(
    photos: impl IntoIterator<Item = (Option<u64>, Option<u64>)>,
    name: impl Fn(u64) -> Option<String>,
    serial_number: impl Fn(u64) -> Option<String>,
) -> Vec<CameraEntry> {
    let mut models: HashMap<u64, (usize, HashMap<u64, usize>)> = HashMap::new();
    for (camera, serial) in photos {
        let Some(model) = camera else {
            continue;
        };
        let (count, bodies) = models.entry(model).or_default();
        *count += 1;
        if let Some(serial) = serial {
            *bodies.entry(serial).or_default() += 1;
        }
    }
    let mut groups: Vec<(CameraEntry, Vec<CameraEntry>)> = models
        .into_iter()
        .map(|(model, (count, bodies))| {
            let name = name(model).unwrap_or_default();
            let entry = CameraEntry {
                pick: CameraPick {
                    model,
                    serial: None,
                },
                name: name.clone(),
                suffix: None,
                count,
            };
            let mut split = Vec::new();
            if bodies.len() > 1 {
                let numbers: Vec<(u64, String, usize)> = bodies
                    .into_iter()
                    .map(|(id, count)| (id, serial_number(id).unwrap_or_default(), count))
                    .collect();
                let keep = suffix_length(numbers.iter().map(|(_, number, _)| number.as_str()));
                split = numbers
                    .into_iter()
                    .map(|(id, number, count)| CameraEntry {
                        pick: CameraPick {
                            model,
                            serial: Some(id),
                        },
                        name: name.clone(),
                        suffix: Some(last_chars(&number, keep)),
                        count,
                    })
                    .collect();
                split.sort_by(most_first);
            }
            (entry, split)
        })
        .collect();
    groups.sort_by(|(a, _), (b, _)| most_first(a, b));
    groups
        .into_iter()
        .flat_map(|(entry, bodies)| std::iter::once(entry).chain(bodies))
        .collect()
}

fn most_first(a: &CameraEntry, b: &CameraEntry) -> std::cmp::Ordering {
    b.count
        .cmp(&a.count)
        .then_with(|| a.name.cmp(&b.name))
        .then_with(|| a.suffix.cmp(&b.suffix))
}

/// How many trailing characters tell these serial numbers apart: `SUFFIX`, or more when two
/// end alike (at most the longest number).
fn suffix_length<'a>(numbers: impl Iterator<Item = &'a str> + Clone) -> usize {
    let longest = numbers
        .clone()
        .map(|n| n.chars().count())
        .max()
        .unwrap_or(0);
    (SUFFIX..=longest.max(SUFFIX))
        .find(|&keep| {
            let ends: HashSet<String> = numbers.clone().map(|n| last_chars(n, keep)).collect();
            ends.len() == numbers.clone().count()
        })
        .unwrap_or(longest.max(SUFFIX))
}

fn last_chars(text: &str, keep: usize) -> String {
    let count = text.chars().count();
    text.chars().skip(count.saturating_sub(keep)).collect()
}
