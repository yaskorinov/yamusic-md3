; Inno Setup script для YaMusic
; Собирает: dist\YaMusic-Setup.exe
; Установка пользовательская (без UAC) — в %LocalAppData%\Programs\YaMusic

#define AppName "YaMusic"
#define AppVersion "0.1.0"
#define AppPublisher "yaskorinov"
#define AppURL "https://github.com/yaskorinov/yamusic-md3"
#define AppExeName "yamusic.exe"

[Setup]
AppId={{A3F2C1B4-8D7E-4F9A-B2C5-1E6D3A4F7B8C}}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher={#AppPublisher}
AppPublisherURL={#AppURL}
AppSupportURL={#AppURL}/issues
AppUpdatesURL={#AppURL}/releases
; Пользовательская установка — не нужен UAC
DefaultDirName={localappdata}\Programs\{#AppName}
DefaultGroupName={#AppName}
AllowNoIcons=yes
OutputDir=dist
OutputBaseFilename=YaMusic-Setup
SetupIconFile=yamusic.ico
UninstallDisplayIcon={app}\{#AppExeName}
UninstallDisplayName={#AppName}
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
; Без прав администратора
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
; Закрываем уже запущенный YaMusic перед обновлением
CloseApplications=yes
CloseApplicationsFilter=*{#AppExeName}
; Минимальная версия Windows — 10
MinVersion=10.0.17763

[Languages]
Name: "russian"; MessagesFile: "compiler:Languages\Russian.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon";    Description: "Создать значок на &рабочем столе";    GroupDescription: "Дополнительные значки:"; Flags: checkedonce
Name: "startupicon";   Description: "Запускать YaMusic при старте Windows"; GroupDescription: "Автозапуск:";           Flags: unchecked

[Files]
; Всё содержимое dist\yamusic\ → папка установки
Source: "dist\yamusic\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
; Меню «Пуск»
Name: "{userprograms}\{#AppName}\{#AppName}";                  Filename: "{app}\{#AppExeName}"; IconFilename: "{app}\{#AppExeName}"
Name: "{userprograms}\{#AppName}\Удалить {#AppName}";          Filename: "{uninstallexe}"

; Рабочий стол
Name: "{userdesktop}\{#AppName}";    Filename: "{app}\{#AppExeName}"; IconFilename: "{app}\{#AppExeName}"; Tasks: desktopicon

; Автозапуск
Name: "{userstartup}\{#AppName}";    Filename: "{app}\{#AppExeName}"; Parameters: "--minimized"; Tasks: startupicon

[Registry]
; Регистрируем в «Программы и компоненты» (Add/Remove Programs) — пользовательский раздел
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Uninstall\{#SetupSetting('AppId')}_is1"; ValueType: string; ValueName: "DisplayIcon"; ValueData: "{app}\{#AppExeName}"; Flags: uninsdeletekey

[Run]
; Предложить запустить YaMusic после установки
Filename: "{app}\{#AppExeName}"; Description: "Запустить {#AppName}"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
; Чистим кеш (скачанные обложки) при удалении — опционально
; Type: filesandordirs; Name: "{localappdata}\yamusic"
