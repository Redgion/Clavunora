# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
"""Quiet Vista-inspired satin surfaces, with complete light and dark palettes."""
from importlib.resources import files
from PySide6.QtGui import QColor, QPainter, QLinearGradient, QPalette
from PySide6.QtWidgets import QWidget, QApplication

LIGHT = dict(bg='#edf2f7', panel='#f9fbfd', end='#e8eff6', text='#20344e', muted='#61738a', border='#b7c9da', accent='#237fc4', hover='#dfedf9', selected='#cbe4f8', field='#ffffff', disabled='#8b98a6')
DARK = dict(bg='#182330', panel='#243343', end='#202e3d', text='#e2eaf3', muted='#a5b7ca', border='#425971', accent='#67b6ec', hover='#314b63', selected='#335b7c', field='#1c2937', disabled='#7e8d9b')


def tokens(dark=False): return DARK if dark else LIGHT


def stylesheet(dark=False):
    c=tokens(dark) | {'check':str(files('pianoflow').joinpath('resources/icons/check.svg')).replace('\\','/')}
    return '''
QWidget { background: transparent; color: %(text)s; font-family: 'Noto Sans', 'DejaVu Sans'; font-size: 13px; }
QMainWindow, QDialog { background: %(bg)s; }
QLabel { background: transparent; }
QLabel#title { font-size: 28px; font-weight: 600; }
QLabel#homeBrand { font-family: 'Noto Serif', 'DejaVu Serif'; font-size: 46px; color: %(text)s; }
QLabel#homeHeading { font-family: 'Noto Serif', 'DejaVu Serif'; font-size: 23px; }
QLabel#sectionTitle { font-size: 17px; font-weight: 600; }
QLabel#muted { color: %(muted)s; }
QLabel#eyebrow { color: %(accent)s; font-size: 12px; font-weight: 600; }
QLabel#chord { font-size: 42px; color: %(accent)s; }
QFrame#sidebar, QFrame#topNav { background: qlineargradient(x1:0,y1:0,x2:0,y2:1,stop:0 %(panel)s,stop:1 %(end)s); border-bottom: 1px solid %(border)s; }
QFrame#card { background: %(panel)s; border: 1px solid %(border)s; border-radius: 12px; }
QPushButton, QToolButton { background: qlineargradient(x1:0,y1:0,x2:0,y2:1,stop:0 %(panel)s,stop:1 %(end)s); border: 1px solid %(border)s; border-radius: 7px; padding: 9px 14px; }
QPushButton:hover, QToolButton:hover { background: %(hover)s; border-color: %(accent)s; }
QPushButton:pressed, QToolButton:pressed, QPushButton:checked { background: %(selected)s; border-color: %(accent)s; }
QPushButton:focus, QToolButton:focus { border: 2px solid %(accent)s; }
QPushButton:disabled, QToolButton:disabled { color: %(disabled)s; }
QPushButton#primary { background: qlineargradient(x1:0,y1:0,x2:0,y2:1,stop:0 #6ab5e9,stop:1 #2377b8); color: #ffffff; border: 1px solid #236da4; font-weight: 600; }
QPushButton#primary:hover { background: #348fd0; }
QPushButton#nav { background: transparent; border: 1px solid transparent; padding: 12px 18px; }
QPushButton#nav:hover { background: %(hover)s; }
QPushButton#nav:checked { background: %(selected)s; border: 1px solid %(border)s; }
QToolButton#folderTile { background: transparent; border: 1px solid transparent; border-radius: 10px; padding: 8px; font-size: 15px; }
QToolButton#folderTile:hover { background: %(hover)s; border: 1px solid %(border)s; }
QToolButton#folderTile:focus { border: 2px solid %(accent)s; }
QLineEdit, QSpinBox, QDoubleSpinBox, QComboBox { background: %(field)s; border: 1px solid %(border)s; padding: 8px; border-radius: 7px; selection-background-color: %(selected)s; selection-color: %(text)s; }
QLineEdit:focus, QSpinBox:focus, QComboBox:focus { border-color: %(accent)s; }
QComboBox { min-width: 100px; }
QComboBox QAbstractItemView { background: %(field)s; color: %(text)s; selection-background-color: %(selected)s; selection-color: %(text)s; }
QCheckBox { spacing: 9px; }
QCheckBox::indicator { width: 17px; height: 17px; border: 1px solid %(border)s; border-radius: 4px; background: %(field)s; }
QCheckBox::indicator:checked { background: %(accent)s; border: 1px solid %(accent)s; image: url("%(check)s"); }
QScrollArea { border: none; background: transparent; }
QScrollArea > QWidget > QWidget { background: transparent; }
QListWidget, QTreeWidget, QTableWidget { background: %(panel)s; color: %(text)s; border: 1px solid %(border)s; border-radius: 8px; alternate-background-color: %(end)s; outline: 0; }
QListWidget::item { padding: 8px; border-radius: 6px; }
QListWidget::item:selected, QTreeWidget::item:selected, QTableWidget::item:selected { background: %(selected)s; color: %(text)s; }
QListWidget::item:hover { background: %(hover)s; }
QListWidget#songGrid, QListWidget#medalGrid { background: transparent; border: none; }
QListWidget#songGrid::item, QListWidget#medalGrid::item { border: 1px solid %(border)s; background: %(panel)s; border-radius: 9px; }
QListWidget#songGrid::item:selected, QListWidget#medalGrid::item:selected { border: 2px solid %(accent)s; background: %(selected)s; }
QHeaderView::section { background: %(end)s; color: %(text)s; border: 1px solid %(border)s; padding: 6px; }
QSlider::groove:horizontal { height: 6px; border-radius: 3px; background: %(border)s; }
QSlider::sub-page:horizontal { background: %(accent)s; border-radius: 3px; }
QSlider::handle:horizontal { width: 16px; margin: -6px 0; border: 1px solid %(border)s; border-radius: 8px; background: %(panel)s; }
QProgressBar { border: 1px solid %(border)s; border-radius: 5px; background: %(end)s; text-align: center; min-height: 8px; }
QProgressBar::chunk { background: %(accent)s; border-radius: 4px; }
QStatusBar { background: %(end)s; color: %(muted)s; border-top: 1px solid %(border)s; }
QToolTip, QMenu { background: %(panel)s; color: %(text)s; border: 1px solid %(border)s; padding: 6px; }
QMenu::item:selected { background: %(selected)s; }
QScrollBar:vertical { background: %(end)s; width: 10px; border-radius: 5px; }
QScrollBar::handle:vertical { background: %(border)s; min-height: 25px; border-radius: 5px; }
QScrollBar::add-line:vertical, QScrollBar::sub-line:vertical { height: 0; }
QFrame#toast { background: %(selected)s; border: 1px solid %(accent)s; border-radius: 8px; }
''' % c


