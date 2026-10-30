# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
"""Educational grand staff: explicit accidentals and proportional time spacing."""
from bisect import bisect_left, bisect_right
from PySide6.QtCore import Qt, QRectF
from PySide6.QtGui import QPainter, QColor, QPen, QFont
from PySide6.QtWidgets import QWidget

from pianoflow.ui.theme import is_dark

STEPS = (0,0,1,1,2,3,3,4,4,5,5,6)


def diatonic(pitch):
    return (pitch//12)*7+STEPS[pitch%12]


class StaffView(QWidget):
    def __init__(self,parent=None):
        super().__init__(parent)
        self.song=None; self.position=0.; self.hand=''; self.selected_tracks=None; self.show_fingers=True
        self.setMinimumHeight(160)
        self.notes=[]; self.starts=[]

    def set_song(self,song):
        self.song=song; self.notes=sorted(song.notes,key=lambda n:n.start); self.starts=[n.start for n in self.notes]; self.update()

    def paintEvent(self,event):
        p=QPainter(self); p.setRenderHint(QPainter.RenderHint.Antialiasing)
        p.fillRect(self.rect(),QColor('#263442' if is_dark() else '#f1eee5'))
        if not self.song: return
        line=8; top=max(25,(self.height()-130)/2); bass=top+85
        p.setPen(QColor('#92a1b1' if is_dark() else '#677080'))
        for base in (top,bass):
            for i in range(5): p.drawLine(45,int(base+i*line),self.width()-10,int(base+i*line))
        p.setPen(QColor('#e4ebf2' if is_dark() else '#202733')); p.setFont(QFont('Serif',24))
        p.drawText(QRectF(4,top-8,38,50),Qt.AlignmentFlag.AlignCenter,'𝄞')
        p.drawText(QRectF(4,bass-8,38,50),Qt.AlignmentFlag.AlignCenter,'𝄢')
        p.setFont(QFont('Sans Serif',9)); p.drawText(48,16,'Учебный нотный стан · расстояние по времени · альтерации у каждой ноты')
        window_start=max(0,self.position-1); horizon=6.; scale=max(1,(self.width()-100)/horizon)
        cursor=85+(max(0,self.position)-window_start)*scale
        p.setPen(QPen(QColor('#119080'),2)); p.drawLine(int(cursor),int(top-15),int(cursor),int(bass+50))
        left=bisect_left(self.starts,window_start-1); right=bisect_right(self.starts,window_start+horizon)
        for n in self.notes[left:right]:
            if self.hand and n.hand!=self.hand: continue
            if self.selected_tracks is not None and n.track not in self.selected_tracks: continue
            if n.start<window_start: continue
            treble=n.hand=='R' or (not n.hand and n.pitch>=60)
            bottom=top+32 if treble else bass+32
            reference=64 if treble else 43  # E4 / G2, bottom lines
            y=bottom-(diatonic(n.pitch)-diatonic(reference))*line/2
            x=85+(n.start-window_start)*scale
            p.setPen(QPen(QColor('#9caab8' if is_dark() else '#555e6d'),1))
            staff_top=top if treble else bass
            if y>bottom:
                for ledger in range(int(bottom+line),int(y)+1,line): p.drawLine(int(x-9),ledger,int(x+9),ledger)
            if y<staff_top:
                for ledger in range(int(staff_top-line),int(y)-1,-line): p.drawLine(int(x-9),ledger,int(x+9),ledger)
            active=n.start<=self.position<n.end
            color=QColor('#65cebb' if active and is_dark() else '#078877' if active else '#e4ebf2' if is_dark() else '#293244')
            p.setPen(QPen(color,1.5))
            beats=n.duration*self.song.bpm/60
            p.setBrush(QColor('#263442' if is_dark() else '#f1eee5') if beats>=1.7 else color)
            p.drawEllipse(QRectF(x-6,y-4,12,8))
            if beats<3.5:
                p.drawLine(int(x+6),int(y),int(x+6),int(y-26))
                if beats<.8: p.drawLine(int(x+6),int(y-26),int(x+13),int(y-18))
                if beats<.4: p.drawLine(int(x+6),int(y-21),int(x+13),int(y-13))
            if n.pitch%12 in (1,3,6,8,10): p.drawText(int(x-18),int(y+4),'♯')
            if self.show_fingers and n.finger: p.drawText(int(x-4),int(y-30),str(n.finger))
        p.end()


class ProgressChart(QWidget):
    def __init__(self,parent=None):
        super().__init__(parent); self.values=[]; self.dates=[]; self.setMinimumHeight(110); self.setMaximumHeight(140)

    def paintEvent(self,event):
        p=QPainter(self); p.fillRect(self.rect(),QColor('#243343' if is_dark() else '#edf8fc')); p.setPen(QColor('#a5b7ca' if is_dark() else '#466679'))
        p.drawText(10,18,'Последние завершённые попытки · точность нот, % (режимы и песни могут различаться)')
        if not self.values: p.drawText(10,55,'Завершите занятие — здесь появится история результатов.'); return
        w=max(1,self.width()-65); h=max(1,self.height()-72)
        p.drawText(8,38,'100'); p.drawText(18,self.height()-28,'0')
        p.setPen(QPen(QColor('#67b6ec' if is_dark() else '#2489ac'),2)); previous=None
        for i,value in enumerate(self.values):
            x=45+i*w/max(1,len(self.values)-1); y=35+h*(1-value/100)
            if previous: p.drawLine(int(previous[0]),int(previous[1]),int(x),int(y))
            p.drawEllipse(QRectF(x-3,y-3,6,6)); previous=(x,y)
        if self.dates:
            from datetime import datetime
            p.setPen(QColor('#a5b7ca' if is_dark() else '#61738a'))
            for i in sorted({0,len(self.dates)//2,len(self.dates)-1}):
                x=45+i*w/max(1,len(self.dates)-1)
                p.drawText(QRectF(x-28,self.height()-23,60,20),Qt.AlignmentFlag.AlignCenter,datetime.fromtimestamp(self.dates[i]).strftime('%d.%m'))
        p.end()
