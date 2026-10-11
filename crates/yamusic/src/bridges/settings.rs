// Settings (settings.py): 24 персистентных свойства с сигналами <имя>Changed. Значения кэшируются в мосте,
// запись — сразу в SettingsStore; изменения из других мест (Player.volume, Wave.select) приходят подпиской.
use std::pin::Pin;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use ym_platform::{SettingKey, SettingValue, SettingsStore};

use super::{Delivery, forward_events};
use crate::context::ctx;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qproperty(QString, theme_mode, cxx_name = "themeMode", READ, WRITE = set_theme_mode, NOTIFY = theme_mode_changed)]
        #[qproperty(QString, scheme_variant, cxx_name = "schemeVariant", READ, WRITE = set_scheme_variant, NOTIFY = scheme_variant_changed)]
        #[qproperty(bool, accent_from_cover, cxx_name = "accentFromCover", READ, WRITE = set_accent_from_cover, NOTIFY = accent_from_cover_changed)]
        #[qproperty(QString, custom_seed, cxx_name = "customSeed", READ, WRITE = set_custom_seed, NOTIFY = custom_seed_changed)]
        #[qproperty(bool, window_buttons, cxx_name = "windowButtons", READ = window_buttons_value, WRITE = set_window_buttons, NOTIFY = window_buttons_changed)]
        #[qproperty(QString, track_transition, cxx_name = "trackTransition", READ, WRITE = set_track_transition, NOTIFY = track_transition_changed)]
        #[qproperty(QString, backdrop_mode, cxx_name = "backdropMode", READ, WRITE = set_backdrop_mode, NOTIFY = backdrop_mode_changed)]
        #[qproperty(f64, now_playing_blur, cxx_name = "nowPlayingBlur", READ, WRITE = set_now_playing_blur, NOTIFY = now_playing_blur_changed)]
        #[qproperty(f64, now_playing_drift, cxx_name = "nowPlayingDrift", READ, WRITE = set_now_playing_drift, NOTIFY = now_playing_drift_changed)]
        #[qproperty(bool, ambient_background, cxx_name = "ambientBackground", READ, WRITE = set_ambient_background, NOTIFY = ambient_background_changed)]
        #[qproperty(bool, cover_spin, cxx_name = "coverSpin", READ, WRITE = set_cover_spin, NOTIFY = cover_spin_changed)]
        #[qproperty(bool, cover_pulse, cxx_name = "coverPulse", READ, WRITE = set_cover_pulse, NOTIFY = cover_pulse_changed)]
        #[qproperty(bool, calm_when_inactive, cxx_name = "calmWhenInactive", READ, WRITE = set_calm_when_inactive, NOTIFY = calm_when_inactive_changed)]
        #[qproperty(bool, right_panel_open, cxx_name = "rightPanelOpen", READ, WRITE = set_right_panel_open, NOTIFY = right_panel_open_changed)]
        #[qproperty(QString, right_panel_tab, cxx_name = "rightPanelTab", READ, WRITE = set_right_panel_tab, NOTIFY = right_panel_tab_changed)]
        #[qproperty(bool, sidebar_collapsed, cxx_name = "sidebarCollapsed", READ, WRITE = set_sidebar_collapsed, NOTIFY = sidebar_collapsed_changed)]
        #[qproperty(i32, window_width, cxx_name = "windowWidth", READ, WRITE = set_window_width, NOTIFY = window_width_changed)]
        #[qproperty(i32, window_height, cxx_name = "windowHeight", READ, WRITE = set_window_height, NOTIFY = window_height_changed)]
        #[qproperty(i32, window_x, cxx_name = "windowX", READ, WRITE = set_window_x, NOTIFY = window_x_changed)]
        #[qproperty(i32, window_y, cxx_name = "windowY", READ, WRITE = set_window_y, NOTIFY = window_y_changed)]
        #[qproperty(bool, window_maximized, cxx_name = "windowMaximized", READ, WRITE = set_window_maximized, NOTIFY = window_maximized_changed)]
        #[qproperty(QString, quality, READ, WRITE = set_quality, NOTIFY = quality_changed)]
        #[qproperty(f64, volume, READ, WRITE = set_volume, NOTIFY = volume_changed)]
        #[qproperty(i32, crossfade, READ, WRITE = set_crossfade, NOTIFY = crossfade_changed)]
        #[qproperty(QString, wave_seeds, cxx_name = "waveSeeds", READ, WRITE = set_wave_seeds, NOTIFY = wave_seeds_changed)]
        #[qproperty(bool, close_to_tray, cxx_name = "closeToTray", READ, WRITE = set_close_to_tray, NOTIFY = close_to_tray_changed)]
        #[qproperty(i32, wheel_step, cxx_name = "wheelStep", READ, WRITE = set_wheel_step, NOTIFY = wheel_step_changed)]
        type Settings = super::SettingsRust;

        fn window_buttons_value(self: &Settings) -> bool;

        fn set_theme_mode(self: Pin<&mut Settings>, value: QString);
        fn set_scheme_variant(self: Pin<&mut Settings>, value: QString);
        fn set_accent_from_cover(self: Pin<&mut Settings>, value: bool);
        fn set_custom_seed(self: Pin<&mut Settings>, value: QString);
        fn set_window_buttons(self: Pin<&mut Settings>, value: bool);
        fn set_track_transition(self: Pin<&mut Settings>, value: QString);
        fn set_backdrop_mode(self: Pin<&mut Settings>, value: QString);
        fn set_now_playing_blur(self: Pin<&mut Settings>, value: f64);
        fn set_now_playing_drift(self: Pin<&mut Settings>, value: f64);
        fn set_ambient_background(self: Pin<&mut Settings>, value: bool);
        fn set_cover_spin(self: Pin<&mut Settings>, value: bool);
        fn set_cover_pulse(self: Pin<&mut Settings>, value: bool);
        fn set_calm_when_inactive(self: Pin<&mut Settings>, value: bool);
        fn set_right_panel_open(self: Pin<&mut Settings>, value: bool);
        fn set_right_panel_tab(self: Pin<&mut Settings>, value: QString);
        fn set_sidebar_collapsed(self: Pin<&mut Settings>, value: bool);
        fn set_window_width(self: Pin<&mut Settings>, value: i32);
        fn set_window_height(self: Pin<&mut Settings>, value: i32);
        fn set_window_x(self: Pin<&mut Settings>, value: i32);
        fn set_window_y(self: Pin<&mut Settings>, value: i32);
        fn set_window_maximized(self: Pin<&mut Settings>, value: bool);
        fn set_quality(self: Pin<&mut Settings>, value: QString);
        fn set_volume(self: Pin<&mut Settings>, value: f64);
        fn set_crossfade(self: Pin<&mut Settings>, value: i32);
        fn set_wave_seeds(self: Pin<&mut Settings>, value: QString);
        fn set_close_to_tray(self: Pin<&mut Settings>, value: bool);
        fn set_wheel_step(self: Pin<&mut Settings>, value: i32);

        #[qsignal]
        #[cxx_name = "themeModeChanged"]
        fn theme_mode_changed(self: Pin<&mut Settings>);
        #[qsignal]
        #[cxx_name = "schemeVariantChanged"]
        fn scheme_variant_changed(self: Pin<&mut Settings>);
        #[qsignal]
        #[cxx_name = "accentFromCoverChanged"]
        fn accent_from_cover_changed(self: Pin<&mut Settings>);
        #[qsignal]
        #[cxx_name = "customSeedChanged"]
        fn custom_seed_changed(self: Pin<&mut Settings>);
        #[qsignal]
        #[cxx_name = "windowButtonsChanged"]
        fn window_buttons_changed(self: Pin<&mut Settings>);
        #[qsignal]
        #[cxx_name = "trackTransitionChanged"]
        fn track_transition_changed(self: Pin<&mut Settings>);
        #[qsignal]
        #[cxx_name = "backdropModeChanged"]
        fn backdrop_mode_changed(self: Pin<&mut Settings>);
        #[qsignal]
        #[cxx_name = "nowPlayingBlurChanged"]
        fn now_playing_blur_changed(self: Pin<&mut Settings>);
        #[qsignal]
        #[cxx_name = "nowPlayingDriftChanged"]
        fn now_playing_drift_changed(self: Pin<&mut Settings>);
        #[qsignal]
        #[cxx_name = "ambientBackgroundChanged"]
        fn ambient_background_changed(self: Pin<&mut Settings>);
        #[qsignal]
        #[cxx_name = "coverSpinChanged"]
        fn cover_spin_changed(self: Pin<&mut Settings>);
        #[qsignal]
        #[cxx_name = "coverPulseChanged"]
        fn cover_pulse_changed(self: Pin<&mut Settings>);
        #[qsignal]
        #[cxx_name = "calmWhenInactiveChanged"]
        fn calm_when_inactive_changed(self: Pin<&mut Settings>);
        #[qsignal]
        #[cxx_name = "rightPanelOpenChanged"]
        fn right_panel_open_changed(self: Pin<&mut Settings>);
        #[qsignal]
        #[cxx_name = "rightPanelTabChanged"]
        fn right_panel_tab_changed(self: Pin<&mut Settings>);
        #[qsignal]
        #[cxx_name = "sidebarCollapsedChanged"]
        fn sidebar_collapsed_changed(self: Pin<&mut Settings>);
        #[qsignal]
        #[cxx_name = "windowWidthChanged"]
        fn window_width_changed(self: Pin<&mut Settings>);
        #[qsignal]
        #[cxx_name = "windowHeightChanged"]
        fn window_height_changed(self: Pin<&mut Settings>);
        #[qsignal]
        #[cxx_name = "windowXChanged"]
        fn window_x_changed(self: Pin<&mut Settings>);
        #[qsignal]
        #[cxx_name = "windowYChanged"]
        fn window_y_changed(self: Pin<&mut Settings>);
        #[qsignal]
        #[cxx_name = "windowMaximizedChanged"]
        fn window_maximized_changed(self: Pin<&mut Settings>);
        #[qsignal]
        #[cxx_name = "qualityChanged"]
        fn quality_changed(self: Pin<&mut Settings>);
        #[qsignal]
        #[cxx_name = "volumeChanged"]
        fn volume_changed(self: Pin<&mut Settings>);
        #[qsignal]
        #[cxx_name = "crossfadeChanged"]
        fn crossfade_changed(self: Pin<&mut Settings>);
        #[qsignal]
        #[cxx_name = "waveSeedsChanged"]
        fn wave_seeds_changed(self: Pin<&mut Settings>);
        #[qsignal]
        #[cxx_name = "closeToTrayChanged"]
        fn close_to_tray_changed(self: Pin<&mut Settings>);
        #[qsignal]
        #[cxx_name = "wheelStepChanged"]
        fn wheel_step_changed(self: Pin<&mut Settings>);
    }

    impl cxx_qt::Threading for Settings {}
    impl cxx_qt::Initialize for Settings {}
}

