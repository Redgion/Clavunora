# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
"""Bounded MusicXML partwise piano import (one part, multiple voices/staves).
Durations/backup/chords follow https://www.w3.org/2021/06/musicxml40/ .
Unsupported repeats, grace notes and transposing parts are rejected explicitly.
"""
from dataclasses import replace
from fractions import Fraction
from pathlib import Path
import math
import xml.etree.ElementTree as ET
from pianoflow.core.song import Note, Song


def load_musicxml(path):
    path=Path(path)
    if path.stat().st_size>16*1024*1024: raise ValueError('MusicXML больше 16 МБ')
    raw=path.read_bytes()
    if b'<!ENTITY' in raw.upper(): raise ValueError('XML с объявлениями сущностей не поддерживается')
    try: root=ET.fromstring(raw)
    except ET.ParseError as e: raise ValueError(f'Некорректный XML: {e}') from e
    if root.tag!='score-partwise': raise ValueError('Нужен несжатый MusicXML score-partwise (.musicxml или .xml)')
    parts=root.findall('part')
    if len(parts)!=1: raise ValueError('Пока поддерживается одна партия фортепиано с несколькими голосами. Экспортируйте её отдельно.')
    for tag in ('repeat','ending','grace','transpose','unpitched'):
        if root.find('.//'+tag) is not None: raise ValueError(f'MusicXML: {tag} пока не поддерживается; экспортируйте развёрнутый MIDI.')
    for sound in root.findall('.//sound'):
        if any(k in sound.attrib for k in ('dacapo','dalsegno','tocoda','fine')): raise ValueError('Переходы D.C./D.S. пока не поддерживаются')
    divisions=Fraction(1); numerator=4; denominator=4; initial_signature=None
    measure_start=Fraction(0); tempo={Fraction(0):120.}; notes=[]; ties={}; max_staff=1
    for measure in parts[0].findall('measure'):
        cursor=measure_start; furthest=cursor; previous_start=cursor
        for element in measure:
            if element.tag=='attributes':
                d=element.findtext('divisions')
                if d is not None:
                    divisions=Fraction(d)
                    if divisions<=0: raise ValueError('Некорректные divisions')
                meter=element.find('time')
                if meter is not None:
                    numerator=int(meter.findtext('beats','4')); denominator=int(meter.findtext('beat-type','4'))
                    if numerator<=0 or denominator not in (1,2,4,8,16,32,64): raise ValueError('Неподдерживаемый размер')
                    if initial_signature is None: initial_signature=(numerator,denominator)
                    elif initial_signature!=(numerator,denominator): raise ValueError('Смена размера MusicXML пока не поддерживается; используйте MIDI')
            elif element.tag in ('direction','sound'):
                sound=element if element.tag=='sound' else element.find('sound')
                if sound is not None and 'tempo' in sound.attrib:
                    value=float(sound.attrib['tempo'])
                    if not math.isfinite(value) or value<=0: raise ValueError('Некорректный темп')
                    offset=Fraction(element.findtext('offset','0'))/divisions if element.tag=='direction' else 0
                    if cursor+offset<0: raise ValueError('Некорректное смещение темпа')
                    tempo[cursor+offset]=value
                elif element.find('.//metronome') is not None:
                    met=element.find('.//metronome')
                    units={'whole':4.,'half':2.,'quarter':1.,'eighth':.5,'16th':.25}
                    unit=units.get(met.findtext('beat-unit'))
                    if unit is None: raise ValueError('Неподдерживаемая единица метронома')
                    dots=len(met.findall('beat-unit-dot'))
                    value=float(met.findtext('per-minute'))*unit*sum(.5**i for i in range(dots+1))
                    if not math.isfinite(value) or value<=0: raise ValueError('Некорректный темп')
                    tempo[cursor]=value
            elif element.tag in ('backup','forward'):
                duration=Fraction(element.findtext('duration','0'))/divisions
                if duration<=0: raise ValueError('Некорректная длительность')
                cursor+=duration if element.tag=='forward' else -duration
                if cursor<measure_start: raise ValueError('Голос выходит за начало такта')
                furthest=max(furthest,cursor)
            elif element.tag=='note':
                duration=Fraction(element.findtext('duration','0'))/divisions
                if duration<=0: raise ValueError('Нота без положительной длительности')
                chord=element.find('chord') is not None
                start=previous_start if chord else cursor
                if not chord: previous_start=start; cursor+=duration
                furthest=max(furthest,start+duration,cursor)
                if element.find('rest') is not None: continue
                pitch=element.find('pitch')
                if pitch is None: raise ValueError('Нота без высоты')
                alter=Fraction(pitch.findtext('alter','0'))
                if alter.denominator!=1: raise ValueError('Микротональные ноты не поддерживаются')
                semitone={'C':0,'D':2,'E':4,'F':5,'G':7,'A':9,'B':11}[pitch.findtext('step')]
                value=(int(pitch.findtext('octave'))+1)*12+semitone+int(alter)
                if not 0<=value<=127: raise ValueError('Высота вне MIDI-диапазона')
                staff=int(element.findtext('staff','1')); max_staff=max(max_staff,staff)
                if not 1<=staff<=16: raise ValueError('Некорректный номер стана')
                fingering=element.findtext('./notations/technical/fingering','0')
                finger=int(fingering) if fingering in ('1','2','3','4','5') else 0
                key=(element.findtext('voice','1'),staff,value)
                types={tie.get('type') for tie in element.findall('tie')}
                if 'stop' in types:
                    if key not in ties: raise ValueError('Лига без начала')
                    index=ties[key]; old=notes[index]
                    if old[1]+old[2]!=start: raise ValueError('Несмежные связанные ноты')
                    notes[index]=(old[0],old[1],old[2]+duration,old[3],old[4])
                    if 'start' not in types: del ties[key]
                else:
                    notes.append((value,start,duration,staff-1,finger))
                    if 'start' in types: ties[key]=len(notes)-1
                if len(notes)>100000: raise ValueError('Слишком много нот')
        length=Fraction(numerator*4,denominator)
        measure_start=furthest if measure.get('implicit')=='yes' else max(furthest,measure_start+length)
    if ties: raise ValueError('Незавершённая лига')
    if not notes: raise ValueError('В файле нет нот')
    changes=sorted(tempo.items())
    def seconds(beat):
        total=0.
        for i,(start,bpm) in enumerate(changes):
            if start>=beat: break
            stop=min(beat,changes[i+1][0]) if i+1<len(changes) else beat
            total+=float(stop-start)*60/bpm
        return total
    output=tuple(sorted((Note(p,seconds(s),seconds(s+d)-seconds(s),track=track,finger=f) for p,s,d,track,f in notes),key=lambda n:(n.start,n.track,n.pitch)))
    title=root.findtext('work/work-title') or root.findtext('movement-title') or path.stem
    composer=root.findtext("identification/creator[@type='composer']",'')
    return Song(title,output,changes[0][1],initial_signature or (4,4),tuple(f'Стан {i+1}' for i in range(max_staff)),composer=composer,tempo_map=tuple((seconds(t),bpm) for t,bpm in changes),description='Импорт MusicXML. Руки назначаются вручную через «Пальцы…». Учебный нотный стан не воспроизводит исходную вёрстку партитуры.')
