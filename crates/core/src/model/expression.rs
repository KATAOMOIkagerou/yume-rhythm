//! 表情：手动选择、随机表情调度、生效规则（功能清单第 6、7 节）。
//!
//! 表情的图片资源属于角色包或用户自定义表情目录，core 只关心元数据与文件引用。

use super::events::{ActivityMode, RandomEvent};
use super::expr_rules::Conditions;
use crate::clock::{Random, WallMs};

/// 必须存在的自然表情。再次点击当前表情会回到它。
pub const NEUTRAL: &str = "neutral";

/// 一级表情替换眼和嘴；二级表情在一级表情上叠加（可替换眼嘴并加特效）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ExpressionKind {
    #[default]
    Base,
    Accessory,
}

/// 表情绑定的资源文件名（相对各自目录；自定义表情在加载后带 `custom/` 前缀）。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExpressionAssets {
    pub eyes: Option<String>,
    pub mouth: Option<String>,
    pub overlay: Option<String>,
}

/// 表情元数据，由角色包加载并经条件规则解析后提供。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExpressionMeta {
    pub id: String,
    pub name: String,
    pub kind: ExpressionKind,
    /// 关联的内置随机事件（如 `Doze` 锁定听歌不可用）。
    pub event: Option<RandomEvent>,
    pub duration_ms: u32,
    /// 是否参与随机表情。
    pub random: bool,
    /// 三个场景下是否允许随机出现。
    pub conditions: Conditions,
    /// 是否为用户自定义表情。
    pub custom: bool,
    /// 资源文件名（编辑自定义表情时展示）。
    pub assets: ExpressionAssets,
}

/// 判断表情是否可用时需要的外部条件。
#[derive(Clone, Copy, Debug)]
pub struct Situation {
    pub activity: ActivityMode,
    pub music: bool,
}

impl Situation {
    fn allows(self, meta: &ExpressionMeta) -> bool {
        if self.music && meta.event == Some(RandomEvent::Doze) {
            return false;
        }
        meta.conditions.allows(self.activity, self.music)
    }

