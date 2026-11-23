//! 表情编辑器与条件规则场景测试。

use crate::clock::{Ctx, SplitMix64, WallMs};
use crate::command::{CharacterCommand, Command, EditorCommand, StudyCommand};
use crate::effect::Effect;
use crate::model::editor::{AssetChoice, AssetSlot, EditorError};
use crate::model::events::{ActivityMode, EventContext, RandomEvent};
use crate::model::expr_rules::Conditions;
use crate::model::expression::{
    ExpressionAssets, ExpressionKind, ExpressionMeta, NEUTRAL, Situation,
};
use crate::persist::Persisted;
use crate::state::AppState;

struct World {
    state: AppState,
    now: WallMs,
    rng: SplitMix64,
}

impl World {
    fn new() -> Self {
        let mut w = Self {
            state: AppState::default(),
            now: WallMs(1_000_000),
            rng: SplitMix64::new(11),
        };
        w.send(Command::Character(CharacterCommand::Loaded {
            id: "sakura".into(),
            name: "樱音".into(),
            expressions: sample_raw(),
            canvas: (800, 800),
            custom_warning: None,
        }));
        w
    }

    fn send(&mut self, cmd: Command) -> Vec<Effect> {
        let mut ctx = Ctx::new(self.now, &mut self.rng);
        super::update(&mut self.state, cmd, &mut ctx)
    }

    fn ed(&mut self, cmd: EditorCommand) -> Vec<Effect> {
        self.send(Command::Editor(cmd))
    }
}

fn sample_raw() -> Vec<ExpressionMeta> {
    vec![
        ExpressionMeta {
            id: NEUTRAL.into(),
            name: "自然".into(),
            kind: ExpressionKind::Base,
            duration_ms: 3200,
            random: false,
            assets: ExpressionAssets {
                eyes: Some("eyes_neutral.png".into()),
                mouth: Some("mouth_neutral.png".into()),
                overlay: None,
            },
            ..Default::default()
        },
        ExpressionMeta {
            id: "think".into(),
            name: "思考".into(),
            kind: ExpressionKind::Accessory,
            event: Some(RandomEvent::Think),
            duration_ms: 3200,
            random: true,
            ..Default::default()
        },
        ExpressionMeta {
            id: "doze".into(),
            name: "打瞌睡".into(),
            kind: ExpressionKind::Accessory,
            event: Some(RandomEvent::Doze),
            duration_ms: 3200,
            random: true,
            ..Default::default()
        },
    ]
}

#[test]
fn new_base_requires_eyes_and_mouth() {
    let mut w = World::new();
    w.ed(EditorCommand::NewDraft);
    w.ed(EditorCommand::SetKind(ExpressionKind::Base));
    w.ed(EditorCommand::SetName("委屈".into()));
    w.ed(EditorCommand::AssetPicked {
        slot: AssetSlot::Eyes,
        path: "C:/eyes.png".into(),
    });
    let fx = w.ed(EditorCommand::Save);
    assert!(fx.is_empty());
    let err = w
        .state
        .editor()
        .draft
        .as_ref()
        .and_then(|d| d.error.as_ref());
    assert_eq!(err, Some(&EditorError::MissingFace));
}

#[test]
fn new_accessory_with_only_overlay_saves_as_custom_1() {
    let mut w = World::new();
    w.ed(EditorCommand::NewDraft);
    w.ed(EditorCommand::SetKind(ExpressionKind::Accessory));
    w.ed(EditorCommand::SetName("星星眼".into()));
    w.ed(EditorCommand::AssetPicked {
        slot: AssetSlot::Overlay,
        path: "C:/stars.png".into(),
    });
    let fx = w.ed(EditorCommand::Save);
    assert!(w.state.editor().saving);
    assert_eq!(fx.len(), 1);
    let Some(Effect::SaveCustomExpression(spec)) = fx.first() else {
        assert_eq!(fx.len(), usize::MAX, "应触发 SaveCustomExpression");
        return;
    };
    assert_eq!(spec.id, "custom-1");
    assert_eq!(spec.name, "星星眼");
    assert_eq!(
        spec.overlay,
        Some(AssetChoice::Import("C:/stars.png".into()))
    );

    w.ed(EditorCommand::Saved);
    assert!(!w.state.editor().saving);
    assert!(w.state.editor().draft.is_none());
}

