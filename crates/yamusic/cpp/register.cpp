// Регистрация типов QML-модуля YaMusic.Core (как qmlRegisterSingletonInstance в app.py).
// Владелец — агент «shell». Синглтоны создаются здесь, ДО движка, и живут до конца процесса.
// Имена типов и заголовки мостов — фиксированы контрактом (docs/RUST_CONTRACT.md §3).
#include "register.h"

#include <QtQml/QQmlEngine>
#include <QtQml/qqml.h>

#include "yamusic/src/bridges/auth.cxxqt.h"
#include "yamusic/src/bridges/catalog.cxxqt.h"
#include "yamusic/src/bridges/data.cxxqt.h"
#include "yamusic/src/bridges/library.cxxqt.h"
#include "yamusic/src/bridges/lyrics.cxxqt.h"
#include "yamusic/src/bridges/player.cxxqt.h"
#include "yamusic/src/bridges/settings.cxxqt.h"
#include "yamusic/src/bridges/theme.cxxqt.h"
#include "yamusic/src/bridges/wave.cxxqt.h"
#include "yamusic/src/icons.cxxqt.h"
#include "yamusic/src/models/list_model.cxxqt.h"
#include "yamusic/src/models/track_filter.cxxqt.h"

namespace ym {

namespace {
constexpr const char* URI = "YaMusic.Core";

template<typename T>
void
singleton(const char* name)
{
  auto* object = new T();
  QQmlEngine::setObjectOwnership(object, QQmlEngine::CppOwnership);
  qmlRegisterSingletonInstance<T>(URI, 1, 0, name, object);
}
} // namespace

void
register_qml_types()
{
  // Порядок важен: Settings и ThemeEngine первыми (их читают остальные при создании)
  singleton<Settings>("Settings");
  singleton<ThemeEngine>("ThemeEngine");
  singleton<IconMetrics>("IconMetrics");
  singleton<Auth>("Auth");
  singleton<Library>("Library");
  singleton<Player>("Player");
  singleton<Wave>("Wave");
  singleton<Catalog>("Catalog");
  singleton<Lyrics>("Lyrics");

  qmlRegisterType<TrackFilter>(URI, 1, 0, "TrackFilter");
  qmlRegisterUncreatableType<DictListModel>(URI, 1, 0, "DictListModel", QStringLiteral("модель создаётся в Rust"));
  qmlRegisterUncreatableType<AlbumData>(URI, 1, 0, "AlbumData", QStringLiteral("Catalog.album(id)"));
  qmlRegisterUncreatableType<ArtistData>(URI, 1, 0, "ArtistData", QStringLiteral("Catalog.artist(id)"));
}

} // namespace ym