/// Тип свойства QML ↔ значение хранилища.
trait Field: Sized + PartialEq + Clone {
    fn load(store: &SettingsStore, key: SettingKey) -> Self;
    fn to_value(&self) -> SettingValue;
}

impl Field for QString {
    fn load(store: &SettingsStore, key: SettingKey) -> Self {
        QString::from(store.get_str(key).as_str())
    }
    fn to_value(&self) -> SettingValue {
        SettingValue::Str(self.to_string())
    }
}

impl Field for bool {
    fn load(store: &SettingsStore, key: SettingKey) -> Self {
        store.get_bool(key)
    }
    fn to_value(&self) -> SettingValue {
        SettingValue::Bool(*self)
    }
}

impl Field for i32 {
    fn load(store: &SettingsStore, key: SettingKey) -> Self {
        store.get_int(key).clamp(i32::MIN as i64, i32::MAX as i64) as i32
    }
    fn to_value(&self) -> SettingValue {
        SettingValue::Int(*self as i64)
    }
}

impl Field for f64 {
    fn load(store: &SettingsStore, key: SettingKey) -> Self {
        store.get_f64(key)
    }
    fn to_value(&self) -> SettingValue {
        SettingValue::Float(*self)
    }
}

fn store() -> &'static SettingsStore {
    &ctx().services.settings
}

