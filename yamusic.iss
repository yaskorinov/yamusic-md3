; Inno Setup script для YaMusic (Rust + Qt 6)
; Собирается из build_windows.ps1:
;   ISCC /DAppVersion=0.1.0 /DDistDir=target\dist\yamusic /Otarget\dist yamusic.iss
; Результат: target\dist\YaMusic-<версия>-Setup.exe
; Установка пользовательская (без UAC) — в %LocalAppData%\Programs\YaMusic

#define AppName "YaMusic"
#ifndef AppVersion
  #define AppVersion "0.2.0"
#endif
; числовая версия для свойств файла (без -rc1 и т. п.)
#ifndef AppNumVersion
  #define AppNumVersion AppVersion
#endif
; папка, собранная build_windows.ps1: yamusic.exe, qml\, assets\, Qt, libmpv-2.dll
#ifndef DistDir
  #define DistDir "target\dist\yamusic"
#endif
#define AppPublisher "yaskorinov"
#define AppURL "https://github.com/yaskorinov/yamusic-md3"
#define AppExeName "yamusic.exe"

[Setup]
; AppId не менять: по нему новая версия ставится поверх старой (в т. ч. Python-сборки)
AppId={{A3F2C1B4-8D7E-4F9A-B2C5-1E6D3A4F7B8C}}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher={#AppPublisher}
AppPublisherURL={#AppURL}
AppSupportURL={#AppURL}/issues
AppUpdatesURL={#AppURL}/releases
VersionInfoVersion={#AppNumVersion}
; Пользовательская установка — не нужен UAC
DefaultDirName={localappdata}\Programs\{#AppName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
AllowNoIcons=yes
OutputDir=target\dist
OutputBaseFilename=YaMusic-{#AppVersion}-Setup
SetupIconFile=yamusic.ico
UninstallDisplayIcon={app}\{#AppExeName}
UninstallDisplayName={#AppName}
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
; Сборка только x64
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
; Без прав администратора
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
; Закрываем уже запущенный YaMusic перед обновлением
CloseApplications=yes
CloseApplicationsFilter=*{#AppExeName}
; Минимальная версия Windows — 10 1809
MinVersion=10.0.17763

[Languages]
Name: "russian"; MessagesFile: "compiler:Languages\Russian.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon";    Description: "Создать значок на &рабочем столе";    GroupDescription: "Дополнительные значки:"; Flags: checkedonce
Name: "startupicon";   Description: "Запускать YaMusic при старте Windows"; GroupDescription: "Автозапуск:";           Flags: unchecked

[InstallDelete]
; Обновление поверх старой версии (PyInstaller-сборки): её _internal\ больше не нужен
Type: filesandordirs; Name: "{app}\_internal"
; QML пересобирается целиком — убираем удалённые из новой версии файлы
Type: filesandordirs; Name: "{app}\qml"

[Files]
; Всё содержимое папки дистрибутива → папка установки
Source: "{#DistDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
; Меню «Пуск»
Name: "{userprograms}\{#AppName}\{#AppName}";                  Filename: "{app}\{#AppExeName}"; IconFilename: "{app}\{#AppExeName}"
Name: "{userprograms}\{#AppName}\Удалить {#AppName}";          Filename: "{uninstallexe}"

; Рабочий стол
Name: "{userdesktop}\{#AppName}";    Filename: "{app}\{#AppExeName}"; IconFilename: "{app}\{#AppExeName}"; Tasks: desktopicon

; Автозапуск
Name: "{userstartup}\{#AppName}";    Filename: "{app}\{#AppExeName}"; Tasks: startupicon

[Run]
; Предложить запустить YaMusic после установки
Filename: "{app}\{#AppExeName}"; Description: "Запустить {#AppName}"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
; Чистим кеш (скачанные обложки) при удалении — опционально
; Type: filesandordirs; Name: "{localappdata}\yamusic"
