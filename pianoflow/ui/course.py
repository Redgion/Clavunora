# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
"""A dedicated, ordered course overview; progress uses actual passed lessons."""
from PySide6.QtCore import Signal, Qt, QSize
from PySide6.QtWidgets import QWidget, QVBoxLayout, QGridLayout, QLabel, QPushButton, QProgressBar, QFrame
from pianoflow.learning.course import course_songs
from pianoflow.ui.artwork import cover_icon


class CoursePage(QWidget):
    open_song=Signal(str)

    def __init__(self,store):
        super().__init__(); self.store=store; self.cards=[]; self.columns=0
        layout=QVBoxLayout(self); layout.setContentsMargins(30,25,30,25); layout.setSpacing(18)
        title=QLabel('Курс с нуля'); title.setObjectName('title'); layout.addWidget(title)
        hint=QLabel('Учитесь шаг за шагом · Инструмент или экранная клавиатура'); hint.setObjectName('muted'); layout.addWidget(hint)
        self.summary=QLabel(); layout.addWidget(self.summary)
        self.progress=QProgressBar(); self.progress.setRange(0,6); self.progress.setTextVisible(False); self.progress.setFixedHeight(9); layout.addWidget(self.progress)
        self.body=QWidget(); self.grid=QGridLayout(self.body); self.grid.setContentsMargins(0,0,0,0); self.grid.setSpacing(18); layout.addWidget(self.body,1)
        for index,(identifier,song) in enumerate(course_songs()):
            card=QFrame(); card.setObjectName('card'); inner=QVBoxLayout(card); inner.setContentsMargins(20,18,20,18); inner.setSpacing(12)
            art=QLabel(); art.setPixmap(cover_icon(song.title, index%5+1).pixmap(QSize(200,100))); inner.addWidget(art)
            heading=QLabel(f'{index+1:02}  {song.title}'); heading.setWordWrap(True); heading.setObjectName('sectionTitle'); inner.addWidget(heading)
            text=QLabel(song.description); text.setWordWrap(True); text.setObjectName('muted'); inner.addWidget(text,1)
            action=QPushButton('Начать →'); action.clicked.connect(lambda checked=False,i=identifier:self.open_song.emit(i)); inner.addWidget(action)
            self.cards.append((identifier,card,action))
        self.continue_button=QPushButton('Продолжить обучение →'); self.continue_button.setObjectName('primary'); self.continue_button.clicked.connect(self.continue_course); layout.addWidget(self.continue_button,alignment=Qt.AlignmentFlag.AlignRight)
        self.arrange(3); self.refresh()

    def arrange(self,columns):
        if columns==self.columns: return
        self.columns=columns
        for _,card,_ in self.cards: self.grid.removeWidget(card)
        for index,(_,card,_) in enumerate(self.cards): self.grid.addWidget(card,index//columns,index%columns)

    def resizeEvent(self,event):
        super().resizeEvent(event); self.arrange(max(1,min(3,self.width()//340)))

    def refresh(self):
        passed=0
        for identifier,_,action in self.cards:
            done=self.store.preference('passed:'+identifier,False); passed+=bool(done)
            action.setText('✓ Пройдено · Повторить' if done else 'Начать →')
        self.progress.setValue(passed); self.summary.setText(f'Пройдено {passed} из 6 занятий')

    def continue_course(self):
        identifier=next((i for i,_,_ in self.cards if not self.store.preference('passed:'+i,False)),self.cards[0][0]); self.open_song.emit(identifier)
