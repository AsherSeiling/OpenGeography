# GIS Studio script worker. Runs in its own Python process, started by crates/engine/src/pyscript.rs with `python -I -X utf8 <this file>`.
# Protocol: binary frames on stdin/stdout, little-endian, standard library only. See docs/PYTHON_SPIKE.md and pyscript.rs for the layout.
#   in : frame = u8 kind, u32 length, payload.   kind 1 = init (JSON), 2 = chunk of rows, 3 = end
#   out: kind 1 = ready (JSON), 2 = chunk result, 3 = done
import sys, os, json, struct, datetime, math, numbers, traceback

# The protocol owns private copies of the original stdin and stdout. Then fd 1 is pointed at stderr and fd 0 at the null device, so nothing the
# script does can touch the frames: print(), input(), os.write(1, ...), a subprocess or a C library writing to fd 1 all go to stderr or nowhere.
_in = os.fdopen(os.dup(0), "rb")
_out = os.fdopen(os.dup(1), "wb")
_null = os.open(os.devnull, os.O_RDWR)
os.dup2(2, 1)
os.dup2(_null, 0)
os.close(_null)
sys.stdin = open(os.devnull, "r")
sys.stdout = sys.stderr

EPOCH_ORD = 719163  # date(1970, 1, 1).toordinal()
EPOCH = datetime.datetime(1970, 1, 1)
MICRO = datetime.timedelta(microseconds=1)
I64 = 1 << 63


def read_exact(n):
    buf = b""
    while len(buf) < n:
        part = _in.read(n - len(buf))
        if not part:
            raise EOFError
        buf += part
    return buf


def read_frame():
    head = read_exact(5)
    kind, n = struct.unpack("<BI", head)
    return kind, read_exact(n)


def write_frame(kind, payload):
    _out.write(struct.pack("<BI", kind, len(payload)))
    _out.write(payload)
    _out.flush()


class Row(dict):
    """The attributes of one feature. Geometry is not part of a row in this version."""

    def __missing__(self, key):
        if isinstance(key, str) and key.lower() in ("geometry", "geom", "shape", "wkt"):
            raise KeyError("A script sees a feature's attributes only; its geometry is not available in this version")
        raise KeyError("There is no field named %r. The fields are: %s" % (key, ", ".join(self.keys())))


def decode_columns(buf, fields):
    n = struct.unpack_from("<I", buf, 0)[0]
    pos = 4
    cols = []
    for f in fields:
        t = f["type"]
        valid = buf[pos:pos + n]
        pos += n
        if t == "int":
            vals = list(struct.unpack_from("<%dq" % n, buf, pos)); pos += 8 * n
        elif t == "float":
            vals = list(struct.unpack_from("<%dd" % n, buf, pos)); pos += 8 * n
        elif t == "bool":
            vals = [b != 0 for b in buf[pos:pos + n]]; pos += n
        elif t == "date":
            vals = [datetime.date.fromordinal(d + EPOCH_ORD) for d in struct.unpack_from("<%di" % n, buf, pos)]; pos += 4 * n
        elif t == "timestamp":
            vals = [EPOCH + datetime.timedelta(microseconds=u) for u in struct.unpack_from("<%dq" % n, buf, pos)]; pos += 8 * n
        else:  # text
            lens = struct.unpack_from("<%dI" % n, buf, pos); pos += 4 * n
            vals = []
            for L in lens:
                vals.append(buf[pos:pos + L].decode("utf-8")); pos += L
        # Nulls: a valid byte of 0 means None. (A null date or timestamp carries a placeholder the loop above decoded.)
        if not all(valid):
            vals = [v if ok else None for v, ok in zip(vals, valid)]
        cols.append(vals)
    return n, cols


def where(exc):
    """' (line N)' for the innermost frame inside the user's script."""
    line = None
    for fr in traceback.extract_tb(exc.__traceback__):
        if fr.filename == "<script>":
            line = fr.lineno
    return " (line %d)" % line if line else ""


def short(v):
    r = repr(v)
    return r if len(r) <= 60 else r[:57] + "..."


class BadReturn(ValueError):
    """The script ran, but what it returned is not something the field can hold."""


