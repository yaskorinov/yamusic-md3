//! Состояние очереди и чистая логика порядка (без mpv) — тестируется отдельно.
//! Поля и правила — как в player.py: _order (индексы очереди с учётом shuffle), _pos (позиция в _order),
//! _make_order, _next_pos(manual), previous (> 3 с — в начало), cycle_repeat (волна: off ↔ one),
//! shuffle недоступен волне, repeat one — loop-file=inf у обоих экземпляров.
use ym_api::items::{PlayContext, TrackItem};

use crate::{FeederId, Repeat};

#[derive(Clone, Debug, Default)]
pub struct PlayerState {
    pub queue: Vec<TrackItem>,
    pub order: Vec<usize>,
    /// позиция в order; None — ничего не выбрано
    pub pos: Option<usize>,
    pub context: PlayContext,
    pub feeder: Option<FeederId>,
    pub shuffle: bool,
    pub repeat: Repeat,
}

/// Что делать по «Назад».
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Previous {
    /// перемотать текущий в начало
    Restart,
    /// перейти на позицию в order
    Pos(usize),
}

impl PlayerState {
    /// _make_order(first): 0..n; при shuffle (не волна, n > 1) — first, затем остальные вперемешку.
    pub fn make_order(&self, first: Option<usize>) -> Vec<usize> {
        let n = self.queue.len();
        let mut order: Vec<usize> = (0..n).collect();
        if self.shuffle && n > 1 && !self.is_wave() {
            let mut rest: Vec<usize> = order.iter().copied().filter(|&i| Some(i) != first).collect();
            fastrand::shuffle(&mut rest);
            order = first.filter(|&f| f < n).into_iter().chain(rest).collect();
        }
        order
    }

    /// _next_pos(manual): repeat one и не manual — та же; дальше по order; у волны в конце — None (ждём догрузки);
    /// иначе 0, если repeat all или manual.
    pub fn next_pos(&self, manual: bool) -> Option<usize> {
        if self.order.is_empty() {
            return None;
        }
        if self.repeat == Repeat::One && !manual {
            return self.pos;
        }
        let next = self.pos.map_or(0, |p| p + 1);
        if next < self.order.len() {
            return Some(next);
        }
        if self.is_wave() {
            return None;
        }
        (self.repeat == Repeat::All || manual).then_some(0)
    }

    /// Индекс текущего трека в очереди (currentIndex), -1 — нет.
    pub fn current_index(&self) -> i32 {
        self.current().map_or(-1, |i| i as i32)
    }

    /// Индекс текущего трека в очереди.
    pub fn current(&self) -> Option<usize> {
        self.pos.and_then(|p| self.order.get(p).copied())
    }

    pub fn current_track(&self) -> Option<&TrackItem> {
        self.current().and_then(|i| self.queue.get(i))
    }

    pub fn is_wave(&self) -> bool {
        self.context.kind == "wave"
    }

    /// previous(): после 3 с или в начале списка (без repeat all) — в начало трека, иначе предыдущий по кругу.
    pub fn previous(&self, position_ms: i64) -> Previous {
        let at_start = self.pos.is_none_or(|p| p == 0);
        if position_ms > 3000 || at_start && self.repeat != Repeat::All || self.order.is_empty() {
            return Previous::Restart;
        }
        let len = self.order.len();
        let pos = self.pos.unwrap_or(0);
        Previous::Pos((pos + len - 1) % len)
    }

    /// cycleRepeat(): off → all → one → off; у волны off ↔ one.
    pub fn cycled_repeat(&self) -> Repeat {
        if self.is_wave() {
            if self.repeat != Repeat::Off { Repeat::Off } else { Repeat::One }
        } else {
            match self.repeat {
                Repeat::Off => Repeat::All,
                Repeat::All => Repeat::One,
                Repeat::One => Repeat::Off,
            }
        }
    }

    /// _set_shuffle: пересобрать порядок, текущий трек остаётся текущим. false — ничего не изменилось.
    pub fn set_shuffle(&mut self, value: bool) -> bool {
        if value == self.shuffle || self.is_wave() {
            return false;
        }
        self.shuffle = value;
        let current = self.current();
        self.order = self.make_order(current);
        self.pos = current.and_then(|c| self.order.iter().position(|&i| i == c));
        true
    }

