use super::*;

#[test]
fn xmp_rating_attribute_and_element() {
    assert_eq!(
        parse_xmp_rating(r#"<rdf:Description xmp:Rating="4" xmp:Label="x"/>"#),
        Some(4)
    );
    assert_eq!(parse_xmp_rating("<a xmp:Rating = '2'/>"), Some(2));
    assert_eq!(
        parse_xmp_rating("<xmp:Rating>5</xmp:Rating><xmp:Label>a</xmp:Label>"),
        Some(5)
    );
    assert_eq!(parse_xmp_rating(r#"<a xap:Rating="3.0"/>"#), Some(3));
    assert_eq!(parse_xmp_rating(r#"<a xmp:Rating="-1"/>"#), Some(-1));
    assert_eq!(parse_xmp_rating(r#"<a MicrosoftPhoto:Rating="75"/>"#), None);
}

#[test]
fn packet_found_in_binary_noise() {
    let mut bytes = vec![0xFF, 0xD8, 0x00, 0x13];
    bytes.extend_from_slice(
            br#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:Description xmp:Rating="3" MicrosoftPhoto:Rating="50"/></x:xmpmeta>"#,
        );
    bytes.extend_from_slice(&[0x00, 0xFF, 0xD9]);
    let meta = read(&bytes);
    assert_eq!(meta.rating.value, Rating::Stars(3));
    assert!(meta.rating.has_ms_photo_rating);
    assert!(!meta.rating.has_exif_rating);
    assert_eq!(meta.orientation, 1);
}

fn exif_from(fields: &[exif::Field]) -> Exif {
    let mut writer = exif::experimental::Writer::new();
    for field in fields {
        writer.push_field(field);
    }
    let mut tiff = Cursor::new(Vec::new());
    writer.write(&mut tiff, false).unwrap();
    exif::Reader::new().read_raw(tiff.into_inner()).unwrap()
}

fn field(tag: Tag, value: Value) -> exif::Field {
    exif::Field {
        tag,
        ifd_num: In::PRIMARY,
        value,
    }
}

fn rational(num: u32, denom: u32) -> Value {
    Value::Rational(vec![exif::Rational { num, denom }])
}

#[test]
fn camera_info_from_exif() {
    let exif = exif_from(&[
        field(Tag::Make, Value::Ascii(vec![b"NIKON CORPORATION".to_vec()])),
        field(Tag::Model, Value::Ascii(vec![b"NIKON Z 6_2".to_vec()])),
        field(
            Tag::LensModel,
            Value::Ascii(vec![b"NIKKOR Z 24-70mm f/4 S".to_vec()]),
        ),
        field(Tag::FocalLength, rational(35, 1)),
        field(Tag::FNumber, rational(28, 10)),
        field(Tag::ExposureTime, rational(1, 250)),
        field(Tag::PhotographicSensitivity, Value::Short(vec![400])),
        field(
            Tag::DateTimeOriginal,
            Value::Ascii(vec![b"2026:09:12 14:03:22".to_vec()]),
        ),
    ]);
    let info = camera_info(&exif);
    assert_eq!(info.camera.as_deref(), Some("NIKON Z 6_2"));
    assert_eq!(info.taken.as_deref(), Some("2026-09-12 14:03"));
    assert_eq!(
        info.exposure_line(),
        "35 mm  ·  f/2.8  ·  1/250 s  ·  ISO 400"
    );
    assert_eq!(info.gear_line(), "NIKON Z 6_2  ·  NIKKOR Z 24-70mm f/4 S");
    assert_eq!(info.model.as_deref(), Some("NIKON Z 6_2"));
}

/// A drone writes "DJI" + "FC7503", but some of its files only the model: still one camera.
#[test]
fn the_model_tells_cameras_apart() {
    let with_make = camera_info(&exif_from(&[
        field(Tag::Make, Value::Ascii(vec![b"DJI".to_vec()])),
        field(Tag::Model, Value::Ascii(vec![b"FC7503".to_vec()])),
    ]));
    let model_only = camera_info(&exif_from(&[field(
        Tag::Model,
        Value::Ascii(vec![b"FC7503".to_vec()]),
    )]));
    assert_eq!(with_make.camera.as_deref(), Some("DJI FC7503"));
    let id = |info: &CameraInfo| info.model.as_deref().map(camera_id);
    assert_eq!(id(&with_make), id(&model_only));
    assert_eq!(camera_id("Pixel 7a"), camera_id(" pixel 7A "));
    assert_ne!(camera_id("Pixel 7a"), camera_id("moto g42"));
    // Without a model the make stands in.
    let make_only = camera_info(&exif_from(&[field(
        Tag::Make,
        Value::Ascii(vec![b"Ricoh".to_vec()]),
    )]));
    assert_eq!(make_only.model.as_deref(), Some("Ricoh"));
}

fn rationals(values: &[(u32, u32)]) -> Value {
    Value::Rational(
        values
            .iter()
            .map(|&(num, denom)| exif::Rational { num, denom })
            .collect(),
    )
}

#[test]
fn gps_position_and_digital_zoom() {
    let ascii = |s: &str| Value::Ascii(vec![s.as_bytes().to_vec()]);
    // 48° 31' 17.76" N, 9° 3' 27.36" W
    let exif = exif_from(&[
        field(Tag::GPSLatitudeRef, ascii("N")),
        field(
            Tag::GPSLatitude,
            rationals(&[(48, 1), (31, 1), (1776, 100)]),
        ),
        field(Tag::GPSLongitudeRef, ascii("W")),
        field(Tag::GPSLongitude, rationals(&[(9, 1), (3, 1), (2736, 100)])),
        field(Tag::DigitalZoomRatio, rational(2, 1)),
    ]);
    let info = camera_info(&exif);
    let (lat, lon) = info.gps.unwrap();
    assert!((lat - 48.5216).abs() < 1e-6, "{lat}");
    assert!((lon + 9.0576).abs() < 1e-6, "{lon}");
    assert_eq!(info.digital_zoom, Some(2.0));
    assert_eq!(
        maps_url((lat, lon)),
        "https://www.google.com/maps/search/?api=1&query=48.521600,-9.057600"
    );
    assert_eq!(
        osm_url((lat, lon)),
        "https://www.openstreetmap.org/?mlat=48.521600&mlon=-9.057600\
             #map=16/48.521600/-9.057600"
    );

    // No fix, a broken rational and "no digital zoom" (1/1 or 0/0) give nothing.
    let exif = exif_from(&[
        field(Tag::GPSLatitude, rationals(&[(0, 1), (0, 1), (0, 1)])),
        field(Tag::GPSLongitude, rationals(&[(0, 1), (0, 1), (0, 1)])),
        field(Tag::DigitalZoomRatio, rational(1, 1)),
    ]);
    let info = camera_info(&exif);
    assert_eq!((info.gps, info.digital_zoom), (None, None));
    let exif = exif_from(&[
        field(Tag::GPSLatitude, rationals(&[(48, 0)])),
        field(Tag::GPSLongitude, rationals(&[(9, 1)])),
        field(Tag::DigitalZoomRatio, rational(0, 0)),
    ]);
    let info = camera_info(&exif);
    assert_eq!((info.gps, info.digital_zoom), (None, None));
}

#[test]
fn exposure_formatting() {
    assert_eq!(format_exposure(1.0 / 8000.0), "1/8000 s");
    assert_eq!(format_exposure(0.3), "0.3 s");
    assert_eq!(format_exposure(2.0), "2 s");
    let info = CameraInfo {
        camera: Some("Sony ILCE-7M4".into()),
        focal_mm: Some(4.25),
        focal_35mm: Some(26),
        f_number: Some(1.8),
        ..CameraInfo::default()
    };
    assert_eq!(info.exposure_line(), "4 mm (26 mm eq.)  ·  f/1.8");
    assert_eq!(info.gear_line(), "Sony ILCE-7M4");
    assert_eq!(CameraInfo::default().exposure_line(), "");
}

#[test]
fn xmp_comment_and_keywords() {
    let xmp = r#"<x:xmpmeta><rdf:Description>
            <dc:subjectCode>not the keywords</dc:subjectCode>
            <dc:description><rdf:Alt>
              <rdf:li xml:lang="de">Hallo</rdf:li>
              <rdf:li xml:lang="x-default">Erste Zeile&#xA;Zweite &amp; mehr</rdf:li>
            </rdf:Alt></dc:description>
            <dc:subject><rdf:Bag>
              <rdf:li>Straße</rdf:li><rdf:li> Äpfel </rdf:li><rdf:li></rdf:li><rdf:li/>
            </rdf:Bag></dc:subject>
            </rdf:Description></x:xmpmeta>"#;
    let d = read_description(b"", Some(xmp));
    assert_eq!(d.comment, "Erste Zeile\nZweite & mehr");
    assert_eq!(d.keywords, ["Straße", "Äpfel"]);
    // Only one language: that one.
    let single = r#"<dc:description><rdf:Alt><rdf:li xml:lang="en">Only</rdf:li></rdf:Alt></dc:description>"#;
    assert_eq!(read_description(b"", Some(single)).comment, "Only");
    assert_eq!(read_description(b"", None), Description::default());
    assert_eq!(
        xml_unescape("a &lt;b&gt; &#252; &unknown; & c"),
        "a <b> ü &unknown; & c"
    );
}

/// A Photoshop APP13 block with IPTC datasets, as old cameras and programs write it.
fn app13(datasets: &[(u8, u8, &[u8])]) -> Vec<u8> {
    let mut iim = Vec::new();
    for (record, dataset, value) in datasets {
        iim.extend([0x1C, *record, *dataset]);
        iim.extend((value.len() as u16).to_be_bytes());
        iim.extend(*value);
    }
    let mut out = b"\xFF\xD8\xFF\xED\0\0Photoshop 3.0\0".to_vec();
    // An unrelated resource first, with an odd size (padded) and a one-letter name.
    out.extend(b"8BIM\x04\x0C\x01x");
    out.extend(3u32.to_be_bytes());
    out.extend(b"abc\0");
    out.extend(b"8BIM\x04\x04\0\0");
    out.extend((iim.len() as u32).to_be_bytes());
    out.extend(iim);
    out
}

#[test]
fn iptc_iim_in_utf8_and_latin1() {
    let utf8 = app13(&[
        (1, 90, b"\x1B%G"),
        (2, 25, "Straße".as_bytes()),
        (2, 25, "Äpfel".as_bytes()),
        (2, 120, "Grüße aus Köln".as_bytes()),
    ]);
    let d = read_description(&utf8, None);
    assert_eq!(d.keywords, ["Straße", "Äpfel"]);
    assert_eq!(d.comment, "Grüße aus Köln");

    let latin1 = app13(&[(2, 25, b"Stra\xDFe"), (2, 120, b"Gr\xFC\xDFe")]);
    let d = read_description(&latin1, None);
    assert_eq!(d.keywords, ["Straße"]);
    assert_eq!(d.comment, "Grüße");

    // XMP wins where it has a value; IPTC fills the rest.
    let xmp = "<dc:subject><rdf:Bag><rdf:li>neu</rdf:li></rdf:Bag></dc:subject>";
    let d = read_description(&utf8, Some(xmp));
    assert_eq!(d.keywords, ["neu"]);
    assert_eq!(d.comment, "Grüße aus Köln");
}

#[test]
fn keywords_are_added_once() {
    let mut d = Description::default();
    assert!(d.add_keyword(" Urlaub "));
    assert!(!d.add_keyword("urlaub"));
    assert!(!d.add_keyword("  "));
    assert!(d.add_keyword("Strand"));
    assert_eq!(d.keywords, ["Urlaub", "Strand"]);
}

#[test]
fn labels_round_trip_including_localised_names() {
    assert_eq!(LabelInfo::parse("Red"), LabelInfo::Known(Label::Red));
    assert_eq!(LabelInfo::parse("  gelb "), LabelInfo::Known(Label::Yellow));
    assert_eq!(LabelInfo::parse("Grün"), LabelInfo::Known(Label::Green));
    assert_eq!(LabelInfo::parse("Violet"), LabelInfo::Known(Label::Purple));
    assert_eq!(LabelInfo::parse("Púrpura"), LabelInfo::Known(Label::Purple));
    assert_eq!(LabelInfo::parse(""), LabelInfo::None);
    assert_eq!(LabelInfo::parse("Selects"), LabelInfo::Other);
    for label in Label::ALL {
        assert_eq!(Label::from_id(label.id()), Some(label));
        assert_eq!(LabelInfo::parse(label.xmp_name()), LabelInfo::Known(label));
    }
    let meta = read(br#"<x:xmpmeta><a xmp:Label="Rouge" xmp:Rating="2"/></x:xmpmeta>"#);
    assert_eq!(meta.label, LabelInfo::Known(Label::Red));
    assert_eq!(meta.rating.value, Rating::Stars(2));
    let meta = read(br#"<x:xmpmeta><xmp:Label>Blue</xmp:Label></x:xmpmeta>"#);
    assert_eq!(meta.label, LabelInfo::Known(Label::Blue));
    let meta = read(br#"<x:xmpmeta><a xmp:Label="Kundenauswahl"/></x:xmpmeta>"#);
    assert_eq!(meta.label, LabelInfo::Other);
}

#[test]
fn capture_time_uses_subseconds_and_falls_back() {
    assert_eq!(days_from_civil(1970, 1, 1), Some(0));
    assert_eq!(days_from_civil(2000, 1, 1), Some(10_957));
    assert_eq!(days_from_civil(2024, 2, 29), Some(19_782));
    assert_eq!(days_from_civil(2023, 2, 29), None);
    assert_eq!(
        capture_millis(Some("1970:01:01 00:00:00".into()), None),
        Some(0)
    );
    assert_eq!(
        capture_millis(Some("1970:01:01 00:00:01".into()), Some("42".into())),
        Some(1_420)
    );
    assert_eq!(
        capture_millis(Some("1970:01:01 00:00:01".into()), Some("4".into())),
        Some(1_400)
    );
    assert_eq!(
        capture_millis(Some("0000:00:00 00:00:00".into()), None),
        None
    );

    let exif = exif_from(&[
        field(
            Tag::DateTimeOriginal,
            Value::Ascii(vec![b"2026:09:12 14:03:22".to_vec()]),
        ),
        field(Tag::SubSecTimeOriginal, Value::Ascii(vec![b"5".to_vec()])),
    ]);
    let info = camera_info(&exif);
    let again = capture_millis(Some("2026:09:12 14:03:22".into()), Some("5".into()));
    assert_eq!(info.taken_ms, again);
    assert!(info.taken_ms.unwrap() % 1000 == 500);

    let exif = exif_from(&[field(
        Tag::DateTimeDigitized,
        Value::Ascii(vec![b"2026:09:12 14:03:22".to_vec()]),
    )]);
    assert_eq!(
        camera_info(&exif).taken_ms,
        capture_millis(Some("2026:09:12 14:03:22".into()), None)
    );
}

#[test]
fn rejected_and_unrated() {
    let meta = read(br#"<x:xmpmeta><a xmp:Rating="-1"/></x:xmpmeta>"#);
    assert_eq!(meta.rating.value, Rating::Rejected);
    assert_eq!(meta.rating.value.stars(), None);
    let meta = read(br#"<x:xmpmeta><a xmp:Rating="0"/></x:xmpmeta>"#);
    assert_eq!(meta.rating.value, Rating::Unrated);
    assert_eq!(read(b"no metadata").rating.value, Rating::Unrated);
    for rating in [Rating::Unrated, Rating::Rejected, Rating::Stars(4)] {
        assert_eq!(
            rating.value().map_or(Rating::Unrated, Rating::from_value),
            rating
        );
    }
}

/// `format_millis` reads back what `capture_millis` made, across a leap day, a year's end
/// and before 1970.
#[test]
fn millis_format_back_to_the_capture_time() {
    for date in [
        "2026:08:24 14:04:59",
        "2024:02:29 00:00:00",
        "1999:12:31 23:59:30",
        "1969:07:20 20:17:40",
    ] {
        let ms = capture_millis(Some(date.into()), None).expect(date);
        let expected = format!(
            "{}-{}-{} {}",
            &date[0..4],
            &date[5..7],
            &date[8..10],
            &date[11..16]
        );
        assert_eq!(format_millis(ms), expected);
    }
    let ms = capture_millis(Some("2026:01:01 00:30:00".into()), None).unwrap();
    assert_eq!(format_millis(ms - 3_600_000), "2025-12-31 23:30");
}
