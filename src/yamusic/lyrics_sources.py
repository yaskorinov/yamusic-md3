"""Источники синхронного текста (из плагина Word Lyrics для DankMaterialShell, того же автора).

fetch() возвращает
  {"ok": true, "source": "NetEase", "kind": "word"|"line",
   "lines": [{"t": начало, "e": конец, "w": [{"x": "слово ", "s": начало, "e": конец, "p": [[символов, s, e], ...]}],
              "i": слова интерполированы} | {"gap": true, "t": начало, "e": конец}]}
или {"ok": false, "error": "...", "details": [...]}.

Цепочка: NetEase YRC (по словам) -> Musixmatch richsync (по словам) -> Яндекс LRC (по строкам)
-> LRCLIB -> Musixmatch subtitles -> NetEase LRC. У построчных текстов время слов
интерполируется по длине. Всё синхронное (urllib) — вызывается из рабочего потока.
"""

import hashlib
import json
import os
import re
import sys
import time
import unicodedata
import urllib.error
import urllib.parse
import urllib.request

UA = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0 Safari/537.36"
CACHE_DIR = ""          # задаёт yamusic.lyrics (каталог кэша приложения)
CACHE_VERSION = 3
NOT_FOUND_TTL = 6 * 3600
TIMEOUT = 8
GAP_MIN = 4.5          # seconds of silence that get the "• • •" interlude
DEBUG = bool(os.environ.get("YAMUSIC_LYRICS_DEBUG"))


def log(*a):
    if DEBUG:
        print("[lyrics]", *a, file=sys.stderr)


def http(url, data=None, headers=None, timeout=TIMEOUT, retries=2):
    h = {"User-Agent": UA}
    h.update(headers or {})
    for attempt in range(retries + 1):
        req = urllib.request.Request(url, data=data, headers=h)
        try:
            with urllib.request.urlopen(req, timeout=timeout) as r:
                return r.read().decode("utf-8", "replace")
        except urllib.error.HTTPError as e:
            if e.code < 500 or attempt == retries:
                raise
        except (urllib.error.URLError, TimeoutError):
            if attempt == retries:
                raise
        time.sleep(0.7 * (attempt + 1))


def http_json(url, **kw):
    return json.loads(http(url, **kw))


# ── text normalisation / matching ────────────────────────────────────────────

_PAREN = re.compile(r"\s*[\(\[\{（【].*?[\)\]\}）】]")
_FEAT = re.compile(r"\s+(feat\.?|ft\.?|featuring|при уч\.?)\s+.*$", re.I)
_DASH_SUFFIX = re.compile(r"\s+[-–—]\s+(.*(remaster|version|edit|live|mix|mono|stereo|ремастер|версия).*)$", re.I)


def clean_title(t):
    t = _DASH_SUFFIX.sub("", t)
    t = _PAREN.sub("", t)
    t = _FEAT.sub("", t)
    return t.strip() or t


def primary_artist(a):
    return re.split(r"\s*(?:,|&|/|;| feat\.? | ft\.? | x | и )\s*", a, flags=re.I)[0].strip() or a


def norm(s):
    s = unicodedata.normalize("NFKD", s.lower())
    s = "".join(c for c in s if not unicodedata.combining(c))
    s = s.replace("ё", "е")
    return re.sub(r"[\W_]+", "", s)


def similar(a, b):
    a, b = norm(a), norm(b)
    if not a or not b:
        return False
    return a == b or a in b or b in a


# ── shared lyric structure helpers ───────────────────────────────────────────

_META_LINE = re.compile(
    r"^\s*(作词|作曲|编曲|制作人|制作|混音|母带|和声|吉他|贝斯|鼓|录音|监制|出品|发行|词|曲|"
    r"lyrics|lyricist|composer|composed|producer|produced|arranger|written|mixed|mastered|vocals?)\s*[:：]",
    re.I,
)


def word_from_parts(parts):
    """parts: list of (text, start, end) syllables forming one visual word."""
    text = "".join(p[0] for p in parts)
    w = {"x": text, "s": round(parts[0][1], 3), "e": round(parts[-1][2], 3)}
    if len(parts) > 1:
        w["p"] = [[max(1, len(p[0].strip()) or 1), round(p[1], 3), round(p[2], 3)] for p in parts]
    return w


def group_syllables(sylls):
    """Merge syllables into words: a word ends where a syllable ends with whitespace,
    or at every CJK character (those can wrap anywhere)."""
    words, cur = [], []
    for text, s, e in sylls:
        if not text:
            continue
        cur.append((text, s, e))
        if text[-1].isspace() or re.search(r"[぀-ヿ㐀-鿿가-힯]$", text):
            words.append(word_from_parts(cur))
            cur = []
    if cur:
        words.append(word_from_parts(cur))
    return words