    /// Новая очередь: порядок от row. false — пустой список или row вне его.
    pub fn reset(&mut self, items: Vec<TrackItem>, row: usize) -> bool {
        if items.is_empty() || row >= items.len() {
            return false;
        }
        self.queue = items;
        self.order = self.make_order(Some(row));
        self.pos = self.order.iter().position(|&i| i == row);
        true
    }

    /// Догрузка в конец очереди (новые треки — в конце порядка).
    pub fn append(&mut self, items: Vec<TrackItem>) {
        let first = self.queue.len();
        self.queue.extend(items);
        self.order.extend(first..self.queue.len());
    }

    /// Убрать всё после текущего; порядок — 0..n. Возвращает (at, count) убранного или None, если нет текущего.
    pub fn truncate_after_current(&mut self) -> Option<(usize, usize)> {
        let current = self.current()?;
        let at = current + 1;
        let count = self.queue.len().saturating_sub(at);
        self.queue.truncate(at);
        self.order = (0..self.queue.len()).collect();
        self.pos = Some(current);
        Some((at, count))
    }

    /// Позиция в order для индекса очереди (playIndex).
    pub fn pos_of(&self, index: usize) -> Option<usize> {
        if index >= self.queue.len() {
            return None;
        }
        self.order.iter().position(|&i| i == index)
    }