// (поле, тип, ключ, сеттер, сигнал) — одна строка на свойство
macro_rules! settings {
    ($( $field:ident : $ty:ty = $key:ident, $setter:ident, $signal:ident; )*) => {
        pub struct SettingsRust {
            $( $field: $ty, )*
        }

        impl Default for SettingsRust {
            fn default() -> Self {
                let store = store();
                Self { $( $field: <$ty as Field>::load(store, SettingKey::$key), )* }
            }
        }

        impl qobject::Settings {
            $(
                fn $setter(mut self: Pin<&mut Self>, value: $ty) {
                    if self.$field == value {
                        return;
                    }
                    self.as_mut().rust_mut().$field = value.clone();
                    store().set(SettingKey::$key, value.to_value());
                    self.$signal();
                }
            )*

            /// Значение изменилось в хранилище (не через этот мост) — перечитать и оповестить.
            fn reload(mut self: Pin<&mut Self>, key: SettingKey) {
                match key {
                    $(
                        SettingKey::$key => {
                            let value = <$ty as Field>::load(store(), key);
                            if self.$field != value {
                                self.as_mut().rust_mut().$field = value;
                                self.$signal();
                            }
                        }
                    )*
                }
            }
        }
    };
}

settings! {
    theme_mode: QString = ThemeMode, set_theme_mode, theme_mode_changed;
    scheme_variant: QString = SchemeVariant, set_scheme_variant, scheme_variant_changed;
    accent_from_cover: bool = AccentFromCover, set_accent_from_cover, accent_from_cover_changed;
    custom_seed: QString = CustomSeed, set_custom_seed, custom_seed_changed;
    window_buttons: bool = WindowButtons, set_window_buttons, window_buttons_changed;
    track_transition: QString = TrackTransition, set_track_transition, track_transition_changed;
    backdrop_mode: QString = BackdropMode, set_backdrop_mode, backdrop_mode_changed;
    now_playing_blur: f64 = NowPlayingBlur, set_now_playing_blur, now_playing_blur_changed;
    now_playing_drift: f64 = NowPlayingDrift, set_now_playing_drift, now_playing_drift_changed;
    ambient_background: bool = AmbientBackground, set_ambient_background, ambient_background_changed;
    cover_spin: bool = CoverSpin, set_cover_spin, cover_spin_changed;
    cover_pulse: bool = CoverPulse, set_cover_pulse, cover_pulse_changed;
    calm_when_inactive: bool = CalmWhenInactive, set_calm_when_inactive, calm_when_inactive_changed;
    right_panel_open: bool = RightPanelOpen, set_right_panel_open, right_panel_open_changed;
    right_panel_tab: QString = RightPanelTab, set_right_panel_tab, right_panel_tab_changed;
    sidebar_collapsed: bool = SidebarCollapsed, set_sidebar_collapsed, sidebar_collapsed_changed;
    window_width: i32 = WindowWidth, set_window_width, window_width_changed;
    window_height: i32 = WindowHeight, set_window_height, window_height_changed;
    window_x: i32 = WindowX, set_window_x, window_x_changed;
    window_y: i32 = WindowY, set_window_y, window_y_changed;
    window_maximized: bool = WindowMaximized, set_window_maximized, window_maximized_changed;
    quality: QString = Quality, set_quality, quality_changed;
    volume: f64 = Volume, set_volume, volume_changed;
    crossfade: i32 = Crossfade, set_crossfade, crossfade_changed;
    wave_seeds: QString = WaveSeeds, set_wave_seeds, wave_seeds_changed;
    close_to_tray: bool = CloseToTray, set_close_to_tray, close_to_tray_changed;
    wheel_step: i32 = WheelStep, set_wheel_step, wheel_step_changed;
}

impl qobject::Settings {
    /// Windows: рамку рисует приложение, кнопки окна нужны всегда (запись сохраняется, но не влияет).
    fn window_buttons_value(&self) -> bool {
        cfg!(target_os = "windows") || self.window_buttons
    }

    fn on_store(mut self: Pin<&mut Self>, delivery: Delivery<SettingKey>) {
        match delivery {
            Delivery::Event(key) => self.reload(key),
            Delivery::Lagged => {
                for key in SettingKey::ALL {
                    self.as_mut().reload(key);
                }
            }
        }
    }
}

impl cxx_qt::Initialize for qobject::Settings {
    fn initialize(self: Pin<&mut Self>) {
        forward_events(self.qt_thread(), store().subscribe(), Self::on_store);
    }
}