    /// 手动选择的表情此刻能否显示：播放音乐时不打瞌睡。
    #[must_use]
    pub fn shows_manual(self, meta: &ExpressionMeta) -> bool {
        !(self.music && meta.event == Some(RandomEvent::Doze))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ActiveRandom {
    id: String,
    until: WallMs,
}

/// 随机调度的依赖项。任何一项变化都会重新计时并清除当前随机表情。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct ScheduleKey {
    enabled: bool,
    interval_s: u32,
    pool: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Expressions {
    catalog: Vec<ExpressionMeta>,
    /// 手动选择的表情。
    selected: String,
    /// 最近一次选择的一级表情。二级表情叠加在它上面。
    base: String,
    random: Option<ActiveRandom>,
    next_random_at: Option<WallMs>,
    schedule: ScheduleKey,
}

impl Default for Expressions {
    fn default() -> Self {
        Self {
            catalog: Vec::new(),
            selected: NEUTRAL.into(),
            base: NEUTRAL.into(),
            random: None,
            next_random_at: None,
            schedule: ScheduleKey::default(),
        }
    }
}

impl Expressions {
    #[must_use]
    pub fn catalog(&self) -> &[ExpressionMeta] {
        &self.catalog
    }

    #[must_use]
    pub fn find(&self, id: &str) -> Option<&ExpressionMeta> {
        self.catalog.iter().find(|m| m.id == id)
    }

    /// 替换表情目录（换角色、重载素材或更新条件后）。已选表情不存在时回到自然表情。
    pub fn set_catalog(&mut self, catalog: Vec<ExpressionMeta>) {
        self.catalog = catalog;
        if self.find(&self.selected).is_none() {
            self.selected = NEUTRAL.into();
        }
        if self
            .find(&self.base)
            .is_none_or(|m| m.kind != ExpressionKind::Base)
        {
            self.base = NEUTRAL.into();
        }
        self.random = None;
    }

    /// 当前参与随机的表情。
    #[must_use]
    pub fn random_pool(&self, situation: &Situation) -> Vec<&ExpressionMeta> {
        self.catalog
            .iter()
            .filter(|m| m.random && situation.allows(m))
            .collect()
    }

    /// 当前实际显示的表情：随机表情优先，否则为手动选择。
    /// 音乐播放中手动选择的"打瞌睡"不显示。
    #[must_use]
    pub fn effective(&self, situation: &Situation) -> Option<&ExpressionMeta> {
        let random = self.random.as_ref().and_then(|r| self.find(&r.id));
        if let Some(meta) = random.filter(|m| situation.allows(m)) {
            return Some(meta);
        }
        self.find(&self.selected)
            .filter(|m| situation.shows_manual(m))
    }

    /// 当前显示的是随机表情（而不是手动选择的）。
    #[must_use]
    pub fn showing_random(&self, situation: &Situation) -> bool {
        self.random
            .as_ref()
            .and_then(|r| self.find(&r.id))
            .is_some_and(|m| situation.allows(m))
    }

    /// 当前的一级表情：生效表情本身是一级表情时用它，否则用最近选择的一级表情。
    #[must_use]
    pub fn effective_base(&self, situation: &Situation) -> Option<&ExpressionMeta> {
        match self.effective(situation) {
            Some(meta) if meta.kind == ExpressionKind::Base => Some(meta),
            _ => self.find(&self.base),
        }
    }

    /// 手动选择表情。再次选择当前生效的表情会回到自然表情。手动选择会打断随机表情。
    pub fn select(&mut self, id: &str, situation: &Situation) {
        if self.find(id).is_none() {
            return;
        }
        let current = self.effective(situation).map(|m| m.id.as_str());
        let next = if current == Some(id) { NEUTRAL } else { id };
        self.random = None;
        self.selected = next.into();
        if self
            .find(next)
            .is_some_and(|m| m.kind == ExpressionKind::Base)
        {
            self.base = next.into();
        }
    }

    /// 随机调度的依赖项变化时重新计时。每次 `update` 之后调用。
    pub fn sync_schedule(
        &mut self,
        enabled: bool,
        interval_s: u32,
        situation: &Situation,
        now: WallMs,
    ) {
        let pool = self
            .random_pool(situation)
            .iter()
            .map(|m| m.id.clone())
            .collect();
        let key = ScheduleKey {
            enabled,
            interval_s,
            pool,
        };
        if key == self.schedule {
            return;
        }
        self.random = None;
        self.next_random_at = enabled.then(|| now.plus(interval_ms(interval_s)));
        self.schedule = key;
    }

    /// 推进随机表情：到期结束，到点抽取下一个。
    pub fn tick(&mut self, situation: &Situation, now: WallMs, rng: &mut dyn Random) {
        if self.random.as_ref().is_some_and(|r| now >= r.until) {
            self.random = None;
        }
        let Some(due) = self.next_random_at.filter(|due| now >= *due) else {
            return;
        };
        let interval = interval_ms(self.schedule.interval_s);
        // 休眠醒来后不补发错过的多次，只从现在起重新计时。
        let next = due.plus(interval);
        self.next_random_at = Some(if next <= now {
            now.plus(interval)
        } else {
            next
        });

        let pool = self.random_pool(situation);
        if let Some(meta) = rng.pick(pool.len()).and_then(|i| pool.get(i)) {
            let until = now.plus(i64::from(meta.duration_ms));
            self.random = Some(ActiveRandom {
                id: meta.id.clone(),
                until,
            });
        }
    }
}

fn interval_ms(interval_s: u32) -> i64 {
    i64::from(interval_s.max(1)) * 1000
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::SplitMix64;

    fn meta(
        id: &str,
        kind: ExpressionKind,
        event: Option<RandomEvent>,
        conditions: Conditions,
    ) -> ExpressionMeta {
        ExpressionMeta {
            id: id.into(),
            name: id.into(),
            kind,
            event,
            duration_ms: 3200,
            random: id != NEUTRAL,
            conditions,
            ..Default::default()
        }
    }

    fn sample() -> Expressions {
        use ExpressionKind::{Accessory, Base};
        let mut e = Expressions::default();
        e.set_catalog(vec![
            meta(NEUTRAL, Base, None, Conditions::ALL_ON),
            meta("happy", Base, None, Conditions::ALL_ON),
            meta(
                "doze",
                Accessory,
                Some(RandomEvent::Doze),
                Conditions {
                    chill: true,
                    study: false,
                    music: false,
                },
            ),
            meta(
                "think",
                Accessory,
                Some(RandomEvent::Think),
                Conditions::ALL_ON,
            ),
        ]);
        e
    }

    fn situation(music: bool) -> Situation {
        Situation {
            activity: ActivityMode::Chill,
            music,
        }
    }

    #[test]
    fn reselecting_current_returns_to_neutral() {
        let s = situation(false);
        let mut e = sample();
        e.select("happy", &s);
        assert_eq!(e.effective(&s).map(|m| m.id.as_str()), Some("happy"));
        e.select("happy", &s);
        assert_eq!(e.effective(&s).map(|m| m.id.as_str()), Some(NEUTRAL));
    }

    #[test]
    fn accessory_keeps_previous_base() {
        let s = situation(false);
        let mut e = sample();
        e.select("happy", &s);
        e.select("think", &s);
        assert_eq!(e.effective(&s).map(|m| m.id.as_str()), Some("think"));
        assert_eq!(e.effective_base(&s).map(|m| m.id.as_str()), Some("happy"));
    }

    #[test]
    fn manual_doze_is_hidden_while_music_plays() {
        let mut e = sample();
        e.select("doze", &situation(false));
        assert_eq!(e.effective(&situation(true)), None);
        assert!(e.effective(&situation(false)).is_some());
    }

    #[test]
    fn random_expression_fires_on_interval_and_expires() {
        let s = situation(false);
        let mut e = sample();
        let mut rng = SplitMix64::new(3);
        let t0 = WallMs(0);
        e.sync_schedule(true, 5, &s, t0);
        e.tick(&s, t0.plus(4_999), &mut rng);
        assert_eq!(e.effective(&s).map(|m| m.id.as_str()), Some(NEUTRAL));
        e.tick(&s, t0.plus(5_000), &mut rng);
        let shown = e.effective(&s).map(|m| m.id.clone());
        assert_ne!(shown.as_deref(), Some(NEUTRAL));
        e.tick(&s, t0.plus(5_000 + 3_200), &mut rng);
        assert_eq!(e.effective(&s).map(|m| m.id.as_str()), Some(NEUTRAL));
    }

    #[test]
    fn pool_change_rearms_and_clears_random() {
        let mut e = sample();
        let mut rng = SplitMix64::new(3);
        let chill = situation(false);
        e.sync_schedule(true, 5, &chill, WallMs(0));
        e.tick(&chill, WallMs(5_000), &mut rng);
        let music = situation(true);
        e.sync_schedule(true, 5, &music, WallMs(6_000));
        assert_eq!(e.effective(&music).map(|m| m.id.as_str()), Some(NEUTRAL));
        assert!(e.random_pool(&music).iter().all(|m| m.id != "doze"));
    }

    #[test]
    fn disabled_schedule_never_fires() {
        let s = situation(false);
        let mut e = sample();
        let mut rng = SplitMix64::new(3);
        e.sync_schedule(false, 5, &s, WallMs(0));
        e.tick(&s, WallMs(1_000_000), &mut rng);
        assert_eq!(e.effective(&s).map(|m| m.id.as_str()), Some(NEUTRAL));
    }
}