def interpolate_words(text, start, end):
    tokens = re.findall(r"\S+\s*", text)
    if not tokens:
        return []
    weights = [len(t.strip()) + 1.5 for t in tokens]
    total = sum(weights)
    out, t = [], start
    for tok, w in zip(tokens, weights):
        d = (end - start) * w / total
        out.append({"x": tok, "s": round(t, 3), "e": round(t + d, 3)})
        t += d
    return out


def lines_from_lrc(lrc):
    """Parse [mm:ss.xx] LRC into (time, text) pairs sorted by time."""
    out = []
    tag = re.compile(r"\[(\d+):(\d+(?:[.:]\d+)?)\]")
    for raw in lrc.splitlines():
        stamps = tag.findall(raw)
        if not stamps:
            continue
        text = tag.sub("", raw).strip()
        # strip enhanced-LRC word tags if present, we only use line timing here
        text = re.sub(r"<\d+:\d+(?:[.:]\d+)?>", "", text).strip()
        for m, s in stamps:
            out.append((int(m) * 60 + float(s.replace(":", ".")), text))
    out.sort(key=lambda x: x[0])
    return out


def build_from_timed_lines(pairs, duration):
    """pairs: [(start, text)] line-level -> interpolated word lines."""
    pairs = [(t, x) for t, x in pairs if not _META_LINE.match(x)]
    lines = []
    for i, (t, text) in enumerate(pairs):
        if not text:
            continue
        nxt = next((p[0] for p in pairs[i + 1:]), duration if duration > t else t + 6)
        n_chars = len(text)
        sung = max(1.2, min(n_chars * 0.085 + 0.4, 9.0))
        end = min(nxt - 0.05, t + sung)
        if end <= t:
            end = t + 0.5
        lines.append({"t": round(t, 3), "e": round(end, 3), "w": interpolate_words(text, t, end), "i": True})
    return lines


def add_gaps(lines, duration):
    out, prev_end = [], 0.0
    for ln in lines:
        if ln["t"] - prev_end >= GAP_MIN:
            out.append({"gap": True, "t": round(prev_end + (0.3 if prev_end else 0.0), 3), "e": round(ln["t"] - 0.15, 3)})
        out.append(ln)
        prev_end = max(prev_end, ln["e"])
    return out


# ── NetEase ──────────────────────────────────────────────────────────────────

_NE_KEY = b"e82ckenh8dichen8"


def _ne_eapi_params(path, params):
    from cryptography.hazmat.primitives import padding
    from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes

    text = json.dumps(params, separators=(",", ":"))
    digest = hashlib.md5(f"nobody{path}use{text}md5forencrypt".encode()).hexdigest()
    data = f"{path}-36cd479b6b5-{text}-36cd479b6b5-{digest}".encode()
    p = padding.PKCS7(128).padder()
    data = p.update(data) + p.finalize()
    enc = Cipher(algorithms.AES(_NE_KEY), modes.ECB()).encryptor()
    return (enc.update(data) + enc.finalize()).hex().upper()


_VERSION_WORDS = ("remix", "live", "acoustic", "instrumental", "karaoke", "cover", "sped up", "slowed",
                  "nightcore", "demo", "edit", "version", "mix", "ремикс", "лайв", "акустика", "минус", "伴奏")


def version_tags(s):
    s = s.lower()
    return {w for w in _VERSION_WORDS if w in s}


def ne_search(title, artist, duration):
    q = f"{clean_title(title)} {primary_artist(artist)}"
    url = "https://music.163.com/api/cloudsearch/pc?" + urllib.parse.urlencode({"s": q, "type": 1, "limit": 15, "offset": 0})
    d = http_json(url, headers={"Referer": "https://music.163.com"})
    songs = (d.get("result") or {}).get("songs") or []
    best, best_score = None, -1
    for s in songs:
        name = s.get("name", "")
        artists = [a.get("name", "") for a in s.get("ar", [])]
        dt = (s.get("dt") or 0) / 1000
        if not similar(clean_title(name), clean_title(title)):
            continue
        a_ok = any(similar(a, primary_artist(artist)) for a in artists) or similar(" ".join(artists), artist)
        diff = abs(dt - duration) if duration > 0 else 0
        if duration > 0 and diff > 6:
            continue
        extra = version_tags(name) - version_tags(title)
        if extra:
            continue  # "Song (Remix)" when "Song" is playing: different timing
        score = (10 if a_ok else 0) + (5 if norm(name) == norm(title) else 0) + max(0, 5 - diff)
        if not a_ok and duration <= 0:
            continue
        if score > best_score:
            best, best_score = s, score
    return best