def classify(v, mode):
    """(tag, int_or_float, text) for a returned value, or raise BadReturn(message) for one a field cannot hold. Strict: no coercion."""
    if v is None:
        if mode == "filter":
            raise BadReturn("a filter must return True or False, not None")
        return 0, 0, None
    is_bool = isinstance(v, bool) or (type(v).__module__ == "numpy" and type(v).__name__ in ("bool_", "bool"))
    if is_bool:
        return 1, 1 if bool(v) else 0, None
    if mode == "filter":
        raise BadReturn("a filter must return True or False, not %s %s" % (type(v).__name__, short(v)))
    if isinstance(v, numbers.Integral):
        iv = int(v)
        if not -I64 <= iv < I64:
            raise BadReturn("the integer %s does not fit in 64 bits" % short(iv))
        return 2, iv, None
    if isinstance(v, numbers.Real):
        fv = float(v)
        if not math.isfinite(fv):
            raise BadReturn("returned %s; a field holds finite numbers, so return None for a missing value" % fv)
        return 3, fv, None
    if isinstance(v, str):
        try:
            return 4, 0, v.encode("utf-8")
        except UnicodeEncodeError:
            raise BadReturn("the text contains a character that cannot be stored (a lone surrogate)")
    if isinstance(v, datetime.datetime):
        if v.tzinfo is not None:
            raise BadReturn("a timezone-aware datetime is not supported; use a naive one")
        return 6, (v - EPOCH) // MICRO, None
    if isinstance(v, datetime.date):
        return 5, v.toordinal() - EPOCH_ORD, None
    raise BadReturn("returned a %s, which a field cannot hold" % type(v).__name__)


def main():
    kind, payload = read_frame()
    init = json.loads(payload.decode("utf-8"))
    fields, mode, script = init["fields"], init["mode"], init["script"]
    glb = {"__name__": "__gis_script__"}
    try:
        exec(compile(script, "<script>", "exec"), glb)
        fn = glb.get("calculate")
        if not callable(fn):
            raise NameError("the script must define a function  def calculate(row):")
    except BaseException as e:  # SyntaxError, NameError, anything at the top level of the script
        line = getattr(e, "lineno", None) or None
        msg = "%s: %s" % (type(e).__name__, e)
        if line is None:
            msg += where(e)
        write_frame(1, json.dumps({"ok": False, "error": msg}).encode("utf-8"))
        return
    write_frame(1, json.dumps({"ok": True, "python": sys.version.split()[0]}).encode("utf-8"))
    names = [f["name"] for f in fields]
    max_msgs = 20
    while True:
        kind, buf = read_frame()
        if kind == 3:
            write_frame(3, b"")
            return
        n, cols = decode_columns(buf, fields)
        tags = bytearray(n)
        slots = bytearray(8 * n)
        text_parts = []
        text_lens = [0] * n
        errors = []
        nerr = 0
        for i in range(n):
            row = Row(zip(names, [c[i] for c in cols]))
            try:
                tag, num, txt = classify(fn(row), mode)
            except BaseException as e:  # a failing script must not stop the others: the row is reported as failed
                if isinstance(e, (KeyboardInterrupt, SystemExit)):
                    raise
                nerr += 1
                tags[i] = 255
                if len(errors) < max_msgs:
                    if isinstance(e, BadReturn):
                        errors.append((i, str(e)))
                    elif isinstance(e, KeyError) and e.args and isinstance(e.args[0], str) and e.args[0].startswith(("A script sees", "There is no field")):
                        errors.append((i, e.args[0] + where(e)))
                    else:
                        errors.append((i, "%s: %s%s" % (type(e).__name__, e, where(e))))
                continue
            tags[i] = tag
            if tag == 3:
                struct.pack_into("<d", slots, 8 * i, num)
            elif tag in (1, 2, 5, 6):
                struct.pack_into("<q", slots, 8 * i, num)
            elif tag == 4:
                text_parts.append(txt)
                text_lens[i] = len(txt)
        out = [struct.pack("<II", n, nerr), bytes(tags), bytes(slots), struct.pack("<%dI" % n, *text_lens), b"".join(text_parts)]
        out.append(struct.pack("<I", len(errors)))
        for i, m in errors:
            mb = m.encode("utf-8", "replace")
            out.append(struct.pack("<II", i, len(mb)))
            out.append(mb)
        write_frame(2, b"".join(out))


try:
    main()
except EOFError:
    pass
