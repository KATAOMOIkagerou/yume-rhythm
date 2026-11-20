//! 活动模式与随机事件开关矩阵（功能清单第 5、7 节）。

use serde::{Deserialize, Serialize};

/// 活动模式：自习或休闲。影响角色道具和随机事件。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ActivityMode {
    #[default]
    Chill,
    Study,
}

/// 随机事件。每种事件对应一个附加表情（见角色包 expressions.json 的 `event` 字段）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RandomEvent {
    Doze,
    Think,
    Shock,
    Love,
}

impl RandomEvent {
    pub const ALL: [Self; 4] = [Self::Doze, Self::Think, Self::Shock, Self::Love];

    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Self::Doze => "doze",
            Self::Think => "think",
            Self::Shock => "shock",
            Self::Love => "love",
        }
    }

    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|e| e.key() == key)
    }
}

/// 开关矩阵的列：休闲、自习、音乐播放中。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EventContext {
    Chill,
    Study,
    Music,
}

impl EventContext {
    pub const ALL: [Self; 3] = [Self::Chill, Self::Study, Self::Music];

    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Self::Chill => "chill",
            Self::Study => "study",
            Self::Music => "music",
        }
    }
}

impl From<ActivityMode> for EventContext {
    fn from(mode: ActivityMode) -> Self {
        match mode {
            ActivityMode::Chill => Self::Chill,
            ActivityMode::Study => Self::Study,
        }
    }
}

/// 一列四个开关。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventRow {
    pub doze: bool,
    pub think: bool,
    pub shock: bool,
    pub love: bool,
}

impl EventRow {
    const fn all(on: bool) -> Self {
        Self {
            doze: on,
            think: on,
            shock: on,
            love: on,
        }
    }

    #[must_use]
    pub fn get(&self, event: RandomEvent) -> bool {
        match event {
            RandomEvent::Doze => self.doze,
            RandomEvent::Think => self.think,
            RandomEvent::Shock => self.shock,
            RandomEvent::Love => self.love,
        }
    }

    fn slot(&mut self, event: RandomEvent) -> &mut bool {
        match event {
            RandomEvent::Doze => &mut self.doze,
            RandomEvent::Think => &mut self.think,
            RandomEvent::Shock => &mut self.shock,
            RandomEvent::Love => &mut self.love,
        }
    }
}

/// 随机事件开关矩阵：事件 × 场景。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventSwitches {
    pub chill: EventRow,
    pub study: EventRow,
    pub music: EventRow,
}

impl Default for EventSwitches {
    /// 休闲全开；自习和音乐中不打瞌睡。
    fn default() -> Self {
        let no_doze = EventRow {
            doze: false,
            ..EventRow::all(true)
        };
        Self {
            chill: EventRow::all(true),
            study: no_doze,
            music: no_doze,
        }
    }
}

impl EventSwitches {
    #[must_use]
    pub fn row(&self, ctx: EventContext) -> &EventRow {
        match ctx {
            EventContext::Chill => &self.chill,
            EventContext::Study => &self.study,
            EventContext::Music => &self.music,
        }
    }

    fn row_mut(&mut self, ctx: EventContext) -> &mut EventRow {
        match ctx {
            EventContext::Chill => &mut self.chill,
            EventContext::Study => &mut self.study,
            EventContext::Music => &mut self.music,
        }
    }

    #[must_use]
    pub fn get(&self, ctx: EventContext, event: RandomEvent) -> bool {
        self.row(ctx).get(event)
    }

    /// 某个开关是否允许用户修改。音乐中打瞌睡永远关闭。
    #[must_use]
    pub fn is_editable(ctx: EventContext, event: RandomEvent) -> bool {
        !(ctx == EventContext::Music && event == RandomEvent::Doze)
    }

    /// 设置开关，返回是否有变化。不可修改的开关保持原值。
    pub fn set(&mut self, ctx: EventContext, event: RandomEvent, on: bool) -> bool {
        if !Self::is_editable(ctx, event) {
            return false;
        }
        let slot = self.row_mut(ctx).slot(event);
        let changed = *slot != on;
        *slot = on;
        changed
    }

    /// 强制满足不变式（读入旧数据后调用）。
    pub fn normalize(&mut self) {
        self.music.doze = false;
    }

    /// 当前场景下是否允许该随机事件。
    ///
    /// 规则：音乐中一律不打瞌睡；当前活动模式那一列必须允许；
    /// 音乐播放中还要求"音乐"那一列也允许。
    #[must_use]
    pub fn is_allowed(&self, event: RandomEvent, activity: ActivityMode, music: bool) -> bool {
        if music && event == RandomEvent::Doze {
            return false;
        }
        if !self.get(activity.into(), event) {
            return false;
        }
        !music || self.get(EventContext::Music, event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ActivityMode::{Chill, Study};
    use RandomEvent::{Doze, Love, Think};

    #[test]
    fn defaults_only_allow_doze_while_chilling() {
        let s = EventSwitches::default();
        assert!(s.is_allowed(Doze, Chill, false));
        assert!(!s.is_allowed(Doze, Study, false));
        assert!(s.is_allowed(Think, Study, false));
    }

    #[test]
    fn music_never_allows_doze_and_requires_music_column() {
        let mut s = EventSwitches::default();
        assert!(!s.is_allowed(Doze, Chill, true));
        assert!(s.is_allowed(Love, Chill, true));
        s.set(EventContext::Music, Love, false);
        assert!(!s.is_allowed(Love, Chill, true));
        assert!(s.is_allowed(Love, Chill, false));
    }

    #[test]
    fn music_doze_switch_is_locked_off() {
        let mut s = EventSwitches::default();
        assert!(!s.set(EventContext::Music, Doze, true));
        assert!(!s.music.doze);
    }
}
