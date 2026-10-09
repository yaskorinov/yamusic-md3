# YaMusic

Неофициальный клиент Яндекс Музыки для Linux в стиле **Material 3 Expressive**: QML + PySide6, звук через libmpv,
тема приложения перекрашивается под обложку играющего трека.

> Проект в разработке. Сейчас готовы MD3-кит и каркас приложения; вход, API и плеер — следующие этапы
> (см. [docs/SPEC.md](docs/SPEC.md)).

## Запуск

```bash
sudo pacman -S mpv uv
uv run yamusic            # приложение
uv run yamusic --gallery  # галерея компонентов MD3
```

## Шрифты

- [Google Sans](https://fonts.google.com/specimen/Google+Sans) — SIL Open Font License 1.1
- [Material Symbols Rounded](https://github.com/google/material-design-icons) — Apache License 2.0

Клиент использует неофициальный API Яндекс Музыки и не связан с Яндексом.
