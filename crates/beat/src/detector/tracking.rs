//! 锁拍状态机：锁定、相位微调、改锁、低音规律判定和显示 BPM。

use super::estimate::Estimate;
use super::{BIN_MS, BeatDetector, MAX_LAG, MIN_LAG, ShownSample, WINDOW_BINS};
use crate::jsmath::{self, at, clamp, round, round_i64};

impl BeatDetector {
    /// 锁拍 6 秒后每 400 ms 用较短窗口（4 秒）复查一次，连续 4 次指向同一个
    /// 非八度关系的新周期时直接改锁。基准：`process` 里 `lastFreshAt` 那一段。
    ///
    /// 返回 `true` 表示已改锁（本次不再调用 `update_lock`）。
    pub(super) fn fresh_check(&mut self, now: f64) -> bool {
        self.last_fresh_at = now;
        let Some(est) = self.estimate(400) else {
            self.fresh_count = 0;
            return false;
        };
        let lag_ms = est.raw_f * BIN_MS; // w
        let locked_bin = round_i64(self.locked_lag_ms / BIN_MS); // W
        let mut locked_score = 0.0; // ft
        for d in -1..=1 {
            // 基准 `Y.scores[W+P] ?? 0`：越界按 0。
            let s = usize::try_from(locked_bin + d)
                .ok()
                .and_then(|i| est.scores.get(i))
                .copied()
                .unwrap_or(0.0);
            locked_score = jsmath::max(locked_score, s);
        }
        let bass_guard = self.bass_protected
            && self.bass_cadence_supports(now, self.locked_lag_ms * 2.0, 2, 0.9);
        let ratio = lag_ms / self.locked_lag_ms;
        let octave_apart = (ratio.log2().abs() - 1.0).abs() < 0.08;
        if !octave_apart
            && !bass_guard
            && (ratio - 1.0).abs() > 0.12
            && est.conf >= 2.0
            && at(&est.scores, est.raw_lag) > locked_score * 2.0
        {
            let same = self.fresh_lag != 0.0
                && !self.fresh_lag.is_nan()
                && (lag_ms / self.fresh_lag - 1.0).abs() < 0.04;
            self.fresh_count = if same { self.fresh_count + 1 } else { 1 };
            self.fresh_lag = lag_ms;
            if self.fresh_count >= 4 {
                self.first_bin = self.first_bin.max(self.last_bin - 400);
                self.commit_with(now, &est.raw_phases, lag_ms, est.bass_override, false);
                self.fresh_count = 0;
                return true;
            }
        } else {
            self.fresh_count = 0;
        }
        false
    }

