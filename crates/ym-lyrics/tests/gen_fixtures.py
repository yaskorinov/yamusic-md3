"""Эталоны для tests/python_fixtures.rs — выходы src/yamusic/lyrics_sources.py.

Запуск из корня репозитория:
    .venv/Scripts/python.exe -I crates/ym-lyrics/tests/gen_fixtures.py
"""

import hashlib
import importlib.util
import json
import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[3]
spec = importlib.util.spec_from_file_location("lyrics_sources", ROOT / "src" / "yamusic" / "lyrics_sources.py")
ls = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ls)

LRC = [
    "[00:12.50]вторая строка\n[00:05.00][01:10.00]первая и повтор\n[ar:кто-то]\nбез метки",
    "[00:01.00]<00:01.00>раз <00:01.50>два\r\n[00:02:50] три \r[00:03]четыре\n[00:03.333]  ",
    "[ti:Title]\n[00:00.00]作词 : 某人\n[00:00.50]作曲 : 某人\n[00:17.12]Hello darkness, my old friend\n"
    "[00:21.86]I've come to talk with you again\n[00:27.01]\n[00:33.40]Because a vision softly creeping\n"
    "[01:02.789]Left its seeds while I was sleeping\n[10:00.00]конец",
    "",
    "no tags at all\n[xx:yy]bad",
]

PAIRS = [
    ([(10.0, "раз два три"), (14.0, ""), (20.0, "конец")], 30.0),
    ([(0.0, "作词 : 某人"), (1.0, "Lyrics: someone"), (5.0, "настоящая строка")], 10.0),
    ([(1.234, "a"), (1.3, "короткая"), (60.0, "очень длинная строка " * 8)], 0.0),
    ([(5.0, "последняя")], 3.0),
    ([(5.0, "тут  два   пробела\tи таб"), (5.0, "одновременно")], 100.0),
    ([(0.1, "Белый снег, серый лёд,"), (3.7, "На растрескавшейся земле."), (8.03, "Одеялом лоскутным на ней —"),
      (12.41, "Город в дорожной петле.")], 286.0),
]
for lrc in LRC:
    PAIRS.append((ls.lines_from_lrc(lrc), 240.0))

YRC = [
    '{"t":0,"c":[{"tx":"作词: "},{"tx":"某人"}]}\n[1000,2000](1000,500,0)Hel(1500,500,0)lo (2000,1000,0)world\n'
    "[4000,100](4000,100,0)你(4100,100,0)好",
    "[17120,4740](17120,400,0)Hello (17520,610,0)dark(18130,390,0)ness, (18520,330,0)my (18850,520,0)old "
    "(19370,1490,0)friend\n[21860,5150](21860,300,0)I've (22160,410,0)come (22570,280,0)to (22850,330,0)talk",
    "[0,1000](0,500,0)作词(500,500,0): 某人\n[5000,3000](5000,1000,0)  \n[9000,500](9000,1200,0)longer ",
    "[1,2](1,1,0)a(2,1,0)b",
]

LINES = []
for y in YRC:
    LINES.append(ls.parse_yrc(y))
for pairs, duration in PAIRS:
    LINES.append(ls.build_from_timed_lines(pairs, duration))

PLACEHOLDER = [
    ["纯音乐，请欣赏"], ["暂无歌词"], [], ["", "  "],
    ["Белый снег, серый лёд", "На растрескавшейся земле"], ["Hello darkness, my old friend"],
    ["你好世界"], ["你好", "世界"], ["你好", "世界", "再见"], ["123 !!"], ["Ok"], ["½"], ["_"],
    ["纯音乐", "a", "b", "c", "d"], ["纯音乐", "暂无歌词", "a", "b", "c", "d"], ["纯音乐", "a", "b", "c", "d", "e"],
]

TITLES = ["Song (Remastered 2011)", "Song - 2011 Remaster", "Song feat. Someone", "Песня [Live] при уч. Кто-то",
          "(Intro)", "  ", "Creep", "Группа крови", "Song – Radio Edit", "Tôi (feat. X) [Mono]", "A - B",
          "Hello 【中文】 World （版）"]
ARTISTS = ["Кино, Виктор Цой", "A feat. B", "Макс и Даша", "AC/DC", ", B", "Simon & Garfunkel", "X x Y", "Radiohead"]
NORM = ["Ёлка — Café!", "Tôi Đi", "ＦＵＬＬ　ＷＩＤＴＨ", "a_b-c", "ﬁ", "Σίσυφος", "naïve", "  "]
SIMILAR = [("Группа крови", "группа крови (remastered)"), ("", "x"), ("abc", "xyz"), ("Creep", "creep!"), ("a", "b")]
ROUND = [0.0625, 2.675, 1.0005, 12.3456, 0.0005, 0.0015, 123.4565, 7.1 + 0.2, 1 / 3, 286.0 * 0.085 + 0.4]

ne_params_sorted = {"a": "1", "b": "ж", "id": "42"}
ne_lyric_params = {"id": "1974443814", "cp": "false", "lv": "0", "kv": "0", "tv": "0", "rv": "0",
                   "yv": "0", "ytv": "0", "yrv": "0", "csrf_token": ""}

out = {
    "lines_from_lrc": [{"lrc": lrc, "out": ls.lines_from_lrc(lrc)} for lrc in LRC],
    "build_from_timed_lines": [{"pairs": p, "duration": d, "out": ls.build_from_timed_lines(p, d)} for p, d in PAIRS],
    "parse_yrc": [{"yrc": y, "out": ls.parse_yrc(y)} for y in YRC],
    "add_gaps": [{"lines": ln, "out": ls.add_gaps(ln, 0)} for ln in LINES],
    "is_placeholder": [{"texts": t, "out": ls.is_placeholder(t)} for t in PLACEHOLDER],
    "clean_title": [[t, ls.clean_title(t)] for t in TITLES],
    "primary_artist": [[a, ls.primary_artist(a)] for a in ARTISTS],
    "norm": [[s, ls.norm(s)] for s in NORM],
    "similar": [[a, b, ls.similar(a, b)] for a, b in SIMILAR],
    "round3": [[x, round(x, 3)] for x in ROUND],
    "ne_eapi_params": [
        {"path": "/api/song/lyric/v1", "params": ne_params_sorted,
         "out": ls._ne_eapi_params("/api/song/lyric/v1", ne_params_sorted)},
    ],
    "ne_lyric_params": {"id": "1974443814", "json": json.dumps(ne_lyric_params, separators=(",", ":")),
                        "out": ls._ne_eapi_params("/api/song/lyric/v1", ne_lyric_params)},
    "cache_key": [[tid, hashlib.sha1(f"{ls.CACHE_VERSION}|{tid}".encode()).hexdigest()] for tid in ["12345", "1", "abc:1"]],
}

path = pathlib.Path(__file__).with_name("fixtures.json")
path.write_text(json.dumps(out, ensure_ascii=False, indent=1), encoding="utf-8")
print("written", path)
