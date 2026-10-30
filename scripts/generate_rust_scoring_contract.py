# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
"""Generate deterministic, user-data-free reference results using the Python engine."""
from pathlib import Path
import sys
ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT))
from dataclasses import asdict
import json,random
from pianoflow.core.song import Note
from pianoflow.core.state import MidiEvent
from pianoflow.learning.engine import Performance


def build():
    cases=[]
    def case(name,notes,events,options=None,complete=True):
        options={'speed':1.,'tolerance':.15,'latency':0.,'start':0.,'end':None}|(options or {})
        engine=Performance(notes,**options)
        payload=[]
        for data,position,*rest in events:
            source=rest[0] if rest else 'midi'
            status,pitch,value=data
            event=MidiEvent('note_on' if status&0xf0==0x90 else 'note_off' if status&0xf0==0x80 else 'control_change',pitch,value,status&15,pitch,value,source=source)
            engine.feed(event,position);payload.append({'bytes':data,'position':position,'source':source})
        expected=engine.finish(complete)
        assert engine.finish(complete)==expected
        cases.append(dict(name=name,notes=[asdict(n) for n in notes],options=options,events=payload,complete=complete,expected=expected))
    case('timing-duration-missing',[Note(60,0,1),Note(62,2,.5),Note(64,3,.5)], [([0x90,60,90],.25),([0x80,60,0],.5),([0x90,66,90],2)])
    case('speed-latency',[Note(60,1,.5)],[([0x90,60,90],1.05),([0x80,60,0],1.55)],dict(speed=.5,tolerance=.1,latency=.1))
    case('interrupted-no-input',[Note(60,0,1)],[],complete=False)
    case('clipped-tail',[Note(60,1,2)],[([0x90,60,90],1)],dict(start=1,end=2))
    case('unison-longest',[Note(60,0,.5),Note(60,0,1.5)],[([0x90,60,90],0),([0x80,60,0],1.5)])
    case('source-channel-isolation',[Note(60,0,1),Note(60,2,1)], [([0x90,60,90],0,'computer'),([0x80,60,0],.5,'midi'),([0x80,60,0],1,'computer'),([0x91,60,90],2),([0x80,60,0],3),([0x81,60,0],3)])
    case('unreleased-order',[Note(60,0,1),Note(64,0,1),Note(67,0,1)],[([0x91,67,90],0),([0x90,60,90],0),([0x90,64,90],0)])
    case('zero-velocity-release',[Note(60,0,1)],[([0x90,60,90],0),([0x90,60,0],1)])
    case('nearest-tie',[Note(60,0,.1),Note(60,1,.1)],[([0x90,60,90],.5),([0x80,60,0],.8)])
    case('python-decimal-key',[Note(60,.00000005,.2),Note(60,.000000050000001,.5),Note(60,1.23456785,.3),Note(60,1.234567849,.6)], [([0x90,60,90],.1),([0x80,60,0],.7)])
    rng=random.Random(1907)
    for index in range(160):
        notes=[Note(rng.randrange(60,68),rng.randrange(12)*.25,rng.choice([.1,.4,.8,1.2])) for _ in range(12)]
        options=dict(speed=rng.choice([.25,.5,.75,1.,1.5,2.]),tolerance=rng.choice([.03,.1,.15,.3]),latency=rng.choice([-.1,0.,.1]),start=rng.choice([0.,.5,1.]),end=rng.choice([None,2.5,3.]))
        events=[]
        for _ in range(32):
            status=rng.choice([0x90,0x80,0x91,0x81,0xb0]);pitch=rng.randrange(60,70) if status!=0xb0 else 64
            events.append(([status,pitch,rng.choice([0,90,110])],rng.uniform(-.2,4.),rng.choice(['midi','computer'])))
        events.sort(key=lambda e:e[1])
        case(f'random-{index}',notes,events,options,complete=index%4!=0)
    return cases

if __name__=='__main__':
    target=ROOT/'rust/crates/clavunora-core/tests/fixtures/python-performance.json'
    cases=build();target.write_text(json.dumps(cases,ensure_ascii=False,separators=(',',':')))
    print(f'{len(cases)} Python reference scenarios saved')
