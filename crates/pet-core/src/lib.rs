pub mod animation;
pub mod assets;
pub mod audio_contract;
pub mod clock;
pub mod expressions;
pub mod manifest;
pub mod settings;
pub mod state;
pub mod storage;
pub mod timer;

pub use animation::{locked_beat_nod_m1, AnimationPose, CharacterAnimator, NoteParticleRender};
pub use assets::{
    load_character_assets, load_character_assets_from_dir, load_embedded_character_assets,
    AnimatedSprite, LoadedCharacterAssets, TightSprite,
};
pub use audio_contract::{
    clamp_sensitivity, is_supported_audio_file, AudioCommand, AudioDeviceInfo, AudioMode,
    AudioTelemetry, DEFAULT_DEMO_BPM, DEFAULT_SENSITIVITY, MAX_SENSITIVITY, MIN_SENSITIVITY,
};
pub use clock::{monotonic_ms, wall_ms, FastRng};
pub use expressions::{
    legacy_expressions, parse_expressions, ExpressionDefinition, ExpressionResources,
    ExpressionType,
};
pub use manifest::CharacterManifest;
pub use settings::{
    ActivityMode, EventContextKey, EventSwitchMatrix, GlobalSettings, PersistedState,
    RandomEventType, TodoItem, TodoList,
};
pub use state::{AppAction, AppEffect, AppModal, AppState, AssetStatus, MainTab, ResolvedFaceLayers};
pub use storage::{load_persisted_state, resolve_data_dir, save_persisted_state};
pub use timer::{format_mm_ss, timer_value, StudyTimer, TimerMode, TimerState};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_assets_load_and_crop_dramatically() {
        let loaded = load_embedded_character_assets().expect("embedded character assets must load");
        assert_eq!(loaded.manifest.canvas_width, 800);
        assert_eq!(loaded.manifest.canvas_height, 800);
        assert_eq!(loaded.expressions.len(), 7);
        assert!(
            loaded.total_decoded_bytes < loaded.full_canvas_equivalent_bytes / 2,
            "tight-bbox cropping should cut decoded memory by >50%: {} vs {}",
            loaded.total_decoded_bytes,
            loaded.full_canvas_equivalent_bytes
        );
    }

    #[test]
    fn expressions_validator_rejects_traversal_and_missing_neutral() {
        let bad_path = r#"{"version":1,"expressions":[{"id":"neutral","name":"自然","type":"base","resources":{"eyes":"../secret.png","mouth":"mouth.png"},"durationMs":3000,"random":false}]}"#;
        assert!(parse_expressions(bad_path).is_err());

        let missing_neutral = r#"{"version":1,"expressions":[{"id":"happy","name":"开心","type":"base","resources":{"eyes":"eyes_happy.png","mouth":"mouth_happy.png"},"durationMs":3000,"random":true}]}"#;
        assert!(parse_expressions(missing_neutral).is_err());
    }

    #[test]
    fn bidirectional_timer_and_pause_resume() {
        let mut t = StudyTimer::new(1);
        assert_eq!(t.format_display(), "01:00");
        t.start_or_resume(10_000);
        assert!(!t.tick(40_000));
        assert_eq!(t.remaining_ms, 30_000);
        assert!(!t.pause(45_000));
        assert_eq!(t.state, TimerState::Paused);
        assert_eq!(t.remaining_ms, 25_000);
        assert!(!t.set_mode(TimerMode::Stopwatch));
        t.start_or_resume(100_000);
        assert!(t.tick(125_000));
        assert_eq!(t.state, TimerState::Completed);

        t.reset();
        assert!(t.set_mode(TimerMode::Stopwatch));
        t.start_or_resume(5_000);
        assert!(!t.tick(12_500));
        assert_eq!(t.remaining_ms, 7_500);
        assert_eq!(t.format_display(), "00:07");
    }

    #[test]
    fn music_doze_strictly_locked_off() {
        let mut matrix = EventSwitchMatrix::default();
        assert!(!matrix.toggle(EventContextKey::Music, RandomEventType::Doze));
        assert!(!matrix.is_event_allowed(RandomEventType::Doze, ActivityMode::Chill, true));
        assert!(matrix.is_event_allowed(RandomEventType::Doze, ActivityMode::Chill, false));
    }

    #[test]
    fn m1_locked_beat_nod_matches_peak_at_beat() {
        let peak = locked_beat_nod_m1(1000.0, Some(1000.0), Some(500.0), 1.5).unwrap();
        assert!((peak - 1.5 * 3.4).abs() < 1e-4);
        let far = locked_beat_nod_m1(1250.0, Some(1000.0), Some(500.0), 1.5).unwrap();
        assert_eq!(far, 0.0);
        assert!(locked_beat_nod_m1(1000.0, Some(1000.0), Some(100.0), 1.5).is_none());
    }

    #[test]
    fn select_expression_toggles_back_to_neutral_and_accessory_keeps_base() {
        let loaded = load_embedded_character_assets().unwrap();
        let mut app = AppState::new(PersistedState::default());

        app.dispatch(AppAction::SelectExpression("happy".into()), &loaded.expressions, 0.0, 0);
        assert_eq!(app.selected_expression_id, "happy");
        assert_eq!(app.base_expression_id, "happy");

        app.dispatch(AppAction::SelectExpression("think".into()), &loaded.expressions, 100.0, 100);
        let layers = app.resolve_face_layers(&loaded.manifest, &loaded.expressions, false);
        assert_eq!(layers.effective_expression_id, "think");
        assert_eq!(layers.overlay.as_deref(), Some("fx_think_bubble.png"));
        assert_eq!(app.base_expression_id, "happy");

        // Clicking current expression restores neutral
        app.dispatch(AppAction::SelectExpression("think".into()), &loaded.expressions, 200.0, 200);
        assert_eq!(app.selected_expression_id, "neutral");
    }
}
