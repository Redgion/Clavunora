# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
from datetime import datetime
from PySide6.QtCore import Qt, QSize
from PySide6.QtWidgets import QWidget, QVBoxLayout, QHBoxLayout, QLabel, QListWidget, QListWidgetItem, QListView, QProgressBar, QFrame
from pianoflow.ui.artwork import medal_icon
from pianoflow.ui.responsive import IconGridList


class AchievementsPage(QWidget):
    def __init__(self):
        super().__init__(); self.states=[]
        layout=QVBoxLayout(self); layout.setContentsMargins(30,25,30,25); layout.setSpacing(18)
        title=QLabel('Достижения'); title.setObjectName('title'); layout.addWidget(title)
        self.summary=QLabel(); self.summary.setObjectName('muted'); layout.addWidget(self.summary)
        self.progress=QProgressBar(); self.progress.setRange(0,6); self.progress.setTextVisible(False); self.progress.setFixedHeight(8); layout.addWidget(self.progress)
        self.list=IconGridList(3,210); self.list.setObjectName('medalGrid'); self.list.setViewMode(QListView.ViewMode.IconMode); self.list.setResizeMode(QListView.ResizeMode.Adjust); self.list.setMovement(QListView.Movement.Static); self.list.setWordWrap(True); self.list.setIconSize(QSize(128,128)); self.list.setGridSize(QSize(248,210)); self.list.setSpacing(10); self.list.setAccessibleName('Витрина достижений'); layout.addWidget(self.list,1)
        panel=QFrame(); panel.setObjectName('card'); detail=QVBoxLayout(panel); detail.setContentsMargins(22,16,22,16)
        self.title=QLabel(); self.title.setObjectName('sectionTitle'); detail.addWidget(self.title)
        self.description=QLabel(); self.description.setWordWrap(True); detail.addWidget(self.description)
        self.detail=QLabel(); self.detail.setObjectName('muted'); detail.addWidget(self.detail)
        self.goal=QProgressBar(); detail.addWidget(self.goal); layout.addWidget(panel)
        self.list.currentRowChanged.connect(self.describe)

    def refresh(self,states):
        selected=max(0,self.list.currentRow()); self.states=states; self.list.clear()
        count=sum(bool(s['earned']) for s in states); self.summary.setText(f'Получено {count} из {len(states)} · Медали сохраняются вместе с вашим прогрессом'); self.progress.setMaximum(len(states)); self.progress.setValue(count)
        for state in states:
            a=state['achievement']; earned=state['earned']
            date=datetime.fromtimestamp(earned).strftime('%d.%m.%Y') if earned else f"{state['progress']} / {a.target}"
            item=QListWidgetItem(medal_icon(a.symbol,bool(earned)),a.title+'\n'+date); item.setToolTip(a.description); self.list.addItem(item)
        self.list.setCurrentRow(min(selected,len(states)-1)); self.describe()

    def describe(self,*_):
        index=self.list.currentRow()
        if not 0<=index<len(self.states): return
        s=self.states[index]; a=s['achievement']; self.title.setText(a.title); self.description.setText(a.description)
        self.detail.setText('Получено '+datetime.fromtimestamp(s['earned']).strftime('%d.%m.%Y') if s['earned'] else f"Лучший результат: {s['progress']} / {a.target} · {a.unit}")
        self.goal.setRange(0,a.target); self.goal.setValue(a.target if s['earned'] else s['progress'])