def ne_lyric(song_id):
    path = "/api/song/lyric/v1"
    params = {"id": str(song_id), "cp": "false", "lv": "0", "kv": "0", "tv": "0", "rv": "0",
              "yv": "0", "ytv": "0", "yrv": "0", "csrf_token": ""}
    body = urllib.parse.urlencode({"params": _ne_eapi_params(path, params)}).encode()
    return http_json("https://interface3.music.163.com/eapi/song/lyric/v1", data=body,
                     headers={"Referer": "https://music.163.com", "Cookie": "os=pc; appver=2.10.13"})


_YRC_LINE = re.compile(r"^\[(\d+),(\d+)\](.*)$")
_YRC_WORD = re.compile(r"\((\d+),(\d+),\d+\)([^(]*)")


def parse_yrc(yrc):
    lines = []
    for raw in yrc.splitlines():
        m = _YRC_LINE.match(raw.strip())
        if not m:
            continue  # JSON credit lines
        ls, ld, body = int(m.group(1)) / 1000, int(m.group(2)) / 1000, m.group(3)
        sylls = [(t, int(s) / 1000, (int(s) + int(d)) / 1000) for s, d, t in _YRC_WORD.findall(body)]
        text = "".join(s[0] for s in sylls).strip()
        if not text or _META_LINE.match(text):
            continue
        words = group_syllables(sylls)
        if not words:
            continue
        lines.append({"t": round(ls, 3), "e": round(max(ls + ld, words[-1]["e"]), 3), "w": words, "i": False})
    return lines


# ── Musixmatch ───────────────────────────────────────────────────────────────

MXM_BASE = "https://apic-desktop.musixmatch.com/ws/1.1/"
MXM_HEADERS = {"authority": "apic-desktop.musixmatch.com", "cookie": "x-mxm-token-guid="}


def mxm_token(user_token):
    if user_token:
        return user_token
    path = os.path.join(CACHE_DIR, "mxm_token.json")
    try:
        with open(path) as f:
            c = json.load(f)
        if c.get("expires", 0) > time.time():
            return c.get("token")  # may be None == blocked, retry later
    except (OSError, ValueError):
        pass
    tok = None
    try:
        d = http_json(MXM_BASE + "token.get?app_id=web-desktop-app-v1.0&format=json", headers=MXM_HEADERS, timeout=5)
        tok = d["message"]["body"].get("user_token")
        if not tok or set(tok) == {"0"}:
            tok = None
    except Exception as e:  # noqa: BLE001
        log("mxm token error", e)
    with open(path, "w") as f:
        json.dump({"token": tok, "expires": time.time() + (6 * 3600 if tok else 3600)}, f)
    return tok


def mxm_call(method, token, **params):
    params.update({"app_id": "web-desktop-app-v1.0", "format": "json", "usertoken": token})
    d = http_json(MXM_BASE + method + "?" + urllib.parse.urlencode(params), headers=MXM_HEADERS)
    return d.get("message", {})


def mxm_fetch(title, artist, duration, token):
    """Returns ('word', lines) / ('line', lines) / None."""
    msg = mxm_call("macro.subtitles.get", token, q_track=clean_title(title), q_artist=artist,
                   q_duration=int(duration) if duration else "", namespace="lyrics_richsynched",
                   subtitle_format="mxm", optional_calls="track.richsync")
    calls = (msg.get("body") or {}).get("macro_calls") or {}
    track = (((calls.get("matcher.track.get") or {}).get("message") or {}).get("body") or {}).get("track") or {}
    if not track:
        return None
    if track.get("has_richsync"):
        rs = mxm_call("track.richsync.get", token, commontrack_id=track.get("commontrack_id"),
                      f_richsync_length=track.get("track_length"))
        body = ((rs.get("body") or {}).get("richsync") or {}).get("richsync_body")
        if body:
            lines = []
            for ln in json.loads(body):
                ts, te = float(ln["ts"]), float(ln["te"])
                chunks = ln.get("l") or []
                sylls = []
                for i, ch in enumerate(chunks):
                    s = ts + float(ch["o"])
                    e = ts + float(chunks[i + 1]["o"]) if i + 1 < len(chunks) else te
                    if ch["c"].strip() == "" and sylls:
                        t0, s0, _ = sylls[-1]
                        sylls[-1] = (t0 + ch["c"], s0, sylls[-1][2])
                    elif ch["c"]:
                        sylls.append((ch["c"], s, e))
                words = group_syllables(sylls)
                if words:
                    lines.append({"t": round(ts, 3), "e": round(te, 3), "w": words, "i": False})
            if lines:
                return "word", lines
    sub = (((calls.get("track.subtitles.get") or {}).get("message") or {}).get("body") or {}).get("subtitle_list") or []
    if sub:
        items = json.loads(sub[0]["subtitle"]["subtitle_body"])
        pairs = [(float(it["time"]["total"]), it.get("text", "")) for it in items]
        return "line", build_from_timed_lines(pairs, duration)
    return None