    /// Сколько треков впереди текущего (для подкормки волны).
    pub fn ahead(&self) -> isize {
        self.order.len() as isize - 1 - self.pos.map_or(-1, |p| p as isize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(id: usize) -> TrackItem {
        TrackItem { track_id: id.to_string(), ..Default::default() }
    }

    fn state(n: usize, kind: &str) -> PlayerState {
        PlayerState {
            queue: (0..n).map(track).collect(),
            order: (0..n).collect(),
            pos: Some(0),
            context: PlayContext::new(kind, ""),
            ..Default::default()
        }
    }

    fn is_permutation(order: &[usize], n: usize) -> bool {
        let mut sorted = order.to_vec();
        sorted.sort_unstable();
        sorted == (0..n).collect::<Vec<_>>()
    }

    #[test]
    fn order_without_shuffle() {
        let s = state(5, "playlist");
        assert_eq!(s.make_order(Some(3)), vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn order_with_shuffle_starts_from_first() {
        let mut s = state(20, "playlist");
        s.shuffle = true;
        for first in [0, 7, 19] {
            let order = s.make_order(Some(first));
            assert_eq!(order[0], first);
            assert!(is_permutation(&order, 20));
        }
        let order = s.make_order(None);
        assert!(is_permutation(&order, 20));
    }

    #[test]
    fn wave_ignores_shuffle() {
        let mut s = state(5, "wave");
        s.shuffle = true;
        assert_eq!(s.make_order(Some(2)), vec![0, 1, 2, 3, 4]);
        assert!(!s.set_shuffle(false));
        let mut s = state(5, "wave");
        assert!(!s.set_shuffle(true));
        assert!(!s.shuffle);
    }

    #[test]
    fn single_track_not_shuffled() {
        let mut s = state(1, "playlist");
        s.shuffle = true;
        assert_eq!(s.make_order(Some(0)), vec![0]);
    }

    #[test]
    fn next_pos_rules() {
        let mut s = state(3, "playlist");
        assert_eq!(s.next_pos(false), Some(1));
        s.pos = Some(2);
        assert_eq!(s.next_pos(false), None);
        assert_eq!(s.next_pos(true), Some(0));
        s.repeat = Repeat::All;
        assert_eq!(s.next_pos(false), Some(0));
        s.repeat = Repeat::One;
        assert_eq!(s.next_pos(false), Some(2));
        assert_eq!(s.next_pos(true), Some(0));
        s.pos = Some(1);
        assert_eq!(s.next_pos(true), Some(2));
        s.pos = None;
        s.repeat = Repeat::Off;
        assert_eq!(s.next_pos(false), Some(0));
        let empty = PlayerState::default();
        assert_eq!(empty.next_pos(true), None);
    }

    #[test]
    fn next_pos_wave_waits_for_more() {
        let mut s = state(3, "wave");
        s.pos = Some(2);
        assert_eq!(s.next_pos(false), None);
        assert_eq!(s.next_pos(true), None);
        s.repeat = Repeat::All;
        assert_eq!(s.next_pos(true), None);
        s.repeat = Repeat::One;
        assert_eq!(s.next_pos(false), Some(2));
    }

    #[test]
    fn previous_rules() {
        let mut s = state(3, "playlist");
        assert_eq!(s.previous(0), Previous::Restart);
        s.repeat = Repeat::All;
        assert_eq!(s.previous(0), Previous::Pos(2));
        assert_eq!(s.previous(3001), Previous::Restart);
        s.repeat = Repeat::Off;
        s.pos = Some(2);
        assert_eq!(s.previous(3000), Previous::Pos(1));
        assert_eq!(s.previous(3001), Previous::Restart);
    }

    #[test]
    fn cycle_repeat() {
        let mut s = state(3, "playlist");
        let mut seen = vec![];
        for _ in 0..3 {
            s.repeat = s.cycled_repeat();
            seen.push(s.repeat);
        }
        assert_eq!(seen, vec![Repeat::All, Repeat::One, Repeat::Off]);
        let mut w = state(3, "wave");
        w.repeat = w.cycled_repeat();
        assert_eq!(w.repeat, Repeat::One);
        w.repeat = w.cycled_repeat();
        assert_eq!(w.repeat, Repeat::Off);
        w.repeat = Repeat::All;
        assert_eq!(w.cycled_repeat(), Repeat::Off);
    }

    #[test]
    fn shuffle_keeps_current() {
        let mut s = state(10, "playlist");
        s.pos = Some(4);
        assert!(s.set_shuffle(true));
        assert_eq!(s.current(), Some(4));
        assert_eq!(s.pos, Some(0));
        assert!(is_permutation(&s.order, 10));
        assert!(!s.set_shuffle(true));
        assert!(s.set_shuffle(false));
        assert_eq!(s.order, (0..10).collect::<Vec<_>>());
        assert_eq!(s.pos, Some(4));
        assert_eq!(s.current_index(), 4);
    }

    #[test]
    fn reset_and_current() {
        let mut s = PlayerState::default();
        assert_eq!(s.current_index(), -1);
        assert!(!s.reset(vec![], 0));
        assert!(!s.reset(vec![track(0)], 1));
        assert!(s.reset((0..4).map(track).collect(), 2));
        assert_eq!(s.pos, Some(2));
        assert_eq!(s.current_index(), 2);
        assert_eq!(s.current_track().map(|t| t.track_id.as_str()), Some("2"));
        s.shuffle = true;
        assert!(s.reset((0..4).map(track).collect(), 3));
        assert_eq!(s.pos, Some(0));
        assert_eq!(s.current_index(), 3);
    }

    #[test]
    fn wave_append_and_replace() {
        let mut s = state(3, "wave");
        s.pos = Some(1);
        assert_eq!(s.ahead(), 1);
        s.append((3..5).map(track).collect());
        assert_eq!(s.order, vec![0, 1, 2, 3, 4]);
        assert_eq!(s.ahead(), 3);
        assert_eq!(s.truncate_after_current(), Some((2, 3)));
        assert_eq!(s.queue.len(), 2);
        assert_eq!(s.order, vec![0, 1]);
        assert_eq!(s.current_index(), 1);
        assert_eq!(s.ahead(), 0);
        s.append((5..7).map(track).collect());
        assert_eq!(s.queue[2].track_id, "5");
        assert_eq!(s.pos_of(3), Some(3));
        assert_eq!(s.pos_of(9), None);
        let mut empty = PlayerState::default();
        assert_eq!(empty.truncate_after_current(), None);
        assert_eq!(empty.ahead(), 0);
    }
}
