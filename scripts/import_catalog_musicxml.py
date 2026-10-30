# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
"""Offline catalogue preparation (optional music21 dependency, not app runtime).

Input: a directory with score.mxl and meta.json containing id, url, licenses,
edition. Accepted licence metadata is retained alongside the original file.
"""
from pathlib import Path
import argparse
from dataclasses import replace
import json
import shutil
import sys
from bisect import bisect_right

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
from pianoflow.core.song import Note, Song
from pianoflow.library.store import encode, decode


def convert(source, title, level, attribution, cutoff_beats=None, linear_form=False):
    from music21 import converter, meter, articulations, harmony, repeat, bar
    score = converter.parse(source)
    if len(score.parts) not in (1,2):
        raise ValueError('Expected one melody staff or two piano staves; inspect source manually')
    if cutoff_beats is not None:
        # Reviewed movement boundary in original quarter-note offsets, before repeats.
        for part in score.parts:
            for element in list(part):
                if float(element.offset) >= cutoff_beats:
                    part.remove(element)
    if linear_form:
        for element in list(score.recurse().getElementsByClass(repeat.RepeatExpression)):
            element.activeSite.remove(element)
        for measure in score.recurse().getElementsByClass('Measure'):
            if isinstance(measure.leftBarline, bar.Repeat): measure.leftBarline = bar.Barline('regular')
            if isinstance(measure.rightBarline, bar.Repeat): measure.rightBarline = bar.Barline('regular')
        for element in list(score.recurse().getElementsByClass('RepeatBracket')):
            element.activeSite.remove(element)
    score = score.expandRepeats()
    score = score.stripTies()
    boundaries = score.metronomeMarkBoundaries()
    points = {0.:120.}
    for start, end, mark in boundaries:
        value = mark.getQuarterBPM()
        # Text-only tempo expressions do not define a numerical speed.
        # Retain the previous explicit tempo instead of inventing a new one.
        if value is None: continue
        bpm = float(value)
        if bpm <= 0: raise ValueError('Invalid tempo')
        points[float(start)] = bpm
    beats = sorted(points); seconds = [0.]
    for i in range(1,len(beats)):
        seconds.append(seconds[-1]+(beats[i]-beats[i-1])*60/points[beats[i-1]])
    def sec(beat):
        j=max(0,bisect_right(beats,float(beat))-1)
        return seconds[j]+(float(beat)-beats[j])*60/points[beats[j]]
    notes=[]; grace_count=0
    for track,part in enumerate(score.parts):
        events=[n for n in part.flatten().notes if not isinstance(n,harmony.Harmony)]
        grace_groups={}
        for n in events:
            if n.duration.isGrace:
                grace_groups.setdefault(float(n.offset),[]).append(n)
        for n in events:
            at=float(n.offset); length=float(n.quarterLength)
            grace=grace_groups.get(at,[])
            # Explicit educational realization: short ornaments on the beat,
            # borrowing time from the following principal note on that staff.
            anchor=min((float(x.quarterLength) for x in events if float(x.offset)==at and not x.duration.isGrace), default=1.)
            steal=min(.5, anchor*.25) if grace else 0.
            if n.duration.isGrace:
                length=steal/len(grace); at+=grace.index(n)*length; grace_count+=1
            else:
                at+=steal; length-=steal
            if length<=0: raise ValueError('Non-positive realized note duration')
            fingerings=[a.fingerNumber for a in n.articulations if isinstance(a,articulations.Fingering)]
            for pitch in n.pitches:
                finger=int(fingerings[0]) if len(n.pitches)==1 and len(fingerings)==1 and fingerings[0] in range(1,6) else 0
                notes.append(Note(int(pitch.midi),sec(at),sec(at+length)-sec(at),track=track,hand='R' if track==0 else 'L',finger=finger))
    signature=next(iter(score.flatten().getElementsByClass(meter.TimeSignature)),None)
    description='Редакция MusicXML; повторы развёрнуты, лиги длительности объединены. Руки по станам, пальцы только из явно записанной аппликатуры. Педаль и выразительная динамика ещё не поддержаны. Педагогическая проверка не завершена.'
    if linear_form: description+=' Сокращённая учебная форма: один проход без возвратов и повторов.'
    if grace_count: description+=' Форшлаги: условное учебное исполнение на доле за счёт следующей ноты.'
    song=Song(title,tuple(sorted(notes,key=lambda n:(n.start,n.track,n.pitch))),points[0.],(signature.numerator,signature.denominator) if signature else (4,4),('Правая','Левая') if len(score.parts)==2 else ('Мелодия',),level,description=description,source=attribution,tempo_map=tuple((s,points[b]) for s,b in zip(seconds,beats)))
    decode(encode(song))
    return song, grace_count


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('directory',type=Path)
    parser.add_argument('--allow-noncommercial', action='store_true', help='Retain and display CC BY-NC-SA restrictions in this educational catalogue')
    args=parser.parse_args()
    src=args.directory;meta=json.loads((src/'meta.json').read_text())
    allowed={'https://creativecommons.org/publicdomain/zero/1.0/','https://creativecommons.org/licenses/by/4.0/','https://creativecommons.org/publicdomain/mark/1.0/'}
    if args.allow_noncommercial: allowed.add('https://creativecommons.org/licenses/by-nc-sa/4.0/')
    if not meta['licenses'] or not set(meta['licenses'])<=allowed:raise ValueError('Review source licence before import')
    root=ROOT/'pianoflow/resources/catalog';data=json.loads((root/'index.json').read_text());entry=data['entries'][meta['id']-1]
    if entry.get('score'):raise ValueError('Refusing to overwrite an existing edition')
    attribution=meta.get('provider','Noctave')+' · '+meta['licenses'][0]+'\n'+meta['url']+'\n'+meta['edition']
    title=entry['title']+(' · учебная обработка Noctave' if meta['id'] in (22,34) else '')
    if meta.get('linear_form'): title+=' · сокращённая учебная версия'
    song,graces=convert(src/'score.mxl',title,entry['difficulty'],attribution,meta.get('movement_cutoff_beats'),meta.get('linear_form',False))
    if any('/by-nc-' in license for license in meta['licenses']):
        song=replace(song, source='Только некоммерческое использование (CC BY-NC-SA 4.0).\n'+song.source)
    dest=root/entry['id'];dest.mkdir(exist_ok=True)
    (dest/'score.json').write_text(encode(song),encoding='utf8');shutil.copy2(src/'score.mxl',dest/'original.mxl')
    for name in ('original.mscx', 'source-readme.md'):
        if (src/name).is_file(): shutil.copy2(src/name, dest/name)
    meta.update(changes='Repeats expanded; ties consolidated; staff hands; explicit single-note fingerings; on-beat grace realization.',realized_graces=graces,review='pending')
    (dest/'provenance.json').write_text(json.dumps(meta,ensure_ascii=False,indent=2),encoding='utf8')
    entry.update(score=entry['id']+'/score.json',status='playable-review-pending')
    (root/'index.json').write_text(json.dumps(data,ensure_ascii=False,indent=2),encoding='utf8')
    print(entry['number'],len(song.notes),'notes',round(song.duration),'seconds',graces,'graces')


if __name__=='__main__':main()
