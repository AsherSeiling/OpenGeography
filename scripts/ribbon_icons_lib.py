"""Drawing helpers and every ribbon icon (UR-23). Rules: docs/DECISIONS.md "Ribbon icons". 32 x 32 grid, dark outline (--ic-line), two or
three fills from the palette, white (--ic-paper) details; fragments marked D() are fine detail dropped at 16 px.
Fill keys: b/g/a/r/k = strong blue/green/amber/red/grey; bl/gl/al/rl/kl = light tones; p = paper; n = none."""
NAME = {'b': 'blue', 'g': 'green', 'a': 'amber', 'r': 'red', 'k': 'grey'}
def V(k):
    if k == 'n': return 'none'
    if k == 'p': return 'var(--ic-paper)'
    if k.endswith('d'): return f'var(--ic-{NAME[k[0]]}-d)'
    return f'var(--ic-{NAME[k[0]]}{"-l" if k.endswith("l") else ""})'
LINE = 'var(--ic-line)'
SW = 'var(--ic-sw)'
class D(str):
    pass
def _o(extra=''): return f'stroke="{LINE}" stroke-width="{SW}" stroke-linejoin="round" stroke-linecap="round"{extra}'
def R(x, y, w, h, f, rx=2, ex=''): return f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{rx}" fill="{V(f)}" {_o(ex)}/>'
def C(cx, cy, r, f, ex=''): return f'<circle cx="{cx}" cy="{cy}" r="{r}" fill="{V(f)}" {_o(ex)}/>'
def E(cx, cy, rx, ry, f, ex=''): return f'<ellipse cx="{cx}" cy="{cy}" rx="{rx}" ry="{ry}" fill="{V(f)}" {_o(ex)}/>'
def P(d, f, ex=''): return f'<path d="{d}" fill="{V(f)}" {_o(ex)}/>'
def LN(d, ex='', w=None, color=None):
    col = LINE if color is None else V(color)
    return f'<path d="{d}" fill="none" stroke="{col}" stroke-width="{w or SW}" stroke-linecap="round" stroke-linejoin="round"{ex}/>'
def W(d, w=2.2): return f'<path d="{d}" fill="none" stroke="{V("p")}" stroke-width="{w}" stroke-linecap="round" stroke-linejoin="round"/>'
def DASH(x, y, w, h, f, c): return f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="1.5" fill="{V(f)}" stroke="{V(c)}" stroke-width="2" stroke-dasharray="3.6 2.6"/>'
def THICK(d, color, w=1.8):
    """a coloured stroke with a dark casing (arrows, handles)"""
    return LN(d, w=w + 1.8) + f'<path d="{d}" fill="none" stroke="{V(color)}" stroke-width="{w}" stroke-linecap="round" stroke-linejoin="round"/>'
def PLUS(cx=23.5, cy=23.5, f='g', r=6.5): return C(cx, cy, r, f) + W(f'M{cx} {cy-3.3}v6.6M{cx-3.3} {cy}h6.6')
def MINUS(cx=23.5, cy=23.5, f='r', r=6.5): return C(cx, cy, r, f) + W(f'M{cx-3.3} {cy}h6.6')
def XB(cx=23.5, cy=23.5, f='r', r=6.5): return C(cx, cy, r, f) + W(f'M{cx-3.2} {cy-3.2}l6.4 6.4M{cx+3.2} {cy-3.2}l-6.4 6.4')
def CHECK(cx, cy, f='g', r=6.5): return C(cx, cy, r, f) + W(f'M{cx-3} {cy+.2}l2.3 2.4 4-4.6')
ARROW = 'M14 12v13.5l3.7-3.4 3 6.2 3-1.4-3-6h5z'
def CURSOR(dx=0, dy=0): return P(ARROW, 'p', f' transform="translate({dx} {dy})"')
DOCP = 'M7 3h13l6 6v20H7z'
def DOC(f='p'): return P(DOCP, f) + LN('M20 3v6h6')
def TXT(y, x1=10, x2=23): return LN(f'M{x1} {y}h{x2-x1}', ' stroke-opacity=".7"')
def SHEET(f='p', x=6, y=4, w=20, h=24): return R(x, y, w, h, f)
I = {}
def icon(name):
    def deco(fn):
        I[name] = fn()
        return fn
    return deco
# ---------------------------------------------------------------- the 12 approved on the sample sheet
@icon('add-data')
def _(): return [P('M4 8v13c0 2.2 4 4 9 4s9-1.8 9-4V8', 'bl'), E(13, 8, 9, 4, 'b'), D(LN('M4 14.5c0 2.2 4 4 9 4s9-1.8 9-4')), PLUS()]
@icon('basemap')
def _(): return [P('M3 8.5 11 5v18.5L3 27z', 'gl'), P('M11 5l10 3.5V27L11 23.5z', 'bl'), P('M21 8.5 29 5v18.5L21 27z', 'gl'), D(LN('M14 12c2 1 3 3 5 3M23.5 12.5c1.3.2 2.3 1 3 2.2', ' stroke-opacity=".75"')), D(C(7, 14, 2.2, 'a'))]
@icon('select')
def _(): return [DASH(4, 4, 19, 19, 'al', 'ad'), CURSOR()]
@icon('clear-selection')
def _(): return [DASH(4, 4, 19, 19, 'kl', 'kd'), XB(22, 22, 'r', 7.5)]
@icon('identify')
def _(): return [P('M3 10l8-4 9 3 9-3v16l-9 3-9-3-8 4z', 'gl'), D(LN('M11 6v16M20 9v16', ' stroke-opacity=".55"')), C(21.5, 11, 7.5, 'b'), W('M21.5 10v5.4', 2.4), f'<circle cx="21.5" cy="7.1" r="1.4" fill="{V("p")}"/>']
@icon('measure')
def _():
    ticks = ''.join(LN(f'M{x} 11v{5 if i % 2 == 0 else 3}') for i, x in enumerate(range(6, 30, 4)))
    return ['<g transform="rotate(-40 16 16)">' + R(1.5, 10.5, 29, 11, 'al', 1.8) + ticks + '</g>', D(C(6, 25, 2.4, 'b') + C(26, 7, 2.4, 'b'))]