def apply_theme(dark=False):
    app=QApplication.instance(); app.setProperty('darkTheme',dark); c=tokens(dark)
    palette=QPalette()
    for role,key in ((QPalette.ColorRole.Window,'bg'),(QPalette.ColorRole.WindowText,'text'),(QPalette.ColorRole.Base,'field'),(QPalette.ColorRole.AlternateBase,'end'),(QPalette.ColorRole.Text,'text'),(QPalette.ColorRole.Button,'panel'),(QPalette.ColorRole.ButtonText,'text'),(QPalette.ColorRole.Highlight,'selected'),(QPalette.ColorRole.HighlightedText,'text'),(QPalette.ColorRole.ToolTipBase,'panel'),(QPalette.ColorRole.ToolTipText,'text'),(QPalette.ColorRole.PlaceholderText,'muted')):
        palette.setColor(role,QColor(c[key]))
    app.setPalette(palette); app.setStyleSheet(stylesheet(dark))
    for widget in app.allWidgets(): widget.update()


def is_dark():
    return bool(QApplication.instance().property('darkTheme'))


class AeroBackground(QWidget):
    def paintEvent(self,event):
        p=QPainter(self); c=tokens(is_dark()); g=QLinearGradient(0,0,0,self.height()); g.setColorAt(0,QColor(c['panel'])); g.setColorAt(1,QColor(c['bg'])); p.fillRect(self.rect(),g)


STYLE=stylesheet()
