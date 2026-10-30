# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
from pianoflow.ui.responsive import FlowLayout
"""Practice controls separated from the score transport widget."""
import hashlib
from pianoflow.library.store import encode
from PySide6.QtCore import Qt
from PySide6.QtWidgets import (QDialog,QVBoxLayout,QHBoxLayout,QFormLayout,QLabel,QPushButton,
    QDoubleSpinBox,QSpinBox,QCheckBox,QComboBox,QDialogButtonBox,QMessageBox,QFileDialog,QLineEdit,QScrollArea,QWidget)
from pianoflow.learning.engine import Performance, LoopProgress
from pianoflow.core.notes import note_name


class PracticeTools:
    def init_tools(self, layout, store):
        self.store=store
        self.song_id=None
        self.performance=None
        self.attempt_wait=False
        self.last_report=None
        self.loop_progress=LoopProgress()
        self.loop_start=0.; self.loop_end=0.; self.loop_enabled=False
        self.smart=False; self.loop_threshold=90; self.loop_repetitions=3; self.loop_step=.05; self.loop_target=1.
        self.tolerance=.15; self.latency=0.; self.hints=0
        self.audio_engine='preview'; self.soundfont=''; self.volume=.65; self.audio_device=None
        if store:
            audio=store.preference('audio',{})
            self.audio_engine=audio.get('engine','preview'); self.soundfont=audio.get('soundfont',''); self.volume=audio.get('volume',.65)
        self.hand=''; self.accompany=False; self.metronome=False; self.last_beat=None
        self.wait_errors=[]; self.attempt_start=0; self.wait_baseline=0
        self.control_enabled=False; self.command_notes=[21,22,23,24]
        self.command_held=set()
        row=FlowLayout()
        self.hand_combo=QComboBox()
        for title,value in [('Все руки',''),('Правая','R'),('Левая','L')]: self.hand_combo.addItem(title,value)
        self.hand_combo.currentIndexChanged.connect(self.change_hand)
        row.addWidget(self.hand_combo)
        options=QPushButton('Практика…'); options.clicked.connect(self.practice_options); row.addWidget(options)
        edit=QPushButton('Пальцы…'); edit.clicked.connect(self.edit_fingers); row.addWidget(edit)
        self.hint_combo=QComboBox(); self.hint_combo.addItems(['Все подсказки','Только пальцы','Без подсказок'])
        self.hint_combo.setCurrentIndex(self.store.preference('hints',0) if self.store else 0)
        self.hints=self.hint_combo.currentIndex()
        self.hint_combo.currentIndexChanged.connect(self.change_hints); row.addWidget(self.hint_combo)
        view=QComboBox(); view.addItems(["Падающие ноты", "Нотный стан"])
        view.currentIndexChanged.connect(lambda index: self.visuals.setCurrentIndex(index))
        row.addWidget(view)
        row.addStretch(); layout.addLayout(row)
        self.finger_label=QLabel(''); self.finger_label.setWordWrap(True); layout.addWidget(self.finger_label)

    def audio_options(self):
        from PySide6.QtMultimedia import QMediaDevices
        self.pause()
        dialog=QDialog(self); dialog.setWindowTitle('Звук и SoundFont'); layout=QVBoxLayout(dialog); form=QFormLayout()
        engine=QComboBox(); engine.addItems(['Встроенный тембр','FluidSynth / SoundFont']); engine.setCurrentIndex(int(self.audio_engine=='fluid'))
        soundfont=QLineEdit(self.soundfont)
        choose=QPushButton('Выбрать SoundFont…')
        def select():
            path,_=QFileDialog.getOpenFileName(dialog,'SoundFont','','SoundFont (*.sf2 *.sf3)')
            if path: soundfont.setText(path)
        choose.clicked.connect(select)
        volume=QDoubleSpinBox(); volume.setRange(0,100); volume.setValue(self.volume*100)
        outputs=QMediaDevices.audioOutputs(); device=QComboBox(); device.addItem('По умолчанию')
        for output in outputs: device.addItem(output.description())
        for name,control in [('Тембр',engine),('SoundFont',soundfont),('',choose),('Громкость, %',volume),('Выход',device)]: form.addRow(name,control)
        layout.addLayout(form)
        hint=QLabel('Для SoundFont нужна установленная библиотека FluidSynth. Файл SoundFont выбирается отдельно. Встроенный тембр доступен без него.'); hint.setWordWrap(True); layout.addWidget(hint)
        buttons=QDialogButtonBox(QDialogButtonBox.StandardButton.Ok|QDialogButtonBox.StandardButton.Cancel); layout.addWidget(buttons)
        buttons.rejected.connect(dialog.reject)
        def apply():
            was_enabled=self.sound_box.isChecked()
            self.sound_box.setChecked(False)
            self.audio_engine='fluid' if engine.currentIndex() else 'preview'; self.soundfont=soundfont.text(); self.volume=volume.value()/100
            self.audio_device=outputs[device.currentIndex()-1] if device.currentIndex() else None
            if self.store: self.store.set_preference('audio',{'engine':self.audio_engine,'soundfont':self.soundfont,'volume':self.volume})
            if was_enabled: self.sound_box.setChecked(True)
            dialog.accept()
        buttons.accepted.connect(apply); dialog.exec()

    def change_hints(self,index):
        self.hints=index
        self.roll.show_labels=index==0; self.roll.show_fingers=index<2
        self.keyboard.show_labels=index==0
        if self.store: self.store.set_preference('hints',index)
        self.render_position()

    def change_hand(self):
        self.finish_attempt(False)
        self.hand=self.hand_combo.currentData()
        self.build_groups(); self.update_range(); self.seek(self.transport.current())
        self.roll.hand=self.hand
        if self.hand and not self.practice_notes:
            self.message.setText('Для этой руки нет разметки. Откройте «Пальцы…» и назначьте руку нотам.')

    def edit_fingers(self):
        from pianoflow.ui.library import ScoreEditor
        self.pause()
        dialog=ScoreEditor(self.song,self)
        if dialog.exec():
            identifier=self.song_id
            self.set_song(dialog.song)
            if self.store:
                if identifier: self.store.update(identifier,dialog.song)
                else: identifier=self.store.add(dialog.song)
            self.song_id=identifier

    def practice_options(self):
        self.pause()
        dialog=QDialog(self); dialog.setWindowTitle('Настройки практики'); dialog.resize(620,600)
        outer=QVBoxLayout(dialog); scroll=QScrollArea(); scroll.setWidgetResizable(True); body=QWidget(); layout=QVBoxLayout(body); form=QFormLayout()
        scroll.setWidget(body); outer.addWidget(scroll)
        def number(value,low,high,step=.1):
            box=QDoubleSpinBox(); box.setRange(low,high); box.setDecimals(2); box.setSingleStep(step); box.setValue(value); return box
        enabled=QCheckBox('Повторять фрагмент'); enabled.setChecked(self.loop_enabled)
        start=number(self.loop_start,0,self.song.duration)
        end=number(self.loop_end or self.song.duration,0,self.song.duration)
        smart=QCheckBox('Повышать скорость после успешных повторов'); smart.setChecked(self.smart)
        reps=QSpinBox(); reps.setRange(1,20); reps.setValue(self.loop_repetitions)
        threshold=number(self.loop_threshold,0,100,5)
        step=number(self.loop_step*100,1,25,1); target=number(self.loop_target*100,25,200,5)
        tolerance=number(self.tolerance*1000,30,1000,10); latency=number(self.latency*1000,-500,500,10)
        accompany=QCheckBox('Воспроизводить другую руку (при включённом звуке)'); accompany.setChecked(self.accompany)
        metronome=QCheckBox('Метроном (для щелчков включите «Звук»)' ); metronome.setChecked(self.metronome)
        controls=QCheckBox('Команды с MIDI-клавиш'); controls.setChecked(self.control_enabled)
        keys=[]
        for p in self.command_notes:
            box=QSpinBox(); box.setRange(0,127); box.setValue(p); keys.append(box)
        for name,box in [('Цикл',enabled),('Начало, с',start),('Конец, с',end),('Умный повтор',smart),('Успешных повторов',reps),('Порог, %',threshold),('Шаг скорости, %',step),('Цель скорости, %',target),('Допуск ритма, мс',tolerance),('Компенсация задержки, мс',latency),('Сопровождение',accompany),('Метроном',metronome),('Управление',controls)]: form.addRow(name,box)
        for name,box in zip(['MIDI: старт/пауза','MIDI: повтор','MIDI: медленнее','MIDI: быстрее'],keys): form.addRow(name,box)
        layout.addLayout(form)
        audio_button=QPushButton('Тембр и звуковое устройство…'); audio_button.clicked.connect(self.audio_options); layout.addWidget(audio_button)
        explanation=QLabel('Номера MIDI-клавиш: 60 = C4. Команды не должны пересекаться с нотами песни.\nВ ожидании оцениваются ноты; ритм проверяется в непрерывном режиме.')
        explanation.setWordWrap(True); layout.addWidget(explanation)
        buttons=QDialogButtonBox(QDialogButtonBox.StandardButton.Ok|QDialogButtonBox.StandardButton.Cancel); outer.addWidget(buttons)
        def apply():
            if enabled.isChecked() and end.value()<=start.value():
                QMessageBox.warning(dialog,'Фрагмент','Конец должен быть позже начала.'); return
            pitches=[b.value() for b in keys]
            if controls.isChecked() and (len(set(pitches))<4 or set(pitches)&{n.pitch for n in self.song.notes}):
                QMessageBox.warning(dialog,'Команды','Выберите четыре разные клавиши вне нот этой песни.'); return
            if enabled.isChecked() and not any(start.value()<=n.start<end.value() for n in self.practice_notes):
                QMessageBox.warning(dialog,'Фрагмент','В выбранном фрагменте нет нот выбранной партии.'); return
            self.loop_enabled=enabled.isChecked(); self.loop_start=start.value(); self.loop_end=end.value()
            self.smart=smart.isChecked(); self.loop_repetitions=reps.value(); self.loop_threshold=threshold.value(); self.loop_step=step.value()/100; self.loop_target=target.value()/100
            self.tolerance=tolerance.value()/1000; self.latency=latency.value()/1000
            self.accompany=accompany.isChecked(); self.metronome=metronome.isChecked()
            self.control_enabled=controls.isChecked(); self.command_notes=pitches; self.command_held.clear(); self.loop_progress.streak=0
            if self.store:
                self.store.set_preference('practice',{'tolerance':self.tolerance,'latency':self.latency,'metronome':self.metronome,'accompany':self.accompany})
                if self.song_id: self.store.set_preference('loop:'+self.song_id,{'start':self.loop_start,'end':self.loop_end,'enabled':self.loop_enabled})
            self.seek(self.loop_start-1 if self.loop_enabled else self.transport.current())
            dialog.accept()
        buttons.accepted.connect(apply); buttons.rejected.connect(dialog.reject); dialog.exec()

    def handle_command(self,event):
        key=(event.source,event.channel,event.note)
        if key in self.command_held:
            if event.type=='note_off' or (event.type=='note_on' and not event.velocity): self.command_held.discard(key)
            return True
        if not self.control_enabled or event.source!='midi' or event.note not in self.command_notes or event.type not in ('note_on','note_off'): return False
        if event.note in {n.pitch for n in self.song.notes}: return False
        if event.type=='note_on' and event.velocity:
            self.command_held.add(key)
            action=self.command_notes.index(event.note)
            if action==0: self.toggle()
            elif action==1: self.restart()
            else: self.set_speed_value(max(.25,min(2,self.transport.speed+(-.05 if action==2 else .05))))
        return True

    def set_speed_value(self,value):
        index=self.speed.findData(round(value,4))
        if index<0: self.speed.addItem(f'{value*100:g}%',round(value,4)); index=self.speed.count()-1
        self.speed.setCurrentIndex(index)

    def start_attempt(self):
        if self.performance is not None or not self.practice_notes: return
        start=max(0,self.transport.current(),self.loop_start if self.loop_enabled else 0)
        end=self.loop_end if self.loop_enabled else None
        self.attempt_wait=self.wait_box.isChecked()
        self.attempt_start=start; self.wait_baseline=self.completed; self.wait_errors=[]
        self.performance=Performance(self.practice_notes,self.transport.speed,self.tolerance,self.latency,start,end)
        self.last_beat=None

    def finish_attempt(self,complete):
        if self.performance is None: return None
        if self.attempt_wait:
            total=len({n.start for n in self.performance.notes})
            hit=self.completed-self.wait_baseline
            report={'complete':complete,'total':total,'hit':hit,'accuracy':round(100*hit/max(1,total),1),'mistakes':self.wait_errors.copy()}
        else: report=self.performance.finish(complete)
        report.update(full_song=bool(complete and not self.loop_enabled and not self.hand and self.selected_tracks is None and self.attempt_start==0), both_hands={n.hand for n in self.practice_notes} >= {'L','R'}, difficulty=self.song.difficulty, title=self.song.title,mode='Ожидание' if self.attempt_wait else 'Ритм',speed=self.transport.speed,hand=self.hand,start=self.attempt_start,end=self.loop_end if self.loop_enabled else self.song.duration,tolerance=self.tolerance,latency=self.latency,score_version=hashlib.sha256(encode(self.song).encode()).hexdigest())
        if self.store and (report['hit'] or report['mistakes'] or complete):
            if not self.song_id: self.song_id=self.store.add(self.song)
            self.store.save_attempt(self.song_id,report)
            if complete and report['accuracy']>=90 and not report['mistakes'] and not self.loop_enabled and not self.hand and self.selected_tracks is None and self.attempt_start==0:
                self.store.set_preference('passed:'+self.song_id,True)
        self.performance=None
        self.last_report=report
        if self.store and hasattr(self, "attempt_saved"): self.attempt_saved.emit()
        return report

    def practice_input(self,event):
        if self.performance and self.running:
            if not self.wait_box.isChecked(): self.performance.feed(event,self.transport.current())
            elif self.waiting and event.type=='note_on' and event.velocity>0 and event.note not in self.groups[self.group_index][1]:
                self.wait_errors.append({'position':self.transport.current(),'pitch':event.note,'kind':'Лишняя нота','delta':0})

    def loop_tick(self,position):
        if self.running and self.loop_enabled and position>=self.loop_end:
            report=self.finish_attempt(True)
            if self.smart and report:
                score=max(0,report['accuracy']-100*len(report['mistakes'])/max(1,report['total']))
                value=self.loop_progress.next_speed(self.transport.speed,score,self.loop_threshold,self.loop_repetitions,self.loop_step,self.loop_target)
                self.running=False
                self.set_speed_value(value)
                self.running=True
            self.seek(max(-3,self.loop_start-1))
            self.start_attempt()
            self.message.setText(f'Повтор фрагмента · скорость {self.transport.speed*100:g}%')
            return True
        return False

    def update_hints(self,position,active):
        self.roll.show_labels=self.hints==0; self.roll.show_fingers=self.hints<2
        self.keyboard.show_labels=self.hints==0
        self.keyboard.fingers={}
        notes=[n for n in self.practice_notes if n.start<=position<n.end]
        if self.waiting: notes=[n for n in self.practice_notes if n.start==self.groups[self.group_index][0]]
        if self.hints<2:
            self.keyboard.fingers={n.pitch:('Л' if n.hand=='L' else 'П')+str(n.finger) for n in notes if n.finger and n.hand}
        text=' · '.join(f'{note_name(n.pitch)}: {"левая" if n.hand=="L" else "правая"} {n.finger}' for n in notes if n.finger and n.hand) if self.hints<2 else ''
        if self.hints<2 and not text: text='Левая: 5 4 3 2 1  |  Правая: 1 2 3 4 5 · 1 — большой, 5 — мизинец'
        if self.metronome and self.running and position>=0:
            tempo=self.song.tempo_map or ((0.,self.song.bpm),)
            beat=0.
            for i,(t,bpm) in enumerate(tempo):
                if t>position: break
                stop=min(position,tempo[i+1][0]) if i+1<len(tempo) else position
                beat+=max(0,stop-t)*bpm/60
            beat=int(beat)
            text+=f'  |  Доля {beat%self.song.time_signature[0]+1}'
            if beat!=self.last_beat:
                self.last_beat=beat
                if self.audio: self.audio.click(beat%self.song.time_signature[0]==0)
        self.finger_label.setText(text)
        if self.hints==2: self.keyboard.expected=set()