@icon('zoom-layer')
def _(): return [C(13, 13, 9.5, 'bl'), P('M13 6.5l6 3.2-6 3.2-6-3.2z', 'g'), P('M7 13.4l6 3.2 6-3.2', 'g'), D(LN('M7 16.5l6 3.2 6-3.2')), THICK('M20 20l8 8', 'a', 2.4)]
@icon('attribute-table')
def _(): return [R(3.5, 5, 25, 22, 'p'), P('M3.5 7a2 2 0 0 1 2-2h21a2 2 0 0 1 2 2v4.5h-25z', 'b'), D(LN('M3.5 22.2h25M20.5 11.5V27')), LN('M3.5 17.5h25M12 11.5V27')]
@icon('processing-nodes')
def _(): return [LN('M10 9.5h6c3 0 3 13 6 13M10 22.5h4'), R(2.5, 4, 10, 9, 'b'), R(2.5, 19, 10, 9, 'g'), R(19.5, 11.5, 10, 9, 'a'), D(f'<circle cx="12.5" cy="8.5" r="1.4" fill="{LINE}"/><circle cx="19.5" cy="16" r="1.4" fill="{LINE}"/>')]
@icon('export')
def _(): return [D(LN('M4 14l3-5h9M28 14l-3-5h-4')), P('M3 14h26v12a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z', 'bl'), P('M3 14h8.5l1.5 3h6l1.5-3H29', 'b'), THICK('M16 15V3M10.5 8.5 16 3l5.5 5.5', 'g')]
@icon('coordinate-system')
def _(): return [C(16, 16, 12.5, 'bl'), f'<path d="M9 8.5c2.5-.5 4.5.5 4.5 3 0 2.5-3 2.5-3 5 0 1.8 1.5 2.5 1 5-3-1.5-6.5-4.5-6.5-9 0-1.8.6-3.2 1.5-4zM19 6.5c3 0 6 2.2 6.8 5.5-1.8.2-3-.6-4.4-.2-1.6.5-1.2 2.8-3.4 2.2-1.2-.4-1.2-3.5.6-5 .5-.6.4-2 .4-2.5z" fill="{V("g")}" stroke="{LINE}" stroke-width="1.1" stroke-linejoin="round"/>', D(LN('M16 3.5a5.3 12.5 0 0 1 0 25M16 3.5a5.3 12.5 0 0 0 0 25M3.5 16h25', ' stroke-opacity=".55"')), C(16, 16, 12.5, 'n')]
@icon('remove')
def _(): return [P('M7 11l1.6 16a2 2 0 0 0 2 1.8h10.8a2 2 0 0 0 2-1.8L25 11z', 'rl'), THICK('M3.5 8.5h25', 'r'), LN('M12.5 8V5.5a1.5 1.5 0 0 1 1.5-1.5h4a1.5 1.5 0 0 1 1.5 1.5V8'), D(LN('M12.5 15v9M16 15v9M19.5 15v9', ' stroke-opacity=".7"'))]
# ---------------------------------------------------------------- the rest
@icon('globe')
def _(): return [C(16, 16, 12.5, 'bl'), f'<path d="M9 8.5c2.5-.5 4.5.5 4.5 3 0 2.5-3 2.5-3 5 0 1.8 1.5 2.5 1 5-3-1.5-6.5-4.5-6.5-9 0-1.8.6-3.2 1.5-4zM19 6.5c3 0 6 2.2 6.8 5.5-1.8.2-3-.6-4.4-.2-1.6.5-1.2 2.8-3.4 2.2-1.2-.4-1.2-3.5.6-5 .5-.6.4-2 .4-2.5z" fill="{V("g")}" stroke="{LINE}" stroke-width="1.1" stroke-linejoin="round"/>', C(16, 16, 12.5, 'n')]
@icon('world-outline')
def _(): return [C(16, 16, 12.5, 'p'), P('M9 8.5c2.5-.5 4.5.5 4.5 3 0 2.5-3 2.5-3 5 0 1.8 1.5 2.5 1 5-3-1.5-6.5-4.5-6.5-9 0-1.8.6-3.2 1.5-4zM19 6.5c3 0 6 2.2 6.8 5.5-1.8.2-3-.6-4.4-.2-1.6.5-1.2 2.8-3.4 2.2-1.2-.4-1.2-3.5.6-5 .5-.6.4-2 .4-2.5z', 'n', ' stroke-width="1.3"')]
@icon('graticule')
def _(): return [C(16, 16, 12.5, 'bl'), LN('M3.5 16h25M16 3.5v25'), LN('M16 3.5a6 12.5 0 0 1 0 25M16 3.5a6 12.5 0 0 0 0 25'), D(LN('M5.5 9.5h21M5.5 22.5h21')), C(16, 16, 12.5, 'n')]
@icon('full-extent')
def _(): return [R(6, 6, 20, 20, 'gl', 1), C(16, 16, 6, 'bl'), D(LN('M10 16h12M16 10v12', ' stroke-opacity=".6"')), LN('M3 11V4h7M22 4h7v7M29 21v7h-7M10 28H3v-7', w=2.4)]
@icon('define-projection')
def _(): return [C(14, 15, 11.5, 'bl'), P('M8 8c2-.4 4 .4 4 2.6 0 2.2-2.6 2.2-2.6 4.4 0 1.6 1.3 2.2.9 4.4-2.6-1.3-5.7-4-5.7-8 0-1.6.5-2.8 1.4-3.4z', 'g', ' stroke-width="1.1"'), C(14, 15, 11.5, 'n'), P('M25 12c-3.2 0-5.4 2.4-5.4 5.4 0 4 5.4 10.6 5.4 10.6s5.4-6.6 5.4-10.6c0-3-2.2-5.4-5.4-5.4z', 'r'), C(25, 17.4, 1.9, 'p')]
@icon('properties')
def _(): return [P('M13.5 3h5l.8 3.4 2.2 1 3-1.9 3.5 3.5-1.9 3 1 2.2 3.4.8v5l-3.4.8-1 2.2 1.9 3-3.5 3.5-3-1.9-2.2 1-.8 3.4h-5l-.8-3.4-2.2-1-3 1.9-3.5-3.5 1.9-3-1-2.2L3 18.5v-5l3.4-.8 1-2.2-1.9-3 3.5-3.5 3 1.9 2.2-1z', 'k'), C(16, 16, 5, 'bl')]
@icon('projection-builder')
def _(): return [C(13, 14, 10.5, 'bl'), LN('M2.5 14h21M13 3.5v21'), LN('M13 3.5a5 10.5 0 0 1 0 21M13 3.5a5 10.5 0 0 0 0 21', ' stroke-opacity=".6"'), C(13, 14, 10.5, 'n'), P('M22.5 15.5l1.8-.2.7 1.5 1.6-.2.7 1.5-1 1.2.7 1.5-1.2 1 .2 1.7-1.5.7-.2 1.8-1.7-.2-1.1 1.3-1.4-1-1.6.4-.9-1.5-1.5-.5.2-1.8-1.1-1.3.9-1.5-.3-1.6 1.5-.9.5-1.6 1.7.2z', 'k', ' stroke-width="1.2"'), C(24.4, 22.5, 2.2, 'a', ' stroke-width="1"')]
@icon('map')
def _(): return [P('M3 8.5 11 5v18.5L3 27z', 'gl'), P('M11 5l10 3.5V27L11 23.5z', 'bl'), P('M21 8.5 29 5v18.5L21 27z', 'gl')]
@icon('map-add')
def _(): return [P('M3 8.5 11 5v18.5L3 27z', 'gl'), P('M11 5l10 3.5V27L11 23.5z', 'bl'), P('M21 8.5 29 5v18.5L21 27z', 'gl'), PLUS()]
@icon('map-frame')
def _(): return [R(3, 5, 26, 22, 'p', 1.5), R(6.5, 8.5, 19, 15, 'gl', 1), P('M6.5 19l5-4 4 3 4-5 6 4v6.5h-19z', 'g', ' stroke-width="1.2"'), D(C(21, 12.5, 1.8, 'a', ' stroke-width="1"'))]
@icon('overview')
def _(): return [R(3, 5, 26, 22, 'gl', 1.5), R(15, 11, 11, 11, 'p', 1), R(17.5, 13.5, 6, 6, 'al', 0.5, ' stroke-width="1.2"'), D(LN('M5 22l6-7 4 4', ' stroke-opacity=".6"'))]
@icon('frame-from-map')
def _(): return [R(3, 4, 26, 24, 'gl', 1.5), C(16, 16, 7, 'p'), C(16, 16, 2.4, 'r', ' stroke-width="1.2"'), LN('M16 5.5v5.5M16 21v5.5M4.5 16H10M22 16h5.5')]
@icon('topo-map')
def _(): return [P('M2 26L11 8l5 8 4-5 10 15z', 'g'), P('M9 12l2-4 2.4 3.8-2.4 1.4-1.6 1.2z', 'p', ' stroke-width="1.2"'), D(LN('M3 28c6-4 12-1 18-4s7-3 9-4', ' stroke-dasharray="2.2 2.4" stroke-width="1.5" stroke-opacity=".85"')), C(24.5, 8, 2.8, 'a')]
@icon('folder-open')
def _(): return [P('M3 8a2 2 0 0 1 2-2h7l3 3h10a2 2 0 0 1 2 2v3H3z', 'al'), P('M3 27l3.2-12a2 2 0 0 1 1.9-1.5H29l-3.4 12.4a2 2 0 0 1-1.9 1.6z', 'a')]
@icon('catalog')
def _(): return [P('M3 8a2 2 0 0 1 2-2h7l3 3h12a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z', 'al'), D(R(8, 13, 7, 5, 'bl', 1, ' stroke-width="1.2"') + R(17, 13, 7, 5, 'gl', 1, ' stroke-width="1.2"')), R(8, 19.5, 7, 4, 'gl', 1, ' stroke-width="1.2"'), R(17, 19.5, 7, 4, 'bl', 1, ' stroke-width="1.2"')]
@icon('new')
def _(): return [DOC(), D(TXT(14) + TXT(18) + TXT(22)), PLUS(22.5, 23.5)]
@icon('save')
def _(): return [P('M4 6a2 2 0 0 1 2-2h17l5 5v17a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2z', 'b'), R(9, 4, 12, 8, 'p', 1), R(8, 17, 16, 11, 'p', 1.2), D(LN('M11 21h10M11 24.5h7', ' stroke-opacity=".6"')), R(16.5, 5.5, 3, 5, 'b', 0.5, ' stroke-width="1.1"')]
@icon('save-as')
def _(): return [P('M4 6a2 2 0 0 1 2-2h17l5 5v17a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2z', 'b'), R(9, 4, 12, 8, 'p', 1), R(8, 17, 14, 11, 'p', 1.2), THICK('M17 27.5l1-4 8-8 3 3-8 8z', 'a', 1.6)]
@icon('open-project')
def _(): return [P('M3 8a2 2 0 0 1 2-2h7l3 3h10a2 2 0 0 1 2 2v3H3z', 'bl'), P('M3 27l3.2-12a2 2 0 0 1 1.9-1.5H29l-3.4 12.4a2 2 0 0 1-1.9 1.6z', 'b')]
@icon('table-new')
def _(): return [R(3.5, 5, 22, 19, 'p'), P('M3.5 7a2 2 0 0 1 2-2h18a2 2 0 0 1 2 2v4h-22z', 'b'), LN('M3.5 16h22M11 11v13'), D(LN('M18 11v13')), PLUS(24, 24)]
@icon('table-add-col')
def _(): return [R(3.5, 5, 22, 22, 'p'), P('M3.5 7a2 2 0 0 1 2-2h18a2 2 0 0 1 2 2v4h-22z', 'b'), LN('M3.5 17h22M11 11v16'), R(18.5, 11, 7, 16, 'gl', 0.5, ' stroke-width="1.2"'), PLUS(25, 24, 'g', 6)]
@icon('people')
def _(): return [C(11, 10, 4.5, 'b'), P('M2.5 27c0-5.2 3.8-8.5 8.5-8.5s8.5 3.3 8.5 8.5z', 'bl'), C(22, 12, 4, 'g'), P('M17.5 20c1.3-.8 2.7-1.2 4.5-1.2 4 0 7.5 2.8 7.5 8.2H20.5', 'gl')]
@icon('select-attributes')
def _(): return [R(3.5, 5, 25, 22, 'p'), P('M3.5 7a2 2 0 0 1 2-2h21a2 2 0 0 1 2 2v4.5h-25z', 'b'), R(4.4, 13, 23.2, 5, 'al', 0, ' stroke="none"'), LN('M3.5 12.5h25M3.5 17.5h25M3.5 22.5h25M12 11.5V27M20.5 11.5V27'), CURSOR(4, 4)]
@icon('select-box')
def _(): return [DASH(4, 6, 23, 17, 'al', 'ad'), CURSOR(1, 4), D(f'<circle cx="4" cy="6" r="1.8" fill="{V("ad")}"/><circle cx="27" cy="23" r="1.8" fill="{V("ad")}"/>')]
@icon('select-location')
def _(): return [DASH(3, 5, 17, 17, 'al', 'ad'), P('M21 10c-3.8 0-6.5 2.8-6.5 6.4 0 4.8 6.5 12.6 6.5 12.6s6.5-7.8 6.5-12.6c0-3.6-2.7-6.4-6.5-6.4z', 'r'), C(21, 16.4, 2.4, 'p')]
@icon('zoom-selection')
def _(): return [DASH(3, 3, 14, 14, 'al', 'ad'), C(14, 14, 9, 'n', ' stroke-opacity="0"'), C(15, 15, 9.5, 'bl'), DASH(10, 10, 10, 10, 'al', 'ad'), THICK('M22 22l7 7', 'a', 2.4)]
@icon('zoom-actual')
def _(): return [C(13, 13, 9.5, 'bl'), W('M10.5 11.5l2.5-1.8v7.6'), THICK('M20 20l8 8', 'a', 2.4)]
@icon('zoom-in')
def _(): return [C(13, 13, 9.5, 'bl'), PLUS(13, 13, 'g', 5.2), THICK('M20 20l8 8', 'a', 2.4)]
@icon('zoom-out')
def _(): return [C(13, 13, 9.5, 'bl'), MINUS(13, 13, 'r', 5.2), THICK('M20 20l8 8', 'a', 2.4)]
@icon('fit-page')
def _(): return [R(8, 5, 16, 22, 'p', 1), D(LN('M12 11h8M12 15h8M12 19h5', ' stroke-opacity=".6"')), LN('M3 11V4h6M23 4h6v7M29 21v7h-6M9 28H3v-7', w=2.4)]
@icon('reset-panes')
def _(): return [R(3, 5, 26, 20, 'p', 1.5), R(3, 5, 8, 20, 'bl', 1.5), R(11, 5, 18, 7, 'gl', 0.5), P('M22 28.5a6.5 6.5 0 1 1 5.5-9.7', 'n', ' stroke-width="3.6"'), THICK('M22 28.5a6.5 6.5 0 1 1 5.5-9.7M29 15.5l-.4 4.6-4.5-.9', 'g', 1.8)]
@icon('layout-guide-h')
def _(): return [R(7, 3, 18, 26, 'p', 1.5), f'<path d="M2 16h28" fill="none" stroke="{V("ad")}" stroke-width="2" stroke-dasharray="3.4 2.4" stroke-linecap="round"/>', D(LN('M10 8h12M10 22h12', ' stroke-opacity=".5"'))]
@icon('layout-guide-v')
def _(): return [R(3, 7, 26, 18, 'p', 1.5), f'<path d="M16 2v28" fill="none" stroke="{V("ad")}" stroke-width="2" stroke-dasharray="3.4 2.4" stroke-linecap="round"/>', D(LN('M8 10v12M24 10v12', ' stroke-opacity=".5"'))]
@icon('clear-guides')
def _(): return [R(5, 3, 17, 24, 'p', 1.5), f'<path d="M2 15h24" fill="none" stroke="{V("ad")}" stroke-width="2" stroke-dasharray="3.4 2.4" stroke-linecap="round"/>', XB(23, 23, 'r', 7)]
@icon('layout-arrow')
def _(): return [P('M3 12h15V6l11 10-11 10v-6H3z', 'b')]
@icon('layout-line')
def _(): return [LN('M5 26L27 6', w=4.2), THICK('M5 26L27 6', 'b', 2), D(C(5, 26, 2.2, 'p') + C(27, 6, 2.2, 'p'))]
@icon('rectangle')
def _(): return [R(3, 7, 26, 18, 'b', 2.5)]
@icon('ellipse')
def _(): return [E(16, 16, 13, 9.5, 'g')]
@icon('triangle')
def _(): return [P('M16 4.5L29 27H3z', 'a')]
@icon('shapes')
def _(): return [C(10, 10, 6.5, 'b'), R(15, 15, 13, 13, 'g', 2), P('M8 29L3 20h10z', 'a')]
@icon('svg')
def _(): return [DOC(), P('M10 24l3.4-7 3.2 4 2-2.4 3.4 5.4z', 'g', ' stroke-width="1.2"'), C(13, 12, 2.2, 'a', ' stroke-width="1.1"')]
@icon('layout-text')
def _(): return [R(3, 4, 26, 24, 'p', 1.5), P('M9 9h14v3.2h-1.8V10.6h-4.3v12.2h2.3V25h-7.6v-2.2h2.3V10.6H10.8v1.6H9z', 'b', ' stroke-width="1.2"')]
@icon('layout-picture-add')
def _(): return [R(3, 5, 25, 20, 'bl', 1.5), P('M3 21l7-7 5 5 4-4 9 8v2a1.5 1.5 0 0 1-1.5 1.5h-21A1.5 1.5 0 0 1 3 25z', 'g', ' stroke-width="1.2"'), C(20, 11, 3, 'a'), PLUS(24.5, 24.5, 'g', 6)]
@icon('image')
def _(): return [R(3, 5, 26, 22, 'bl', 1.5), P('M3 22l7-8 5 6 4-4 10 9v1.5a1.5 1.5 0 0 1-1.5 1.5h-23A1.5 1.5 0 0 1 3 25.5z', 'g', ' stroke-width="1.2"'), C(21.5, 11.5, 3, 'a')]
@icon('pdf')
def _(): return [DOC(), R(4, 15, 18, 9, 'r', 1.5), W('M7.5 21.2v-3.4h1.6a1.2 1.2 0 0 1 0 2.4H7.5M11.5 17.8v3.4h1.1a1.7 1.7 0 0 0 0-3.4zM16 21.2v-3.4h2.4M16 19.5h1.8', 1.3)]
@icon('new-layout')
def _(): return [DOC(), D(R(10, 13, 7, 5, 'bl', 0.5, ' stroke-width="1.2"')), D(R(10, 20, 12, 4, 'gl', 0.5, ' stroke-width="1.2"')), PLUS(22.5, 23.5)]
@icon('layout-lock')
def _(): return [P('M10 14v-4a6 6 0 0 1 12 0v4', 'n', ' stroke-width="3"'), P('M10 14v-4a6 6 0 0 1 12 0v4', 'n'), R(5.5, 14, 21, 15, 'a', 2.5), C(16, 20.5, 2.2, 'p', ' stroke-width="1.2"'), LN('M16 22.5v3', w=2.2)]
@icon('layout-group')
def _(): return [f'<rect x="2.5" y="2.5" width="27" height="27" rx="2" fill="none" stroke="{V("bd") if False else V("kd")}" stroke-width="1.8" stroke-dasharray="3.6 2.6"/>', R(6.5, 7, 11, 10, 'b', 1.5), R(14.5, 15.5, 11, 10, 'g', 1.5)]
@icon('layout-ungroup')
def _(): return [R(3, 3, 12, 11, 'b', 1.5), R(17, 18, 12, 11, 'g', 1.5), THICK('M19 9l6 0M22.5 6.5L25 9l-2.5 2.5', 'a', 1.5)]
@icon('layout-legend')
def _(): return [R(3, 3, 26, 26, 'p', 1.5), R(6.5, 7, 6, 4.5, 'b', 1, ' stroke-width="1.2"'), R(6.5, 14, 6, 4.5, 'g', 1, ' stroke-width="1.2"'), R(6.5, 21, 6, 4.5, 'a', 1, ' stroke-width="1.2"'), LN('M15.5 9.2H26M15.5 16.2H26M15.5 23.2H23', ' stroke-opacity=".75"')]
@icon('layout-chart')
def _(): return [R(3, 3, 26, 26, 'p', 1.5), R(7, 16, 5, 9.5, 'b', 0.8, ' stroke-width="1.2"'), R(13.5, 9, 5, 16.5, 'g', 0.8, ' stroke-width="1.2"'), R(20, 13, 5, 12.5, 'a', 0.8, ' stroke-width="1.2"'), D(LN('M5.5 25.5h21', ' stroke-opacity=".8"'))]
@icon('layout-sources')
def _(): return [P('M5 4h18a2 2 0 0 1 2 2v21H7.5A2.5 2.5 0 0 1 5 24.5z', 'bl'), P('M5 24.5A2.5 2.5 0 0 1 7.5 22H25v5H7.5A2.5 2.5 0 0 1 5 24.5z', 'b'), C(15, 12, 5, 'p'), LN('M15 11.2v4.2', w=2), f'<circle cx="15" cy="8.9" r="1.1" fill="{LINE}"/>']
@icon('info')
def _(): return [C(16, 16, 12.5, 'b'), W('M16 14.5v8', 2.8), f'<circle cx="16" cy="9.8" r="1.8" fill="{V("p")}"/>']
@icon('north-arrow')
def _(): return [C(16, 16, 12.5, 'p'), P('M16 3.5l6 15-6-2.6-6 2.6z', 'b'), P('M16 29l-6-9.5 6 2.6 6-2.6z', 'kl')]
@icon('declination')
def _(): return [C(16, 16, 12.5, 'p'), P('M16 3.5l4.5 12.5h-9z', 'b'), P('M16 28.5L11.5 16h9z', 'kl'), THICK('M16 16L25 7', 'r', 1.4), C(16, 16, 2, 'p')]
@icon('scale-bar')
def _(): return [R(3, 13, 26, 7, 'p', 1), R(3, 13, 6.5, 7, 'k', 0), R(16, 13, 6.5, 7, 'k', 0), R(3, 13, 26, 7, 'n', 1), D(LN('M3 9v4M9.5 9v4M16 9v4M22.5 9v4M29 9v4')), D(LN('M3 25h26', ' stroke-opacity=".6"'))]
@icon('calc-geometry')
def _(): return [P('M4 22l6-15 12 6-4 15z', 'gl'), THICK('M6 29L28 29M6 27v4M28 27v4', 'a', 1.3), D(C(10, 7, 2, 'b', ' stroke-width="1"')), D(C(22, 13, 2, 'b', ' stroke-width="1"'))]
@icon('measure-area')
def _(): return [P('M4 20l5-13 13-3 6 14-9 9z', 'al'), C(9, 7, 2.2, 'b', ' stroke-width="1.2"'), C(22, 4, 2.2, 'b', ' stroke-width="1.2"'), C(28, 18, 2.2, 'b', ' stroke-width="1.2"'), C(19, 27, 2.2, 'b', ' stroke-width="1.2"'), C(4, 20, 2.2, 'b', ' stroke-width="1.2"')]
@icon('calc-field')
def _(): return [R(6, 3, 20, 26, 'k', 2.5), R(9, 6, 14, 6, 'bl', 1, ' stroke-width="1.2"'), C(11.5, 17, 1.8, 'p', ' stroke-width="1"'), C(16, 17, 1.8, 'p', ' stroke-width="1"'), C(20.5, 17, 1.8, 'p', ' stroke-width="1"'), C(11.5, 22.5, 1.8, 'p', ' stroke-width="1"'), C(16, 22.5, 1.8, 'p', ' stroke-width="1"'), C(20.5, 22.5, 1.8, 'a', ' stroke-width="1"')]
@icon('definition-query')
def _(): return [P('M3 5h26l-9.5 11.5V26l-7 3V16.5z', 'b'), D(LN('M8 9.5h16', ' stroke-opacity=".5"')), C(24, 24, 5.5, 'a'), W('M22 24h4', 2)]
@icon('layer-add')
def _(): return [P('M16 4l13 6.5L16 17 3 10.5z', 'b'), P('M3 15.5l13 6.5 13-6.5', 'n'), P('M3 20.5l13 6.5 4-2', 'n'), PLUS(24, 24, 'g', 6)]
@icon('split')
def _(): return [R(3, 4, 10, 10, 'b', 1.5), R(18, 4, 11, 10, 'g', 1.5), R(10, 18, 12, 10, 'a', 1.5), LN('M13 14l3 4M19 14l-3 4', ' stroke-dasharray="2 2"')]
@icon('intersect')
def _(): return [C(12, 16, 9, 'bl'), C(20, 16, 9, 'gl'), f'<path d="M16 8.2a9 9 0 0 1 0 15.6 9 9 0 0 1 0-15.6z" fill="{V("a")}" stroke="{LINE}" stroke-width="{SW}" stroke-linejoin="round"/>', C(12, 16, 9, 'n'), C(20, 16, 9, 'n')]
@icon('usng-grid')
def _(): return [R(3, 3, 26, 26, 'gl', 1.5), LN('M3 11.7h26M3 20.3h26M11.7 3v26M20.3 3v26'), R(11.7, 11.7, 8.6, 8.6, 'a', 0.5)]
@icon('tissot')
def _(): return [R(3, 3, 26, 26, 'bl', 1.5), E(10, 10, 4, 3, 'a', ' stroke-width="1.3"'), E(22, 10, 4, 4.5, 'a', ' stroke-width="1.3"'), E(10, 22, 4.5, 4, 'a', ' stroke-width="1.3"'), E(22, 22, 3.5, 5, 'a', ' stroke-width="1.3"'), D(LN('M3 16h26M16 3v26', ' stroke-opacity=".5"'))]
@icon('datum-grids')
def _(): return [P('M4 8v16c0 2.2 5 4 12 4s12-1.8 12-4V8', 'kl'), E(16, 8, 12, 4, 'k'), LN('M4 16c0 2.2 5 4 12 4s12-1.8 12-4'), D(LN('M10 11.5v13M16 12.5v14M22 11.5v13', ' stroke-opacity=".45"'))]
@icon('test-data')
def _(): return [C(10, 11, 3, 'b'), C(22, 8, 3, 'g'), C(18, 19, 3, 'a'), C(8, 23, 3, 'r'), C(25, 24, 3, 'b'), D(LN('M3 29h26', ' stroke-opacity=".5"'))]
@icon('palette')
def _(): return [P('M16 3.5C8.3 3.5 3 9 3 15.5c0 6.5 5 12 11 12 2.2 0 2.6-1.8 1.6-3.2-1-1.5-.2-3.3 1.7-3.3H24c3 0 5-2 5-4.8C29 8 23.5 3.5 16 3.5z', 'p'), C(9.5, 14, 2.2, 'r', ' stroke-width="1.1"'), C(13, 8.7, 2.2, 'a', ' stroke-width="1.1"'), C(19.8, 8.4, 2.2, 'g', ' stroke-width="1.1"'), C(24, 13.2, 2.2, 'b', ' stroke-width="1.1"')]
@icon('symbology')
def _(): return [THICK('M4 28c4 0 6-1.6 6-4.5 0-2-1.5-3-3.2-3C4.6 20.5 4 22.7 4 24.5', 'b', 1.8), P('M10 20l14-15a2 2 0 0 1 3 3L13 23z', 'a'), D(C(5, 27, 1.8, 'b', ' stroke-width="1"'))]
@icon('theme')
def _(): return [C(16, 16, 12.5, 'p'), '<path d="M16 3.5a12.5 12.5 0 0 0 0 25z" fill="var(--ic-line)"/>', C(16, 16, 12.5, 'n'), D(C(20.5, 11, 1.8, 'a', ' stroke-width="1"'))]
@icon('fps')
def _(): return [P('M3 24a13 13 0 1 1 26 0z', 'bl'), LN('M6.8 20.5l1.8-1M10 12.8l1.6 1.2M16 8v2M22 12.8l-1.6 1.2M25.2 20.5l-1.8-1'), THICK('M16 24l6.5-9', 'r', 1.6), C(16, 24, 3, 'k')]
@icon('run-model')
def _(): return [P('M8 4.5v23l19-11.5z', 'g')]
@icon('add-tool')
def _(): return [R(3, 3, 26, 26, 'bl', 3), R(3, 3, 26, 8, 'b', 3), W('M16 15v11M10.5 20.5h11', 2.6)]
@icon('duplicate')
def _(): return [P('M9 8a2 2 0 0 1 2-2h13l4 4v14a2 2 0 0 1-2 2H11a2 2 0 0 1-2-2z', 'bl'), P('M4 13a2 2 0 0 1 2-2h9l3 3v13a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2z', 'p'), D(LN('M7 18h8M7 22h8', ' stroke-opacity=".6"'))]
@icon('export-table')
def _(): return [R(3, 5, 19, 18, 'p'), P('M3 7a2 2 0 0 1 2-2h15a2 2 0 0 1 2 2v3.5H3z', 'b'), LN('M3 16h19M10 10.5v12.5'), THICK('M16 24l6-4.5M22 19.5l-1.5 5M22 19.5l-5-.8', 'g', 1.6), THICK('M17 27h10M24 24l3 3-3 3', 'g', 1.8)]
@icon('preflight')
def _(): return [R(4, 3, 24, 26, 'p', 2), CHECK(10.5, 9.5, 'g', 3.6), CHECK(10.5, 16.5, 'g', 3.6), C(10.5, 23.5, 3.6, 'a'), LN('M17 9.5h8M17 16.5h8M17 23.5h6', ' stroke-opacity=".75"')]
@icon('explore')
def _(): return [THICK('M16 3v26M3 16h26', 'b', 1.8), THICK('M12 7l4-4 4 4M12 25l4 4 4-4M7 12l-4 4 4 4M25 12l4 4-4 4', 'b', 1.8), C(16, 16, 3.2, 'p')]
@icon('resection')
def _(): return [C(16, 16, 10, 'p'), C(16, 16, 4.2, 'r'), LN('M16 2v8M16 22v8M2 16h8M22 16h8')]
@icon('page-setup')
def _(): return [R(6, 3, 17, 24, 'p', 1.5), P('M20 17l1.6-.2.6 1.4 1.5-.2.6 1.4-.9 1.1.6 1.4-1.1.9.2 1.6-1.4.6-.2 1.6-1.6-.2-1-1.2-1.3.9-1.4-.9-1.5.4-.8-1.4-1.4-.4.2-1.6-1-1.2.9-1.4-.3-1.5 1.4-.8.5-1.5 1.5.2z', 'k', ' stroke-width="1.2"'), C(22.5, 23, 2.2, 'p', ' stroke-width="1"'), D(LN('M10 9h9M10 13h9', ' stroke-opacity=".5"'))]
@icon('orientation')
def _(): return [R(3, 9, 18, 14, 'bl', 1.5), THICK('M22 8a8 8 0 0 1 6 8M25.5 17.5L28 16l1.5-3', 'g', 1.6), R(22, 19, 7, 9, 'p', 1, ' stroke-width="1.2"')]
@icon('series')
def _(): return [R(9, 6, 17, 22, 'bl', 1.5), R(6, 4, 17, 22, 'gl', 1.5), R(3, 2, 17, 22, 'p', 1.5), D(LN('M7 8h9M7 12h9M7 16h6', ' stroke-opacity=".6"'))]
@icon('template')
def _(): return [DOC(), R(10, 12, 12, 4, 'b', 0.5, ' stroke-width="1.1"'), R(10, 18, 5, 6, 'g', 0.5, ' stroke-width="1.1"'), R(17, 18, 5, 6, 'a', 0.5, ' stroke-width="1.1"')]
@icon('order')
def _(): return [R(3, 3, 18, 16, 'bl', 1.5), R(11, 13, 18, 16, 'g', 1.5)]
@icon('align')
def _(): return [LN('M4 3v26', w=2.2), R(7, 6, 18, 6, 'b', 1), R(7, 15, 12, 6, 'g', 1), R(7, 24, 21, 4, 'a', 1)]
@icon('distribute')
def _(): return [LN('M3 3h26M3 29h26', w=2), R(5, 9, 22, 4, 'b', 1), R(5, 19, 22, 4, 'g', 1)]
@icon('same-size')
def _(): return [R(3, 4, 13, 11, 'b', 1.5), R(3, 18, 13, 11, 'g', 1.5), THICK('M20 9h9M20 24h9M24.5 4v10M24.5 19v10', 'a', 1.2)]
@icon('undo')
def _(): return [THICK('M6 12h13a7 7 0 0 1 0 14h-9', 'b', 2.2), THICK('M11 6.5L5.5 12 11 17.5', 'b', 2.2)]
@icon('redo')
def _(): return [THICK('M26 12H13a7 7 0 0 0 0 14h9', 'b', 2.2), THICK('M21 6.5L26.5 12 21 17.5', 'b', 2.2)]
@icon('back')
def _(): return [THICK('M27 16H6M13 8.5L5.5 16 13 23.5', 'b', 2.4)]
@icon('series-export')
def _(): return [R(9, 6, 17, 22, 'bl', 1.5), R(3, 2, 17, 22, 'p', 1.5), THICK('M22 19l6 6M22 25h6v-6', 'g', 1.6)]
@icon('frame-crs')
def _(): return [R(3, 5, 22, 20, 'p', 1.5), C(14, 15, 7, 'bl'), P('M10 11c1.5-.3 3 .3 3 2s-2 1.7-2 3.4c0 1.2 1 1.6.6 3.3-2-1-4.4-3-4.4-6 0-1.2.4-2 .8-2.7z', 'g', ' stroke-width="1"'), C(14, 15, 7, 'n'), C(24, 24, 5.5, 'a'), LN('M24 21.5v5M21.5 24h5', w=1.6)]
@icon('pane-layers')
def _(): return [R(3, 4, 26, 24, 'p', 2), R(3, 4, 11, 24, 'bl', 2), P('M8.5 9l4 2-4 2-4-2z', 'b', ' stroke-width="1.2"'), P('M4.5 14.5l4 2 4-2', 'n', ' stroke-width="1.2"'), P('M4.5 18l4 2 4-2', 'n', ' stroke-width="1.2"'), D(LN('M18 10h8M18 15h8M18 20h5', ' stroke-opacity=".6"'))]
@icon('pane-items')
def _(): return [R(3, 4, 26, 24, 'p', 2), R(3, 4, 11, 24, 'gl', 2), LN('M6 10h5M6 15h5M6 20h5'), D(R(18, 9, 8, 6, 'bl', 1, ' stroke-width="1.2"')), D(R(18, 18, 8, 5, 'al', 1, ' stroke-width="1.2"'))]
@icon('pane-properties')
def _(): return [R(3, 4, 26, 24, 'p', 2), R(18, 4, 11, 24, 'kl', 2), LN('M6 10h8M6 15h8M6 20h5', ' stroke-opacity=".6"'), C(23.5, 11, 2.4, 'b', ' stroke-width="1.2"'), D(LN('M20.5 17h6M20.5 21h6', ' stroke-opacity=".7"'))]
@icon('book')
def _(): return [P('M5 4h18a2 2 0 0 1 2 2v21H7.5A2.5 2.5 0 0 1 5 24.5z', 'bl'), P('M5 24.5A2.5 2.5 0 0 1 7.5 22H25v5H7.5A2.5 2.5 0 0 1 5 24.5z', 'b'), D(LN('M10 9h10M10 13h10M10 17h6', ' stroke-opacity=".7"'))]
@icon('page')
def _(): return [DOC(), D(TXT(14) + TXT(18) + TXT(22))]
@icon('settings')
def _(): return [LN('M4 9h24M4 16h24M4 23h24', w=2), C(11, 9, 3.4, 'b'), C(21, 16, 3.4, 'g'), C(13, 23, 3.4, 'a')]
@icon('startup-layout')
def _(): return [R(3, 5, 26, 22, 'p', 2), R(3, 5, 9, 22, 'bl', 2), R(12, 5, 17, 7, 'gl', 0.5), C(24, 22, 6, 'a'), W('M24 18.5l1.2 2.4 2.6.4-1.9 1.8.5 2.6-2.4-1.3-2.4 1.3.5-2.6-1.9-1.8 2.6-.4z', 1.2)]
# ---------------------------------------------------------------- shape editing (E2)
@icon('edit-start')
def _(): return [P('M5 24l3-1 15-15-2-2L6 21z', 'a'), P('M21 6l2 2 2-2-2-2z', 'r'), LN('M5 27h22', w=2), D(LN('M8 23l-1 3 3-1'))]
@icon('edit-save')
def _(): return [P('M5 4h18l4 4v20H5z', 'b'), R(9, 4, 12, 8, 'p', 1), R(9, 18, 14, 10, 'bl', 1), D(LN('M17 6v4'))]
@icon('edit-discard')
def _(): return [P('M5 4h18l4 4v20H5z', 'kl'), R(9, 4, 12, 8, 'p', 1), XB(22, 22, 'r', 7.5)]
@icon('edit-point')
def _(): return [C(16, 16, 6, 'b'), C(16, 16, 2, 'p'), PLUS(24, 24, 'g', 6)]
@icon('edit-line')
def _(): return [LN('M5 24L13 10l8 9 7-12', w=2.6), C(5, 24, 2.6, 'b'), C(13, 10, 2.6, 'b'), C(21, 19, 2.6, 'b'), C(28, 7, 2.6, 'b')]
@icon('edit-polygon')
def _(): return [P('M6 22L10 8l14 3 3 12z', 'bl'), C(6, 22, 2.4, 'b'), C(10, 8, 2.4, 'b'), C(24, 11, 2.4, 'b'), C(27, 23, 2.4, 'b'), PLUS(24, 24, 'g', 5.5)]
@icon('edit-vertex')
def _(): return [P('M6 24L9 9l15 4 2 11z', 'gl'), C(9, 9, 2.4, 'p'), C(24, 13, 3.6, 'a'), THICK('M18 6l6 6', 'a', 1.4), C(6, 24, 2.4, 'p'), C(26, 24, 2.4, 'p')]
@icon('edit-delete')
def _(): return [P('M6 24L9 9l15 4 2 11z', 'kl'), XB(22, 10, 'r', 7)]
@icon('edit-snap')
def _(): return [LN('M5 22h22', w=2), C(16, 22, 3, 'b'), P('M9 6v8a7 7 0 0 0 14 0V6h-4v8a3 3 0 0 1-6 0V6z', 'r'), R(9, 6, 4, 3, 'p', 0), R(19, 6, 4, 3, 'p', 0)]
@icon('edit-fix')
def _(): return [P('M6 22L10 8l14 3 3 12z', 'rl'), LN('M8 10l17 11M25 10L8 21', ' stroke-opacity=".6"'), CHECK(24, 24, 'g', 6)]
@icon('layer-new')
def _(): return [P('M16 4l13 6.5L16 17 3 10.5z', 'g'), P('M3 15.5l13 6.5 13-6.5', 'n'), P('M3 20.5l13 6.5 4-2', 'n'), PLUS(24, 24, 'a', 6)]
@icon('undo-save')
def _(): return [P('M5 4h14l4 4v10H5z', 'b'), R(9, 4, 9, 6, 'p', 1), THICK('M10 25h11a4 4 0 0 0 0-8h-3M14 21l-4 4 4 4', 'a', 2)]
