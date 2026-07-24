/// Parsed screenshot request size (v0.810).
/// - Any non-JSON content, or JSON without width/height: window-size capture
///   (the original v0.639 "any content" contract).
/// - Both `width` and `height` as positive integers: hi-res offscreen capture.
/// - Anything half-given or non-positive: Invalid, reported via done.json.
#[derive(Debug, PartialEq)]
pub(crate) enum ScreenshotSize {
    Window,
    Custom(u32, u32),
    Invalid(String),
}

pub(crate) fn parse_screenshot_request(text: &str) -> ScreenshotSize {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(text) else {
        return ScreenshotSize::Window;
    };
    match (v.get("width"), v.get("height")) {
        (None, None) => ScreenshotSize::Window,
        (Some(w), Some(h)) => {
            // as_u64 is None for negatives, floats, and strings -- all invalid.
            let (Some(w), Some(h)) = (w.as_u64(), h.as_u64()) else {
                return ScreenshotSize::Invalid(
                    "width and height must be positive integers".to_string(),
                );
            };
            if w == 0 || h == 0 {
                return ScreenshotSize::Invalid("width and height must be nonzero".to_string());
            }
            if w > u32::MAX as u64 || h > u32::MAX as u64 {
                return ScreenshotSize::Invalid("width/height out of range".to_string());
            }
            ScreenshotSize::Custom(w as u32, h as u32)
        }
        _ => ScreenshotSize::Invalid(
            "give BOTH width and height, or neither for a window-size capture".to_string(),
        ),
    }
}

/// Monotonic-per-session output path for a captured screenshot. Pulled out of
/// `poll_screenshot_request` so the naming is directly testable without a GPU.
pub(crate) fn screenshot_output_path(counter: u32) -> String {
    format!("debug/screenshot_{counter}.png")
}

/// The `debug/screenshot_done.json` body for a capture attempt: `{"ok":true,"path":...}` on
/// success (plus `width`/`height` when the capture was a sized offscreen render, v0.810),
/// `{"ok":false,"error":...}` on failure. Pulled out of `poll_screenshot_request` so
/// the shape is directly testable without a GPU.
pub(crate) fn screenshot_done_json(
    result: &Result<Option<(u32, u32)>, String>,
    path: &str,
) -> serde_json::Value {
    match result {
        Ok(None) => serde_json::json!({"ok": true, "path": path}),
        Ok(Some((w, h))) => {
            serde_json::json!({"ok": true, "path": path, "width": w, "height": h})
        }
        Err(e) => serde_json::json!({"ok": false, "error": e}),
    }
}

/// Resources + status shown in the planet-info tooltip, sourced from
/// `data/planets/tooltips.json` (infinite-of-X: the body list is data, not a
/// match arm). Parsed once and cached. Unlisted bodies fall back to
/// Unknown/Uncharted, exactly as the old hardcoded match did.
pub(crate) fn planet_tooltip_info(name: &str) -> (String, String) {
    use std::collections::HashMap;
    use std::sync::OnceLock;
    static TABLE: OnceLock<HashMap<String, (String, String)>> = OnceLock::new();
    let table = TABLE.get_or_init(|| {
        let mut m = HashMap::new();
        // Disk first (so an operator can edit it live), embedded fallback for a
        // distributed exe shipped without the data/ folder.
        let text = std::fs::read_to_string("data/planets/tooltips.json")
            .unwrap_or_else(|_| crate::embedded_data::PLANET_TOOLTIPS_JSON.to_string());
        {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(bodies) = v.get("bodies").and_then(|b| b.as_object()) {
                    for (body, info) in bodies {
                        let res = info
                            .get("resources")
                            .and_then(|r| r.as_str())
                            .unwrap_or("Unknown")
                            .to_string();
                        let status = info
                            .get("status")
                            .and_then(|s| s.as_str())
                            .unwrap_or("Uncharted")
                            .to_string();
                        m.insert(body.clone(), (res, status));
                    }
                }
            }
        }
        m
    });
    table
        .get(name)
        .cloned()
        .unwrap_or_else(|| ("Unknown".to_string(), "Uncharted".to_string()))
}

/// Parsed `notification_prefs_data` payload (v0.641). Pulled out of the WS-message match so
/// the parsing is directly testable: a missing/malformed `dm`/`mentions`/`tasks` field must
/// default to `true` (matching the server's own `notification_prefs` table column defaults
/// -- see `src/relay/storage/mod.rs`'s `CREATE TABLE`), never silently default to `false`
/// and mute someone who never asked to be muted.
pub(crate) struct NotifPrefsPayload {
    pub(crate) dm: bool,
    pub(crate) mentions: bool,
    pub(crate) tasks: bool,
    pub(crate) dnd_start: Option<String>,
    pub(crate) dnd_end: Option<String>,
}

pub(crate) fn parse_notification_prefs(val: &serde_json::Value) -> NotifPrefsPayload {
    NotifPrefsPayload {
        dm: val.get("dm").and_then(|v| v.as_bool()).unwrap_or(true),
        mentions: val.get("mentions").and_then(|v| v.as_bool()).unwrap_or(true),
        tasks: val.get("tasks").and_then(|v| v.as_bool()).unwrap_or(true),
        dnd_start: val.get("dnd_start").and_then(|v| v.as_str()).map(|s| s.to_string()),
        dnd_end: val.get("dnd_end").and_then(|v| v.as_str()).map(|s| s.to_string()),
    }
}

