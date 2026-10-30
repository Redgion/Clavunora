# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
"""Small native artwork: satin folders, landscape covers and collectible medals."""
import hashlib
from functools import lru_cache
from PySide6.QtCore import Qt, QRectF, QPointF
from PySide6.QtGui import QColor, QPainter, QPainterPath, QLinearGradient, QPixmap, QIcon, QFont, QPen, QPolygonF

COLORS = ['#6aaabd', '#91ad79', '#6b9ac5', '#d4ac65', '#a193c0', '#c68f95', '#6aaba9', '#8097af']


@lru_cache(maxsize=8)
def folder_icon(index=0):
    pix = QPixmap(240, 170); pix.fill(Qt.GlobalColor.transparent)
    p = QPainter(pix); p.setRenderHint(QPainter.RenderHint.Antialiasing)
    c = QColor(COLORS[index % len(COLORS)])
    p.setPen(Qt.PenStyle.NoPen); p.setBrush(QColor(20, 40, 60, 16)); p.drawRoundedRect(QRectF(23, 44, 202, 118), 14, 14)
    p.setPen(QPen(c.darker(125), 1)); p.setBrush(c.darker(105)); p.drawRoundedRect(QRectF(22, 25, 76, 40), 8, 8); p.drawRoundedRect(QRectF(20, 40, 198, 112), 9, 9)
    for offset in (0, 10):
        p.setPen(QColor('#d3d3ca')); p.setBrush(QColor('#fffdf5')); p.drawRoundedRect(QRectF(36+offset, 27+offset, 163, 92), 3, 3)
    p.setPen(QPen(QColor('#a3a89e'), .8))
    for y in range(51, 75, 5): p.drawLine(64, y, 184, y)
    p.setFont(QFont('Serif', 20)); p.drawText(84, 72, '♪  ♫  ♪')
    g = QLinearGradient(0, 67, 0, 151); g.setColorAt(0, c.lighter(125)); g.setColorAt(1, c)
    path = QPainterPath(); path.moveTo(18, 68); path.quadTo(18, 60, 28, 60); path.lineTo(87, 60); path.lineTo(101, 72); path.lineTo(214, 72); path.quadTo(224, 72, 222, 83); path.lineTo(211, 146); path.quadTo(210, 155, 198, 155); path.lineTo(27, 155); path.quadTo(18, 155, 18, 146); path.closeSubpath()
    p.setPen(QPen(c.darker(125), 1.3)); p.setBrush(g); p.drawPath(path)
    p.setPen(QPen(QColor(255, 255, 255, 130), 1)); p.drawLine(30, 78, 210, 78)
    if index in (0,2,4):
        p.save(); p.setClipPath(path); p.setPen(Qt.PenStyle.NoPen)
        for j in range(2):
            p.setBrush(c.darker(110+j*18)); points=[QPointF(20,155),QPointF(30,140-j*8),QPointF(75,100+j*17),QPointF(106,127),QPointF(141,95+j*20),QPointF(200,137),QPointF(223,120),QPointF(223,158)]
            p.drawPolygon(QPolygonF(points))
        p.setBrush(QColor(255,255,255,140)); p.drawPolygon(QPolygonF([QPointF(65,112),QPointF(75,100),QPointF(88,113),QPointF(76,109)]))
        for x,y in ((39,123),(166,118),(182,133)):
            p.setBrush(c.darker(145)); p.drawPolygon(QPolygonF([QPointF(x,y-12),QPointF(x-9,y+7),QPointF(x+9,y+7)])); p.drawRect(QRectF(x-1,y,2,15))
        p.restore()
    p.setPen(QColor(255,255,255,185)); p.setFont(QFont('DejaVu Sans', 36)); p.drawText(QRectF(40,82,155,65), Qt.AlignmentFlag.AlignCenter, ['♩','♪','♫','♬','♫','♥','♫','≡'][index % 8]); p.end()
    return QIcon(pix)


@lru_cache(maxsize=256)
def cover_icon(title, level=1):
    seed = int(hashlib.sha256(title.encode()).hexdigest()[:8], 16)
    pix = QPixmap(260, 150); pix.fill(Qt.GlobalColor.transparent)
    p = QPainter(pix); p.setRenderHint(QPainter.RenderHint.Antialiasing)
    c = QColor(COLORS[(level-1) % 5]); g = QLinearGradient(0,0,0,150); g.setColorAt(0,c.darker(145)); g.setColorAt(1,c.lighter(160)); p.fillRect(pix.rect(), g)
    p.setPen(Qt.PenStyle.NoPen); p.setBrush(QColor(255,255,235,160)); p.drawEllipse(QPointF(180+seed%35, 32), 16,16)
    for j in range(3):
        p.setBrush(c.darker(105+j*25)); points=[QPointF(0,150),QPointF(0,100+j*15)]
        for x in range(0,281,40): points.append(QPointF(x,60+j*25+(seed//(x+1))%42))
        points.append(QPointF(260,150)); p.drawPolygon(QPolygonF(points))
    p.setPen(QColor(255,255,255,200)); p.setFont(QFont('Serif',42)); p.drawText(QRectF(15,65,80,72),Qt.AlignmentFlag.AlignCenter,'♫'); p.end(); return QIcon(pix)


@lru_cache(maxsize=24)
def medal_icon(symbol, unlocked):
    pix=QPixmap(160,160); pix.fill(Qt.GlobalColor.transparent)
    p=QPainter(pix); p.setRenderHint(QPainter.RenderHint.Antialiasing)
    c=QColor('#bd9346' if unlocked else '#8796a3'); g=QLinearGradient(0,20,0,140); g.setColorAt(0,c.lighter(165)); g.setColorAt(.5,c.lighter(115)); g.setColorAt(1,c)
    p.setBrush(g); p.setPen(QPen(c.darker(125),2)); p.drawEllipse(QRectF(23,17,114,114)); p.setBrush(Qt.BrushStyle.NoBrush); p.drawEllipse(QRectF(30,24,100,100)); p.setPen(c.darker(155)); p.setFont(QFont('DejaVu Sans',38,QFont.Weight.DemiBold)); p.drawText(QRectF(28,22,104,104),Qt.AlignmentFlag.AlignCenter,symbol)
    if not unlocked:
        p.setFont(QFont('DejaVu Sans',10)); p.drawText(QRectF(20,130,120,24),Qt.AlignmentFlag.AlignCenter,'Ещё впереди')
    p.end(); return QIcon(pix)
