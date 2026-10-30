# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
from dataclasses import replace
from pianoflow.core.song import Note, Song, birthday_song


def course_songs():
    lessons = [
        ('Найдите до', [60,60,60,60], [1,1,1,1], 'R', 'Найдите до первой октавы (C4). Сыграйте большим пальцем правой руки. Отпускайте клавишу перед повтором.'),
        ('Пять пальцев правой руки', [60,62,64,65,67,65,64,62,60], [1,2,3,4,5,4,3,2,1], 'R', 'Поставьте пальцы на до–ре–ми–фа–соль. 1 — большой, 5 — мизинец. Играйте по порядку без перемещения руки.'),
        ('Пять пальцев левой руки', [48,50,52,53,55,53,52,50,48], [5,4,3,2,1,2,3,4,5], 'L', 'Левая рука: мизинец на до, большой на соль. Сначала играйте с ожиданием, затем попробуйте ровный ритм.'),
        ('Ритм и паузы', [60,62,64,60,64,62,60], [1,2,3,1,3,2,1], 'R', 'Считайте 1–2–3–4. После каждой ноты оставляйте паузу. В непрерывном режиме следите за моментом нажатия и отпускания.'),
    ]
    result = []
    for i, (title, pitches, fingers, hand, description) in enumerate(lessons):
        notes = tuple(Note(p, j * .75, .35 if i == 3 else .65, hand=hand, finger=f) for j, (p,f) in enumerate(zip(pitches,fingers)))
        result.append((f'course-{i+1}', Song(title, notes, 80, (4,4), difficulty=1, work='Курс с нуля', description=description, source='Собственные упражнения Clavunora')))
    notes = tuple(n for j in range(4) for n in (Note(48,j, .8,track=0,hand='L',finger=5),Note(60+ (j%3)*2,j,.8,track=1,hand='R',finger=1+j%3)))
    result.append(('course-5',Song('Две руки вместе', notes,60,(4,4),('Левая','Правая'),2,work='Курс с нуля',description='Левая рука повторяет до, правая играет до–ре–ми–до. Нажимайте обе ноты одновременно.',source='Собственное упражнение Clavunora')))
    notes = tuple(Note(p,j*1.5,1.2,hand='R',finger=f) for j in range(3) for p,f in ((60,1),(64,3),(67,5)))
    result.append(('course-6',Song('Первый аккорд', notes,80,(4,4),difficulty=2,work='Курс с нуля',description='Сыграйте до–ми–соль пальцами 1–3–5 правой руки. Все три клавиши должны быть нажаты вместе.',source='Собственное упражнение Clavunora')))
    return result


def birthday_variants():
    song = birthday_song()
    # Explicit suggested fingering for this arrangement, editable by the learner.
    fingers = [1,1,2,1,4,3,1,1,2,1,5,4,1,1,5,4,2,1,2,5,5,4,2,3,2]
    melody = tuple(replace(n, hand='R', finger=f) for n,f in zip(song.notes,fingers))
    base = replace(song, notes=melody, difficulty=1, work='С днём рождения', source='Собственная учебная аранжировка Clavunora', description='Подсказки пальцев — один из вариантов аппликатуры. Разучивайте короткими фразами.')
    bass = tuple(Note(p,t,1.5,track=1,hand='L',finger=f) for p,t,f in ((41,0,5),(48,3.75,1),(41,7.5,5),(46,11.25,2)))
    chords = tuple(Note(p,t,1.4,track=1,hand='L',finger=f) for t,ps in ((0,(41,45,48)),(3.75,(48,52,55)),(7.5,(41,45,48)),(11.25,(46,50,53))) for p,f in zip(ps,(5,3,1)))
    return [('birthday-melody',base),('birthday-bass',replace(base,title='С днём рождения · с басом',notes=tuple(sorted(melody+bass,key=lambda n:n.start)),track_names=('Правая','Левая'),difficulty=3)),('birthday-chords',replace(base,title='С днём рождения · с аккордами',notes=tuple(sorted(melody+chords,key=lambda n:n.start)),track_names=('Правая','Левая'),difficulty=3))]