    /// 根据一次节奏估计更新锁拍状态。基准：`x1.updateLock(h, m)`。
    pub(super) fn update_lock(&mut self, now: f64, est: Option<&Estimate>) {
        let Some(m) = est else {
            self.pending_count = 0;
            return;
        };
        if self.sound_since == 0.0 {
            self.sound_since = now;
        }
        if !self.locked {
            // 基准 `fastLag` 恒为 null，这里只走 rawF / rawPhases 分支。
            let lag_ms = m.raw_f * BIN_MS;
            if now - self.sound_since > 4e3 && m.conf >= 1.08 {
                self.commit_with(now, &m.raw_phases, lag_ms, false, false);
                return;
            }
            if m.conf >= 1.25 {
                self.count_pending(lag_ms);
                if self.pending_count >= 2 {
                    self.commit_with(now, &m.raw_phases, lag_ms, m.bass_override, false);
                }
            } else {
                self.pending_count = 0;
            }
            return;
        }

        let lag_ms = m.raw_f * BIN_MS; // x
        let rel = (lag_ms - self.locked_lag_ms).abs() / self.locked_lag_ms; // N
        if self.bass_protected
            && !m.bass_override
            && (lag_ms / self.locked_lag_ms - 1.5).abs() < 0.13
        {
            if self.bass_cadence_supports(now, self.locked_lag_ms * 2.0, 2, 0.9)
                || !self.bass_cadence_supports(now, lag_ms * 2.0, 1, 0.9)
            {
                return;
            }
            self.bass_protected = false;
        }
        let ratio = lag_ms / self.locked_lag_ms;
        if ratio > 1.85
            && ratio < 2.15
            && self.bass_cadence_supports(now, self.locked_lag_ms, 2, 0.0)
        {
            self.pending_count = 0;
            return;
        }
        if m.conf < 1.05 {
            if self.low_conf_since == 0.0 {
                self.low_conf_since = now;
            }
            if now - self.low_conf_since > 5e3 {
                self.locked = false;
                self.low_conf_since = 0.0;
                self.shown_bpm = 0.0;
                return;
            }
        } else {
            self.low_conf_since = 0.0;
        }

        let locked_bin = round_i64(self.locked_lag_ms / BIN_MS); // R
        let mut locked_score = 0.0; // U
        for d in -1..=1 {
            let w = locked_bin + d;
            if (MIN_LAG..=MAX_LAG).contains(&w) {
                locked_score = jsmath::max(locked_score, at(&m.scores, w));
            }
        }
        let raw_score = at(&m.scores, m.raw_lag); // M
        let per = |i: i64| at(&m.periodicity, i);
        let cov = |i: i64| at(&m.coverage_by_lag, i);
        // K：估计落在 2/3 周期附近且各项指标都支持（附点 / 三连音感）。
        let three_two = !m.percussive
            && !self.bass_protected
            && per(m.raw_lag) >= 0.25
            && ratio > 0.64
            && ratio < 0.7
            && raw_score >= locked_score * 0.85
            && raw_score < locked_score * 1.2
            && cov(m.raw_lag) >= cov(locked_bin) * 1.25
            && per(m.raw_lag) >= per(locked_bin) * 0.8;
        let metrical = m.metrical_override || three_two; // B
        let halving = ratio > 0.46 && ratio < 0.54; // H
        let bass_halving = halving && self.bass_cadence_supports(now, lag_ms, 3, 0.55); // nt
        let keep_ratio = if halving { 0.65 } else { 0.8 }; // et

        // 当前周期仍然够好（或估计与之几乎相同）：缓慢跟随并把相位往候选相位拉 ±8 ms。
        if (!m.bass_override
            && !metrical
            && !bass_halving
            && locked_score >= keep_ratio * raw_score)
            || rel < 0.035
        {
            let target = if rel < 0.035 {
                lag_ms
            } else {
                self.locked_lag_ms
            };
            self.locked_lag_ms = self.locked_lag_ms * 0.92 + target * 0.08;
            let window = self.window(WINDOW_BINS);
            let phases = self.phases_for(
                &window.v,
                &window.weights,
                window.n,
                round_i64(self.locked_lag_ms / BIN_MS),
            );
            let anchor = if self.bass_protected {
                self.bass_anchor(now, self.locked_lag_ms)
            } else {
                None
            };
            let phase_bin = match anchor {
                None => self.pick_phase(&phases, self.locked_lag_ms, now),
                Some(t) => round_i64(t / BIN_MS),
            };
            let lag = self.locked_lag_ms;
            let err = ((phase_bin as f64 * BIN_MS - self.next_beat + lag * 1.5) % lag + lag) % lag
                - lag / 2.0;
            self.next_beat += clamp(err, -8.0, 8.0);
            self.pending_count = 0;
            return;
        }
        if rel < 0.12 && m.conf >= 1.25 && locked_score >= raw_score * 0.5 {
            self.locked_lag_ms = self.locked_lag_ms * 0.88 + lag_ms * 0.12;
            self.pending_count = 0;
            return;
        }

        // 改锁需要连续若干次同样的估计；八度跳变在刚锁定时门槛较低，其后较高。
        let advantage = if locked_score > 0.0 {
            raw_score / locked_score
        } else {
            f64::INFINITY
        }; // X
        let octaves = ratio.log2();
        let octave_jump = (octaves - round(octaves)).abs() < 0.06 && round(octaves).abs() >= 1.0; // rt
        let early_metrical = now - self.acquired_at < 2e4
            && !self.initial_octave_corrected
            && (bass_halving || (metrical && per(m.raw_lag) >= per(locked_bin) * 0.9)); // st
        let early =
            !self.initial_octave_corrected && (now - self.acquired_at < 6500.0 || early_metrical); // dt
        let need_advantage = if octave_jump {
            if early {
                if bass_halving {
                    0.45
                } else if early_metrical {
                    0.75
                } else {
                    1.15
                }
            } else {
                1.8
            }
        } else if metrical || bass_halving {
            0.45
        } else if halving {
            1.15
        } else {
            1.2
        }; // Mt
        let need_count = if octave_jump {
            if early {
                if bass_halving { 2 } else { 5 }
            } else {
                10
            }
        } else if m.bass_override || bass_halving {
            2
        } else if metrical {
            5
        } else if halving {
            4
        } else {
            3
        }; // k
        if m.conf >= 1.25 && (m.bass_override || advantage >= need_advantage) {
            self.count_pending(lag_ms);
            if self.pending_count >= need_count {
                self.commit_with(now, &m.raw_phases, lag_ms, m.bass_override, m.percussive);
            }
        } else {
            self.pending_count = 0;
        }
    }

