# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
"""Achievements derived from actual input, with durable first-earned dates."""
from dataclasses import dataclass
from datetime import datetime


@dataclass(frozen=True)
class Achievement:
    key: str
    title: str
    description: str
    symbol: str
    target: int
    unit: str


ACHIEVEMENTS = (
    Achievement('first-notes', 'Первые ноты', 'Сыграйте хотя бы одну верную ноту в занятии.', '♪', 1, 'занятие'),
    Achievement('first-piece', 'Первое произведение', 'Завершите целую пьесу с точностью от 60%.', '♫', 1, 'пьеса'),
    Achievement('accurate', 'Точное попадание', 'Завершите целую пьесу с точностью от 90%.', '◎', 90, '% точности'),
    Achievement('both-hands', 'Обе руки', 'Завершите пьесу с партиями обеих рук с точностью от 80%.', '♬', 1, 'пьеса'),
    Achievement('seven-days', 'Семь дней музыки', 'Занимайтесь семь дней подряд с верными нотами.', '7', 7, 'дней подряд'),
    Achievement('explorer', 'Исследователь', 'Сыграйте верные ноты в пьесах всех пяти уровней.', '✦', 5, 'уровней'),
)


def achievement_state(store):
    earned = store.preference('achievements', {})
    if not isinstance(earned, dict): earned = {}
    earned = {k: v for k, v in earned.items() if type(v) in (int, float) and 0 < v <= 32503680000}
    previous = set(earned)
    progress = {a.key: 0 for a in ACHIEVEMENTS}
    levels, days = set(), set()
    longest = 0
    last_day = None
    streak = 0
    songs = {i: s for i, s, _ in store.songs()}
    for row in reversed(store.attempts()):
        report = row['report']
        if report.get('hit', 0) <= 0: continue
        stamp = row['created']
        day = datetime.fromtimestamp(stamp).date()
        progress['first-notes'] = 1
        if day not in days:
            streak = streak + 1 if last_day and (day-last_day).days == 1 else 1
            days.add(day); last_day = day; longest = max(longest, streak)
        progress['seven-days'] = longest
        song = songs.get(row['song_id'])
        level = report.get('difficulty', song.difficulty if song else 0)
        if level in range(1, 6): levels.add(level)
        progress['explorer'] = len(levels)
        # A fragment, a filtered track or a single-hand exercise is not a full piece.
        full = report.get('complete') and report.get('full_song', False)
        accuracy = report.get('accuracy', 0)
        if full:
            progress['accurate'] = max(progress['accurate'], int(accuracy))
            if accuracy >= 60: progress['first-piece'] = 1
            if accuracy >= 80 and report.get('both_hands'): progress['both-hands'] = 1
        for a in ACHIEVEMENTS:
            if progress[a.key] >= a.target and a.key not in earned: earned[a.key] = stamp
    if earned != store.preference('achievements', {}): store.set_preference('achievements', earned)
    return [dict(achievement=a, progress=min(a.target, progress[a.key]), earned=earned.get(a.key)) for a in ACHIEVEMENTS], [a for a in ACHIEVEMENTS if a.key in earned and a.key not in previous]