# ── LRCLIB ───────────────────────────────────────────────────────────────────

def lrclib_fetch(title, artist, album, duration):
    params = {"track_name": title, "artist_name": artist}
    if album:
        params["album_name"] = album
    if duration:
        params["duration"] = int(round(duration))
    try:
        d = http_json("https://lrclib.net/api/get?" + urllib.parse.urlencode(params),
                      headers={"Lrclib-Client": "yamusic"})
        if d.get("syncedLyrics"):
            return d["syncedLyrics"]
    except urllib.error.HTTPError as e:
        if e.code != 404:
            raise
    q = urllib.parse.urlencode({"track_name": clean_title(title), "artist_name": primary_artist(artist)})
    for r in http_json("https://lrclib.net/api/search?" + q, headers={"Lrclib-Client": "yamusic"}):
        if not r.get("syncedLyrics"):
            continue
        if duration and abs((r.get("duration") or 0) - duration) > 5:
            continue
        return r["syncedLyrics"]
    return None


# ── main ─────────────────────────────────────────────────────────────────────

def fetch(title, artist, album, duration, mxm_user_token="", yandex_lrc=None):
    """yandex_lrc — функция без аргументов, возвращающая LRC от Яндекса или None."""
    errors = []
    ne_song = None

    def attempt(name, fn):
        try:
            return fn()
        except Exception as e:  # noqa: BLE001
            log(name, "failed:", repr(e))
            errors.append(f"{name}: {e}")
            return None

    # 1. NetEase YRC (word level)
    ne_song = attempt("netease-search", lambda: ne_search(title, artist, duration))
    ne_data = None
    if ne_song:
        log("netease match", ne_song.get("id"), ne_song.get("name"))
        ne_data = attempt("netease-lyric", lambda: ne_lyric(ne_song["id"]))
        yrc = ((ne_data or {}).get("yrc") or {}).get("lyric")
        if yrc:
            lines = parse_yrc(yrc)
            if lines:
                return {"ok": True, "source": "NetEase", "kind": "word", "lines": lines}

    # 2. Musixmatch richsync (word level) — also gives us line fallback
    mxm_line = None
    tok = attempt("musixmatch-token", lambda: mxm_token(mxm_user_token))
    if tok:
        r = attempt("musixmatch", lambda: mxm_fetch(title, artist, duration, tok))
        if r and r[0] == "word":
            return {"ok": True, "source": "Musixmatch", "kind": "word", "lines": r[1]}
        if r:
            mxm_line = r[1]

    # 3. Яндекс: синхронный текст по строкам (хорошо покрывает русскую музыку)
    if yandex_lrc is not None:
        lrc = attempt("yandex", yandex_lrc)
        if lrc:
            lines = build_from_timed_lines(lines_from_lrc(lrc), duration)
            if lines:
                return {"ok": True, "source": "Яндекс", "kind": "line", "lines": lines}

    # 4. LRCLIB (line level, interpolated)
    lrc = attempt("lrclib", lambda: lrclib_fetch(title, artist, album, duration))
    if lrc:
        lines = build_from_timed_lines(lines_from_lrc(lrc), duration)
        if lines:
            return {"ok": True, "source": "LRCLIB", "kind": "line", "lines": lines}

    # 5. Musixmatch subtitles
    if mxm_line:
        return {"ok": True, "source": "Musixmatch", "kind": "line", "lines": mxm_line}

    # 6. NetEase plain LRC
    ne_lrc = ((ne_data or {}).get("lrc") or {}).get("lyric")
    if ne_lrc:
        lines = build_from_timed_lines(lines_from_lrc(ne_lrc), duration)
        if lines:
            return {"ok": True, "source": "NetEase", "kind": "line", "lines": lines}

    return {"ok": False, "error": "not found", "details": errors}