    /// 基准：`pendingLag>0 && |w-pendingLag|/w<.04 ? pendingCount++ : (pendingLag=w, pendingCount=1)`。
    fn count_pending(&mut self, lag_ms: f64) {
        if self.pending_lag > 0.0 && (lag_ms - self.pending_lag).abs() / lag_ms < 0.04 {
            self.pending_count += 1;
        } else {
            self.pending_lag = lag_ms;
            self.pending_count = 1;
        }
    }

    /// 锁定到周期 `lag_ms`，并选定下一拍。基准：`x1.commitWith(h, m, f, b = false, x = false)`。
    pub(super) fn commit_with(
        &mut self,
        now: f64,
        phases: &[i64],
        lag_ms: f64,
        bass_override: bool,
        percussive: bool,
    ) {
        if self.locked {
            if ((lag_ms / self.locked_lag_ms).log2().abs() - 1.0).abs() < 0.08 {
                self.initial_octave_corrected = true;
            }
        } else {
            self.acquired_at = now;
            self.initial_octave_corrected = false;
        }
        if !self.locked || (lag_ms / self.locked_lag_ms - 1.0).abs() > 0.12 {
            self.shown_samples.clear();
            self.shown_median = 0.0;
            self.shown_cand = 0.0;
        }
        if self.locked && percussive && (lag_ms / self.locked_lag_ms - 1.0).abs() > 0.12 {
            self.first_bin = self.first_bin.max(self.last_bin - 250);
            self.bass_onsets.retain(|o| now - o.time < 2500.0);
        }
        self.locked = true;
        self.committed_at = now;
        self.locked_lag_ms = lag_ms;
        self.bass_protected = bass_override;
        let anchor = if bass_override {
            self.bass_anchor(now, lag_ms)
        } else {
            None
        };
        // 注意：pick_phase 在 locked 已置真之后调用，用的是旧的 next_beat（与基准相同）。
        let phase_bin = match anchor {
            None => self.pick_phase(phases, lag_ms, now),
            Some(t) => round_i64(t / BIN_MS),
        };
        self.next_beat = next_grid_after(now, phase_bin, lag_ms);
        self.pending_count = 0;
        self.pending_lag = 0.0;
        self.low_conf_since = 0.0;
        self.fresh_count = 0;
        self.fresh_lag = 0.0;
    }

    /// 选相位：未锁拍取第一个，锁拍时取离当前预测最近的。基准：`x1.pickPhase(h, m, f)`。
    pub(super) fn pick_phase(&self, phases: &[i64], lag_ms: f64, now: f64) -> i64 {
        let Some(&first) = phases.first() else {
            return (now / BIN_MS).floor() as i64;
        };
        if !self.locked {
            return first;
        }
        let mut best = first;
        let mut best_dist = f64::INFINITY;
        for &p in phases {
            let r = ((p as f64 * BIN_MS - self.next_beat) % lag_ms + lag_ms) % lag_ms;
            let d = jsmath::min(r, lag_ms - r);
            if d < best_dist {
                best_dist = d;
                best = p;
            }
        }
        best
    }