#[test]
fn duplicate_name_is_rejected() {
    let mut w = World::new();
    w.ed(EditorCommand::NewDraft);
    w.ed(EditorCommand::SetKind(ExpressionKind::Accessory));
    w.ed(EditorCommand::SetName("思考".into()));
    w.ed(EditorCommand::AssetPicked {
        slot: AssetSlot::Overlay,
        path: "C:/a.png".into(),
    });
    w.ed(EditorCommand::Save);
    let err = w
        .state
        .editor()
        .draft
        .as_ref()
        .and_then(|d| d.error.as_ref());
    assert_eq!(err, Some(&EditorError::NameDuplicate));
}

#[test]
fn builtin_rule_edit_updates_catalog_and_locks_music_doze() {
    let mut w = World::new();
    w.ed(EditorCommand::EditExisting("think".into()));
    w.ed(EditorCommand::SetCondition {
        context: EventContext::Study,
        on: false,
    });
    let fx = w.ed(EditorCommand::Save);
    assert!(fx.is_empty());
    assert!(w.state.editor().draft.is_none());

    w.send(Command::Study(StudyCommand::SelectActivity(
        ActivityMode::Study,
    )));
    let study_pool = w.state.expressions().random_pool(&w.state.situation());
    assert!(study_pool.iter().all(|m| m.id != "think"));

    // 打瞌睡的听歌开关保持锁定关闭，ResetRule 恢复默认条件。
    w.ed(EditorCommand::EditExisting("doze".into()));
    w.ed(EditorCommand::SetCondition {
        context: EventContext::Music,
        on: true,
    });
    w.ed(EditorCommand::SetCondition {
        context: EventContext::Study,
        on: true,
    });
    w.ed(EditorCommand::Save);
    let Some(doze) = w.state.expressions().find("doze") else {
        assert!(w.state.expressions().find("doze").is_some());
        return;
    };
    assert!(!doze.conditions.music);
    assert!(doze.conditions.study);

    w.ed(EditorCommand::EditExisting("doze".into()));
    w.ed(EditorCommand::ResetRule);
    w.ed(EditorCommand::Save);
    let Some(doze) = w.state.expressions().find("doze") else {
        assert!(w.state.expressions().find("doze").is_some());
        return;
    };
    assert!(!doze.conditions.study);
    assert!(doze.conditions.chill);
    assert!(!doze.conditions.allows(ActivityMode::Study, false));
    assert!(
        !Situation {
            activity: ActivityMode::Chill,
            music: true
        }
        .shows_manual(doze)
    );
}

#[test]
fn delete_only_affects_custom_expressions() {
    let mut w = World::new();
    let mut list = sample_raw();
    list.push(ExpressionMeta {
        id: "custom-1".into(),
        name: "自定义".into(),
        kind: ExpressionKind::Accessory,
        duration_ms: 3200,
        random: true,
        conditions: Conditions::ALL_ON,
        custom: true,
        ..Default::default()
    });
    w.send(Command::Character(CharacterCommand::Loaded {
        id: "sakura".into(),
        name: "樱音".into(),
        expressions: list,
        canvas: (800, 800),
        custom_warning: None,
    }));

    assert!(w.ed(EditorCommand::Delete("think".into())).is_empty());
    let fx = w.ed(EditorCommand::Delete("custom-1".into()));
    assert_eq!(
        fx,
        vec![Effect::DeleteCustomExpression {
            character: "sakura".into(),
            id: "custom-1".into(),
        }]
    );

    // 验证规则持久化往返。
    w.ed(EditorCommand::EditExisting("think".into()));
    w.ed(EditorCommand::SetCondition {
        context: EventContext::Music,
        on: false,
    });
    w.ed(EditorCommand::Save);
    let json = w.state.persisted().to_json();
    let round = Persisted::from_json(&json);
    assert_eq!(
        round
            .expression_rules
            .get("sakura", "think")
            .map(|r| r.conditions.music),
        Some(false)
    );
}