#[cfg(test)]
mod notification_prefs_tests {
    use super::parse_notification_prefs;

    #[test]
    fn real_payload_parses_every_field() {
        let v = serde_json::json!({
            "dm": false, "mentions": true, "tasks": false,
            "dnd_start": "22:00", "dnd_end": "07:00",
        });
        let p = parse_notification_prefs(&v);
        assert!(!p.dm);
        assert!(p.mentions);
        assert!(!p.tasks);
        assert_eq!(p.dnd_start.as_deref(), Some("22:00"));
        assert_eq!(p.dnd_end.as_deref(), Some("07:00"));
    }

    #[test]
    fn missing_dnd_is_none_not_a_default_string() {
        let v = serde_json::json!({"dm": true, "mentions": true, "tasks": true});
        let p = parse_notification_prefs(&v);
        assert_eq!(p.dnd_start, None);
        assert_eq!(p.dnd_end, None);
    }

    #[test]
    fn malformed_or_missing_bool_fields_default_to_true_not_false() {
        // Fail OPEN here specifically: a malformed payload must never silently mute
        // someone who never asked to be muted -- true (notified) is the safe default,
        // matching the server's own column defaults.
        let v = serde_json::json!({});
        let p = parse_notification_prefs(&v);
        assert!(p.dm);
        assert!(p.mentions);
        assert!(p.tasks);

        let wrong_type = serde_json::json!({"dm": "yes", "mentions": 1, "tasks": null});
        let p2 = parse_notification_prefs(&wrong_type);
        assert!(p2.dm);
        assert!(p2.mentions);
        assert!(p2.tasks);
    }
}

#[cfg(test)]
mod screenshot_command_tests {
    use super::{
        parse_screenshot_request, screenshot_done_json, screenshot_output_path,
        ScreenshotSize,
    };

    #[test]
    fn output_path_is_monotonic_and_collision_free() {
        assert_eq!(screenshot_output_path(1), "debug/screenshot_1.png");
        assert_eq!(screenshot_output_path(2), "debug/screenshot_2.png");
        assert_ne!(screenshot_output_path(1), screenshot_output_path(2));
    }

    #[test]
    fn done_json_success_shape_carries_the_real_path() {
        let v = screenshot_done_json(&Ok(None), "debug/screenshot_3.png");
        assert_eq!(v["ok"], true);
        assert_eq!(v["path"], "debug/screenshot_3.png");
        assert!(v.get("error").is_none(), "a success body must not carry an error key");
        assert!(
            v.get("width").is_none(),
            "a window-size capture must not claim explicit dimensions"
        );
    }

    #[test]
    fn done_json_sized_success_reports_the_actual_dimensions() {
        let v = screenshot_done_json(&Ok(Some((3840, 2160))), "debug/screenshot_5.png");
        assert_eq!(v["ok"], true);
        assert_eq!(v["path"], "debug/screenshot_5.png");
        assert_eq!(v["width"], 3840);
        assert_eq!(v["height"], 2160);
    }

    #[test]
    fn done_json_failure_shape_carries_the_real_error_not_a_path() {
        let err = "swapchain surface has no COPY_SRC usage on this backend -- frame capture unavailable".to_string();
        let v = screenshot_done_json(
            &Err::<Option<(u32, u32)>, _>(err.clone()),
            "debug/screenshot_4.png",
        );
        assert_eq!(v["ok"], false);
        assert_eq!(v["error"], err);
        assert!(v.get("path").is_none(), "a failure body must not carry a path key");
    }

    #[test]
    fn any_content_still_means_window_capture() {
        // The original v0.639 contract: dropping ANY file content triggers a
        // window-size capture. Non-JSON, empty, and sizeless JSON all qualify.
        assert_eq!(parse_screenshot_request(""), ScreenshotSize::Window);
        assert_eq!(parse_screenshot_request("go"), ScreenshotSize::Window);
        assert_eq!(parse_screenshot_request("{}"), ScreenshotSize::Window);
        assert_eq!(
            parse_screenshot_request(r#"{"note":"hello"}"#),
            ScreenshotSize::Window
        );
    }

    #[test]
    fn width_and_height_request_a_custom_capture() {
        assert_eq!(
            parse_screenshot_request(r#"{"width":3840,"height":2160}"#),
            ScreenshotSize::Custom(3840, 2160)
        );
        assert_eq!(
            parse_screenshot_request(r#"{"width":7680,"height":4320}"#),
            ScreenshotSize::Custom(7680, 4320)
        );
    }

    #[test]
    fn malformed_sizes_are_rejected_not_silently_windowed() {
        // Half-given, zero, negative, and non-integer sizes must produce a
        // CLEAR error (Invalid -> ok:false in done.json), never a silent
        // fallback that hands back the wrong resolution.
        for bad in [
            r#"{"width":3840}"#,
            r#"{"height":2160}"#,
            r#"{"width":0,"height":2160}"#,
            r#"{"width":3840,"height":0}"#,
            r#"{"width":-1,"height":2160}"#,
            r#"{"width":1.5,"height":2160}"#,
            r#"{"width":"4k","height":"uhd"}"#,
        ] {
            match parse_screenshot_request(bad) {
                ScreenshotSize::Invalid(_) => {}
                other => panic!("{bad} parsed as {other:?}, expected Invalid"),
            }
        }
    }
}