    /// 低音最规律的起音时间，作为相位锚点。基准：`x1.bassAnchor(h, m)`。
    pub(super) fn bass_anchor(&self, now: f64, lag_ms: f64) -> Option<f64> {
        let recent: Vec<_> = self
            .bass_onsets
            .iter()
            .filter(|o| now - o.time < 4500.0)
            .copied()
            .collect();
        let last = recent.last()?;
        if recent.len() < 2 {
            return None;
        }
        let mut best = 0.0;
        let mut anchor = last.time;
        for n in &recent {
            let mut count = 0_usize;
            let mut strength_sum = 0.0;
            for m in &recent {
                let k = (m.time - n.time) / lag_ms;
                if (k - round(k)).abs() < 0.15 {
                    count += 1;
                    strength_sum += m.strength;
                }
            }
            let score = count as f64 + strength_sum * 0.2;
            if score > best {
                best = score;
                anchor = n.time;
            }
        }
        (best >= 2.2).then_some(anchor)
    }

    /// 最近 4.2 秒内强度大于 `min_strength` 的低音起音中，至少有 `need` 个之后还有一个
    /// 间隔约为 `period`（±12%）的起音。
    ///
    /// 基准：`x1.bassCadenceSupports(h, m, f = 2, b = false, x = b ? .9 : 0)`；
    /// 默认参数在调用处展开（`b = true` 对应 `min_strength = 0.9`）。
    pub(super) fn bass_cadence_supports(
        &self,
        now: f64,
        period: f64,
        need: usize,
        min_strength: f64,
    ) -> bool {
        let times: Vec<f64> = self
            .bass_onsets
            .iter()
            .filter(|o| now - o.time < 4200.0 && o.strength > min_strength)
            .map(|o| o.time)
            .collect();
        let mut hits = 0;
        for (i, &t) in times.iter().enumerate() {
            if times[i + 1..]
                .iter()
                .any(|&u| (u - t - period).abs() < period * 0.12)
            {
                hits += 1;
            }
        }
        hits >= need
    }

    /// 显示用 BPM：每 100 ms 采样、取最近 9 个的中位数，变化超过 1 且持续 400 ms 才更新。
    /// 基准：`x1.displayBpm(h)`。
    pub(super) fn display_bpm(&mut self, now: f64) -> f64 {
        if !self.locked {
            self.shown_samples.clear();
            self.shown_median = 0.0;
            return 0.0;
        }
        let bpm = round(6e4 / self.locked_lag_ms);
        let due = self
            .shown_samples
            .last()
            .is_none_or(|s| now - s.at >= 100.0);
        if due {
            self.shown_samples.push(ShownSample { at: now, bpm });
            if self.shown_samples.len() > 9 {
                self.shown_samples.remove(0);
            }
            let mut sorted: Vec<f64> = self.shown_samples.iter().map(|s| s.bpm).collect();
            sorted.sort_by(f64::total_cmp);
            self.shown_median = sorted[sorted.len() / 2];
        }
        let b = self.shown_median;
        if self.shown_bpm == 0.0 {
            self.shown_bpm = b;
            self.shown_cand = b;
            self.shown_cand_at = now;
            return b;
        }
        if (b - self.shown_bpm).abs() <= 1.0 {
            self.shown_cand = self.shown_bpm;
            return self.shown_bpm;
        }
        if b != self.shown_cand {
            self.shown_cand = b;
            self.shown_cand_at = now;
        } else if now - self.shown_cand_at > 400.0 {
            self.shown_bpm = b;
        }
        self.shown_bpm
    }
}

/// 网格 `bin·10 + k·lag` 上第一个晚于 `now` 的时刻。基准：`x1.nextGridAfter(h, m, f)`。
pub(super) fn next_grid_after(now: f64, bin: i64, lag_ms: f64) -> f64 {
    let mut x = bin as f64 * BIN_MS;
    if x <= now {
        x += ((now - x) / lag_ms).ceil() * lag_ms;
    }
    if x <= now {
        x += lag_ms;
    }
    x
}
