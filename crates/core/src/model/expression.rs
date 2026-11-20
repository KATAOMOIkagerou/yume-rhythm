//! 表情：手动选择、随机表情调度、生效规则（功能清单第 6、7 节）。
//!
//! 表情的图片资源属于角色包，core 只关心元数据（id、类型、事件、时长、是否参与随机）。

use super::events::{ActivityMode, EventSwitches, RandomEvent};
use crate::clock::{Random, WallMs};

/// 必须存在的自然表情。再次点击当前表情会回到它。
pub const NEUTRAL: &str = "neutral";

/// 基础表情替换眼和嘴；附加表情在基础表情上叠加（可替换眼嘴并加特效）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExpressionKind {
    Base,
    Accessory,
}

/// 表情元数据，由角色包加载后提供。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExpressionMeta {
    pub id: String,
    pub name: String,
    pub kind: ExpressionKind,
    /// 关联的随机事件。有事件的表情受开关矩阵约束。
    pub event: Option<RandomEvent>,
    pub duration_ms: u32,
    /// 是否参与随机表情。
    pub random: bool,
}

/// 判断表情是否可用时需要的外部条件。
#[derive(Clone, Copy, Debug)]
pub struct Situation<'a> {
    pub activity: ActivityMode,
    pub music: bool,
    pub switches: &'a EventSwitches,
}

impl Situation<'_> {
    fn allows(&self, meta: &ExpressionMeta) -> bool {
        meta.event
            .is_none_or(|e| self.switches.is_allowed(e, self.activity, self.music))
    }

    /// 手动选择的表情此刻能否显示：播放音乐时不打瞌睡。
    #[must_use]
    pub fn shows_manual(&self, meta: &ExpressionMeta) -> bool {
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
    /// 最近一次选择的基础表情。附加表情叠加在它上面。
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

    /// 替换表情目录（换角色或重载素材后）。已选表情不存在时回到自然表情。
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
    pub fn random_pool(&self, situation: &Situation<'_>) -> Vec<&ExpressionMeta> {
        self.catalog
            .iter()
            .filter(|m| m.random && situation.allows(m))
            .collect()
    }

    /// 当前实际显示的表情：随机表情优先，否则为手动选择。
    /// 音乐播放中手动选择的"打瞌睡"不显示。
    #[must_use]
    pub fn effective(&self, situation: &Situation<'_>) -> Option<&ExpressionMeta> {
        let random = self.random.as_ref().and_then(|r| self.find(&r.id));
        if let Some(meta) = random.filter(|m| situation.allows(m)) {
            return Some(meta);
        }
        self.find(&self.selected)
            .filter(|m| situation.shows_manual(m))
    }

    /// 当前显示的是随机表情（而不是手动选择的）。
    #[must_use]
    pub fn showing_random(&self, situation: &Situation<'_>) -> bool {
        self.random
            .as_ref()
            .and_then(|r| self.find(&r.id))
            .is_some_and(|m| situation.allows(m))
    }

    /// 当前的基础表情：生效表情本身是基础表情时用它，否则用最近选择的基础表情。
    #[must_use]
    pub fn effective_base(&self, situation: &Situation<'_>) -> Option<&ExpressionMeta> {
        match self.effective(situation) {
            Some(meta) if meta.kind == ExpressionKind::Base => Some(meta),
            _ => self.find(&self.base),
        }
    }

    /// 手动选择表情。再次选择当前生效的表情会回到自然表情。手动选择会打断随机表情。
    pub fn select(&mut self, id: &str, situation: &Situation<'_>) {
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
        situation: &Situation<'_>,
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
    pub fn tick(&mut self, situation: &Situation<'_>, now: WallMs, rng: &mut dyn Random) {
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

    fn meta(id: &str, kind: ExpressionKind, event: Option<RandomEvent>) -> ExpressionMeta {
        ExpressionMeta {
            id: id.into(),
            name: id.into(),
            kind,
            event,
            duration_ms: 3200,
            random: id != NEUTRAL,
        }
    }

    fn sample() -> Expressions {
        use ExpressionKind::{Accessory, Base};
        let mut e = Expressions::default();
        e.set_catalog(vec![
            meta(NEUTRAL, Base, None),
            meta("happy", Base, None),
            meta("doze", Accessory, Some(RandomEvent::Doze)),
            meta("think", Accessory, Some(RandomEvent::Think)),
        ]);
        e
    }

    fn situation(switches: &EventSwitches, music: bool) -> Situation<'_> {
        Situation {
            activity: ActivityMode::Chill,
            music,
            switches,
        }
    }

    #[test]
    fn reselecting_current_returns_to_neutral() {
        let sw = EventSwitches::default();
        let s = situation(&sw, false);
        let mut e = sample();
        e.select("happy", &s);
        assert_eq!(e.effective(&s).map(|m| m.id.as_str()), Some("happy"));
        e.select("happy", &s);
        assert_eq!(e.effective(&s).map(|m| m.id.as_str()), Some(NEUTRAL));
    }

    #[test]
    fn accessory_keeps_previous_base() {
        let sw = EventSwitches::default();
        let s = situation(&sw, false);
        let mut e = sample();
        e.select("happy", &s);
        e.select("think", &s);
        assert_eq!(e.effective(&s).map(|m| m.id.as_str()), Some("think"));
        assert_eq!(e.effective_base(&s).map(|m| m.id.as_str()), Some("happy"));
    }

    #[test]
    fn manual_doze_is_hidden_while_music_plays() {
        let sw = EventSwitches::default();
        let mut e = sample();
        e.select("doze", &situation(&sw, false));
        assert_eq!(e.effective(&situation(&sw, true)), None);
        assert!(e.effective(&situation(&sw, false)).is_some());
    }

    #[test]
    fn random_expression_fires_on_interval_and_expires() {
        let sw = EventSwitches::default();
        let s = situation(&sw, false);
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
        let sw = EventSwitches::default();
        let mut e = sample();
        let mut rng = SplitMix64::new(3);
        let chill = situation(&sw, false);
        e.sync_schedule(true, 5, &chill, WallMs(0));
        e.tick(&chill, WallMs(5_000), &mut rng);
        let music = situation(&sw, true);
        e.sync_schedule(true, 5, &music, WallMs(6_000));
        assert_eq!(e.effective(&music).map(|m| m.id.as_str()), Some(NEUTRAL));
        assert!(e.random_pool(&music).iter().all(|m| m.id != "doze"));
    }

    #[test]
    fn disabled_schedule_never_fires() {
        let sw = EventSwitches::default();
        let s = situation(&sw, false);
        let mut e = sample();
        let mut rng = SplitMix64::new(3);
        e.sync_schedule(false, 5, &s, WallMs(0));
        e.tick(&s, WallMs(1_000_000), &mut rng);
        assert_eq!(e.effective(&s).map(|m| m.id.as_str()), Some(NEUTRAL));
    }
}
