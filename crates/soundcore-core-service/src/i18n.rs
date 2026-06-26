//! Localization for the egui GUI.
//!
//! Not every [`Key`] is wired into [`crate::gui`] yet; allow dead code so
//! the translation table can be completed incrementally without warnings.
#![allow(dead_code)]
//!
//!
//! Strings live in a compile-checked table: every [`Key`] maps to exactly
//! one translation per [`Lang`]. The active language defaults to the
//! Windows display language and can be overridden from the Settings tab
//! (persisted in config.json as `ui_language`).
//!
//! Also hosts [`install_system_fonts`], which appends Windows system
//! fonts (Segoe UI + Microsoft YaHei) to egui's font stack so Cyrillic
//! and CJK render correctly without embedding multi-megabyte fonts.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lang {
    #[default]
    En = 0,
    Ru = 1,
    Uk = 2,
    De = 3,
    Fr = 4,
    Es = 5,
    ZhCn = 6,
}

impl Lang {
    pub const ALL: [Lang; 7] = [
        Lang::En,
        Lang::Ru,
        Lang::Uk,
        Lang::De,
        Lang::Fr,
        Lang::Es,
        Lang::ZhCn,
    ];

    /// BCP-47-ish tag used in config.json.
    pub fn tag(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Ru => "ru",
            Lang::Uk => "uk",
            Lang::De => "de",
            Lang::Fr => "fr",
            Lang::Es => "es",
            Lang::ZhCn => "zh-CN",
        }
    }

    /// Native (untranslated) display name for the language picker.
    pub fn native_name(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::Ru => "Русский",
            Lang::Uk => "Українська",
            Lang::De => "Deutsch",
            Lang::Fr => "Français",
            Lang::Es => "Español",
            Lang::ZhCn => "简体中文",
        }
    }

    pub fn from_tag(tag: &str) -> Option<Lang> {
        let lower = tag.to_ascii_lowercase();
        let primary = lower.split(['-', '_']).next().unwrap_or("");
        match primary {
            "en" => Some(Lang::En),
            "ru" => Some(Lang::Ru),
            "uk" => Some(Lang::Uk),
            "de" => Some(Lang::De),
            "fr" => Some(Lang::Fr),
            "es" => Some(Lang::Es),
            "zh" => Some(Lang::ZhCn),
            _ => None,
        }
    }

    /// Best match for the current Windows display language.
    pub fn system_default() -> Lang {
        #[link(name = "kernel32")]
        extern "system" {
            fn GetUserDefaultUILanguage() -> u16;
        }
        // Primary language ID lives in the low 10 bits of the LANGID.
        let primary = unsafe { GetUserDefaultUILanguage() } & 0x3FF;
        match primary {
            0x19 => Lang::Ru,
            0x22 => Lang::Uk,
            0x07 => Lang::De,
            0x0C => Lang::Fr,
            0x0A => Lang::Es,
            0x04 => Lang::ZhCn,
            _ => Lang::En,
        }
    }
}

/// Translate a key. `tr!` in the GUI wraps this.
pub fn tr(lang: Lang, key: Key) -> &'static str {
    strings(key)[lang as usize]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    // Tabs
    TabDevices,
    TabPerApp,
    TabMicLock,
    TabCamera,
    TabVst,
    TabSetup,
    // Top bar
    Refresh,
    MicLockActive,
    ServiceLabel,
    SvcRunning,
    SvcStopped,
    SvcNotInstalled,
    SvcTransitioning,
    // Devices tab
    DevicesHeading,
    PlaybackFmt,
    CaptureFmt,
    ColName,
    ColSampleRate,
    ColChannels,
    ColVolume,
    ColFlags,
    FlagDefault,
    FlagComms,
    FlagMuted,
    NoDevices,
    // Per-app tab
    PerAppHeading,
    PerAppStubBanner,
    FilterLabel,
    ColPid,
    ColImage,
    ColPath,
    // Mic-lock tab
    MicLockHeading,
    MicLockDesc,
    MicLockSvcRunningBanner,
    MicLockSvcHintBanner,
    Enabled,
    CaptureDevice,
    PickDevice,
    DefaultSuffix,
    LockedVolume,
    AlsoLockMute,
    WhitelistLabel,
    Apply,
    PickDeviceFirst,
    // Camera tab
    CameraHeading,
    CameraRunningBanner,
    CameraIdleBanner,
    CameraEnable,
    CameraSource,
    PickCamera,
    CameraResolution,
    CameraRefreshList,
    CameraPersistedFmt,
    CamerasOnSystemFmt,
    // VST tab
    VstHeading,
    VstBanner,
    Rescan,
    ChainFmt,
    SaveChain,
    SaveChainHint,
    ClearChain,
    DiscoveredPlugins,
    ActiveChain,
    PressRescan,
    ChainEmpty,
    AddToChain,
    Remove,
    VstRestartWarning,
    // Setup tab
    SetupHeading,
    SvcHeader,
    SvcDesc,
    StatusLabel,
    InstallStart,
    StartService,
    StopService,
    UninstallService,
    DllHeader,
    DllDesc,
    ReRegister,
    Unregister,
    ProbeHeader,
    ProbeDesc,
    RunProbe,
    FeaturesHeader,
    InstallDirFmt,
    ConfigDirFmt,
    LogsDirFmt,
    // Settings / language
    LanguageLabel,
    ThemeLabel,
    ThemeSystem,
    ThemeLight,
    ThemeDark,
    AppearanceHeader,
}

/// Order: [En, Ru, Uk, De, Fr, Es, ZhCn]
fn strings(key: Key) -> [&'static str; 7] {
    match key {
        // ----- Tabs -----
        Key::TabDevices => [
            "Devices", "Устройства", "Пристрої", "Geräte", "Périphériques", "Dispositivos", "设备",
        ],
        Key::TabPerApp => [
            "Per-app", "Приложения", "Застосунки", "Pro App", "Par application", "Por aplicación", "按应用",
        ],
        Key::TabMicLock => [
            "Mic lock", "Защита микрофона", "Захист мікрофона", "Mikrofonschutz", "Protection micro", "Bloqueo de micro", "麦克风锁定",
        ],
        Key::TabCamera => [
            "Camera", "Камера", "Камера", "Kamera", "Caméra", "Cámara", "摄像头",
        ],
        Key::TabVst => ["VST", "VST", "VST", "VST", "VST", "VST", "VST"],
        Key::TabSetup => [
            "Setup", "Настройка", "Налаштування", "Einrichtung", "Configuration", "Configuración", "设置",
        ],
        // ----- Top bar -----
        Key::Refresh => [
            "Refresh", "Обновить", "Оновити", "Aktualisieren", "Actualiser", "Actualizar", "刷新",
        ],
        Key::MicLockActive => [
            "mic lock active", "защита микрофона активна", "захист мікрофона активний",
            "Mikrofonschutz aktiv", "protection micro active", "bloqueo de micro activo", "麦克风锁定已启用",
        ],
        Key::ServiceLabel => [
            "service", "служба", "служба", "Dienst", "service", "servicio", "服务",
        ],
        Key::SvcRunning => [
            "running", "запущена", "запущена", "wird ausgeführt", "en cours", "en ejecución", "运行中",
        ],
        Key::SvcStopped => [
            "stopped", "остановлена", "зупинена", "angehalten", "arrêté", "detenido", "已停止",
        ],
        Key::SvcNotInstalled => [
            "not installed", "не установлена", "не встановлена", "nicht installiert", "non installé", "no instalado", "未安装",
        ],
        Key::SvcTransitioning => [
            "transitioning…", "переключение…", "перемикання…", "Übergang…", "transition…", "cambiando…", "切换中…",
        ],
        // ----- Devices tab -----
        Key::DevicesHeading => [
            "Audio devices", "Аудиоустройства", "Аудіопристрої", "Audiogeräte", "Périphériques audio", "Dispositivos de audio", "音频设备",
        ],
        Key::PlaybackFmt => [
            "Playback ({})", "Воспроизведение ({})", "Відтворення ({})", "Wiedergabe ({})", "Lecture ({})", "Reproducción ({})", "播放（{}）",
        ],
        Key::CaptureFmt => [
            "Recording ({})", "Запись ({})", "Запис ({})", "Aufnahme ({})", "Enregistrement ({})", "Grabación ({})", "录制（{}）",
        ],
        Key::ColName => [
            "Name", "Название", "Назва", "Name", "Nom", "Nombre", "名称",
        ],
        Key::ColSampleRate => [
            "Sample rate", "Частота", "Частота", "Abtastrate", "Fréquence", "Frecuencia", "采样率",
        ],
        Key::ColChannels => [
            "Channels", "Каналы", "Канали", "Kanäle", "Canaux", "Canales", "声道",
        ],
        Key::ColVolume => [
            "Volume", "Громкость", "Гучність", "Lautstärke", "Volume", "Volumen", "音量",
        ],
        Key::ColFlags => [
            "Flags", "Метки", "Мітки", "Status", "Statut", "Estado", "状态",
        ],
        Key::FlagDefault => [
            "default", "по умолчанию", "за замовчуванням", "Standard", "par défaut", "predeterminado", "默认",
        ],
        Key::FlagComms => [
            "comms", "для связи", "для зв'язку", "Kommunikation", "communications", "comunicaciones", "通信",
        ],
        Key::FlagMuted => [
            "muted", "без звука", "без звуку", "stumm", "muet", "silenciado", "静音",
        ],
        Key::NoDevices => [
            "(no devices)", "(нет устройств)", "(немає пристроїв)", "(keine Geräte)", "(aucun périphérique)", "(sin dispositivos)", "（无设备）",
        ],
        // ----- Per-app tab -----
        Key::PerAppHeading => [
            "Running processes", "Запущенные процессы", "Запущені процеси", "Laufende Prozesse", "Processus en cours", "Procesos en ejecución", "正在运行的进程",
        ],
        Key::PerAppStubBanner => [
            "Per-application FX (binding a VST chain to a process) is not implemented yet. It needs the Process Loopback API plus the VST scanner. See Setup → Feature status.",
            "Эффекты для отдельных приложений (привязка VST-цепочки к процессу) пока не реализованы. Нужны Process Loopback API и сканер VST. См. Настройка → статус функций.",
            "Ефекти для окремих застосунків (прив'язка VST-ланцюжка до процесу) поки не реалізовані. Потрібні Process Loopback API і сканер VST. Див. Налаштування → статус функцій.",
            "Effekte pro Anwendung (VST-Kette an einen Prozess binden) sind noch nicht implementiert. Benötigt die Process-Loopback-API und den VST-Scanner. Siehe Einrichtung → Funktionsstatus.",
            "Les effets par application (liaison d'une chaîne VST à un processus) ne sont pas encore implémentés. Nécessite l'API Process Loopback et le scanner VST. Voir Configuration → état des fonctions.",
            "Los efectos por aplicación (vincular una cadena VST a un proceso) aún no están implementados. Requiere la API Process Loopback y el escáner VST. Vea Configuración → estado de funciones.",
            "按应用效果（将 VST 链绑定到进程）尚未实现。需要 Process Loopback API 和 VST 扫描器。参见设置 → 功能状态。",
        ],
        Key::FilterLabel => [
            "Filter:", "Фильтр:", "Фільтр:", "Filter:", "Filtre :", "Filtro:", "筛选：",
        ],
        Key::ColPid => ["PID", "PID", "PID", "PID", "PID", "PID", "PID"],
        Key::ColImage => [
            "Image", "Имя файла", "Ім'я файлу", "Abbild", "Image", "Imagen", "映像",
        ],
        Key::ColPath => [
            "Path", "Путь", "Шлях", "Pfad", "Chemin", "Ruta", "路径",
        ],
        // ----- Mic-lock tab -----
        Key::MicLockHeading => [
            "Microphone volume lock", "Защита громкости микрофона", "Захист гучності мікрофона",
            "Mikrofonlautstärke sperren", "Verrouillage du volume du micro", "Bloqueo del volumen del micrófono", "麦克风音量锁定",
        ],
        Key::MicLockDesc => [
            "Stops any app (Chrome / Google Meet auto-gain) from changing the microphone volume. A worker thread compares current vs. target every 15 ms and re-applies the target — robust even against apps spamming one value.",
            "Запрещает любому приложению (AutoGain в Chrome / Google Meet) менять громкость микрофона. Рабочий поток каждые 15 мс сравнивает текущее значение с целевым и возвращает целевое — устойчиво даже к спаму одним значением.",
            "Забороняє будь-якому застосунку (AutoGain у Chrome / Google Meet) змінювати гучність мікрофона. Робочий потік кожні 15 мс порівнює поточне значення з цільовим і повертає цільове — стійко навіть до спаму одним значенням.",
            "Verhindert, dass Apps (Chrome / Google Meet Auto-Gain) die Mikrofonlautstärke ändern. Ein Worker-Thread vergleicht alle 15 ms Ist und Soll und stellt den Sollwert wieder her — robust auch gegen Apps, die einen Wert dauerhaft setzen.",
            "Empêche toute application (gain automatique de Chrome / Google Meet) de modifier le volume du micro. Un thread compare la valeur actuelle à la cible toutes les 15 ms et rétablit la cible — robuste même face aux applications insistantes.",
            "Impide que cualquier aplicación (ganancia automática de Chrome / Google Meet) cambie el volumen del micrófono. Un hilo compara cada 15 ms el valor actual con el objetivo y lo restablece — robusto incluso ante aplicaciones insistentes.",
            "阻止任何应用（Chrome / Google Meet 自动增益）更改麦克风音量。工作线程每 15 毫秒比较当前值与目标值并恢复目标值——即使应用反复设置同一数值也能稳定工作。",
        ],
        Key::MicLockSvcRunningBanner => [
            "The SoundCore service is running → the mic lock keeps working in the background even when this window is closed.",
            "Служба SoundCore запущена → защита микрофона работает в фоне даже при закрытом окне.",
            "Служба SoundCore запущена → захист мікрофона працює у фоні навіть із закритим вікном.",
            "Der SoundCore-Dienst läuft → der Mikrofonschutz arbeitet im Hintergrund weiter, auch wenn dieses Fenster geschlossen ist.",
            "Le service SoundCore est en cours d'exécution → la protection du micro continue en arrière-plan même fenêtre fermée.",
            "El servicio SoundCore está en ejecución → el bloqueo del micrófono sigue funcionando en segundo plano aunque esta ventana esté cerrada.",
            "SoundCore 服务正在运行 → 即使关闭此窗口，麦克风锁定仍在后台工作。",
        ],
        Key::MicLockSvcHintBanner => [
            "Want the lock to work without keeping the app open? Install the background service in Setup → Background service. One click.",
            "Хотите, чтобы защита работала без открытого приложения? Установите фоновую службу: Настройка → Фоновая служба. Один клик.",
            "Хочете, щоб захист працював без відкритого застосунку? Установіть фонову службу: Налаштування → Фонова служба. Один клік.",
            "Soll die Sperre ohne geöffnete App funktionieren? Installieren Sie den Hintergrunddienst unter Einrichtung → Hintergrunddienst. Ein Klick.",
            "Vous voulez que le verrouillage fonctionne sans garder l'application ouverte ? Installez le service en arrière-plan : Configuration → Service en arrière-plan. Un clic.",
            "¿Quiere que el bloqueo funcione sin mantener la aplicación abierta? Instale el servicio en segundo plano: Configuración → Servicio en segundo plano. Un clic.",
            "想在不打开应用的情况下保持锁定？在设置 → 后台服务中安装服务，一键完成。",
        ],
        Key::Enabled => [
            "Enabled", "Включено", "Увімкнено", "Aktiviert", "Activé", "Activado", "已启用",
        ],
        Key::CaptureDevice => [
            "Microphone", "Микрофон", "Мікрофон", "Mikrofon", "Microphone", "Micrófono", "麦克风",
        ],
        Key::PickDevice => [
            "— pick a device —", "— выберите устройство —", "— виберіть пристрій —",
            "— Gerät wählen —", "— choisir un périphérique —", "— elija un dispositivo —", "— 选择设备 —",
        ],
        Key::DefaultSuffix => [
            " (default)", " (по умолчанию)", " (за замовчуванням)", " (Standard)", " (par défaut)", " (predeterminado)", "（默认）",
        ],
        Key::LockedVolume => [
            "Locked volume %", "Зафиксированная громкость %", "Зафіксована гучність %",
            "Gesperrte Lautstärke %", "Volume verrouillé %", "Volumen bloqueado %", "锁定音量 %",
        ],
        Key::AlsoLockMute => [
            "Also lock mute state", "Также фиксировать «без звука»", "Також фіксувати «без звуку»",
            "Auch Stummschaltung sperren", "Verrouiller aussi la sourdine", "Bloquear también el silencio", "同时锁定静音状态",
        ],
        Key::WhitelistLabel => [
            "Allowed apps (one per line; e.g. OBS64.exe, Streamlabs*.exe):",
            "Разрешённые приложения (по одному в строке; напр. OBS64.exe, Streamlabs*.exe):",
            "Дозволені застосунки (по одному в рядку; напр. OBS64.exe, Streamlabs*.exe):",
            "Erlaubte Apps (eine pro Zeile; z. B. OBS64.exe, Streamlabs*.exe):",
            "Applications autorisées (une par ligne ; p. ex. OBS64.exe, Streamlabs*.exe) :",
            "Aplicaciones permitidas (una por línea; p. ej. OBS64.exe, Streamlabs*.exe):",
            "允许的应用（每行一个；例如 OBS64.exe、Streamlabs*.exe）：",
        ],
        Key::Apply => [
            "Apply", "Применить", "Застосувати", "Übernehmen", "Appliquer", "Aplicar", "应用",
        ],
        Key::PickDeviceFirst => [
            "Pick a device first.", "Сначала выберите устройство.", "Спочатку виберіть пристрій.",
            "Wählen Sie zuerst ein Gerät.", "Choisissez d'abord un périphérique.", "Primero elija un dispositivo.", "请先选择设备。",
        ],
        // ----- Camera tab -----
        Key::CameraHeading => [
            "Camera sharing", "Совместный доступ к камере", "Спільний доступ до камери",
            "Kamerafreigabe", "Partage de caméra", "Uso compartido de cámara", "摄像头共享",
        ],
        Key::CameraRunningBanner => [
            "Camera producer is running. Frames are published to the shared-memory channel and served to every connected consumer.",
            "Производитель кадров запущен. Кадры публикуются в канал общей памяти и раздаются всем подключённым потребителям.",
            "Виробник кадрів запущений. Кадри публікуються в канал спільної пам'яті та роздаються всім підключеним споживачам.",
            "Der Kamera-Producer läuft. Frames werden in den Shared-Memory-Kanal veröffentlicht und an alle verbundenen Verbraucher verteilt.",
            "Le producteur de caméra est actif. Les images sont publiées dans le canal de mémoire partagée et servies à tous les consommateurs connectés.",
            "El productor de cámara está activo. Los fotogramas se publican en el canal de memoria compartida y se sirven a todos los consumidores conectados.",
            "摄像头生产者正在运行。帧被发布到共享内存通道，并分发给所有已连接的消费者。",
        ],
        Key::CameraIdleBanner => [
            "Pick a physical camera below and press Apply. SoundCore opens it once through Media Foundation and serves frames to every app that uses it.",
            "Выберите физическую камеру ниже и нажмите «Применить». SoundCore откроет её один раз через Media Foundation и будет раздавать кадры всем приложениям.",
            "Виберіть фізичну камеру нижче й натисніть «Застосувати». SoundCore відкриє її один раз через Media Foundation і роздаватиме кадри всім застосункам.",
            "Wählen Sie unten eine physische Kamera und klicken Sie auf Übernehmen. SoundCore öffnet sie einmal über Media Foundation und verteilt die Frames an alle Apps.",
            "Choisissez une caméra physique ci-dessous et cliquez sur Appliquer. SoundCore l'ouvre une seule fois via Media Foundation et sert les images à toutes les applications.",
            "Elija una cámara física abajo y pulse Aplicar. SoundCore la abre una sola vez mediante Media Foundation y sirve los fotogramas a todas las aplicaciones.",
            "在下方选择物理摄像头并点击「应用」。SoundCore 通过 Media Foundation 打开它一次，并向所有使用它的应用分发帧。",
        ],
        Key::CameraEnable => [
            "Enable camera sharing", "Включить совместный доступ", "Увімкнути спільний доступ",
            "Kamerafreigabe aktivieren", "Activer le partage de caméra", "Activar uso compartido", "启用摄像头共享",
        ],
        Key::CameraSource => [
            "Physical camera", "Физическая камера", "Фізична камера", "Physische Kamera", "Caméra physique", "Cámara física", "物理摄像头",
        ],
        Key::PickCamera => [
            "— pick a camera —", "— выберите камеру —", "— виберіть камеру —",
            "— Kamera wählen —", "— choisir une caméra —", "— elija una cámara —", "— 选择摄像头 —",
        ],
        Key::CameraResolution => [
            "Resolution / fps", "Разрешение / к/с", "Роздільна здатність / к/с",
            "Auflösung / fps", "Résolution / ips", "Resolución / fps", "分辨率 / 帧率",
        ],
        Key::CameraRefreshList => [
            "Refresh camera list", "Обновить список камер", "Оновити список камер",
            "Kameraliste aktualisieren", "Actualiser la liste des caméras", "Actualizar lista de cámaras", "刷新摄像头列表",
        ],
        Key::CameraPersistedFmt => [
            "Saved: {} @ {}×{}", "Сохранено: {} @ {}×{}", "Збережено: {} @ {}×{}",
            "Gespeichert: {} @ {}×{}", "Enregistré : {} @ {}×{}", "Guardado: {} @ {}×{}", "已保存：{} @ {}×{}",
        ],
        Key::CamerasOnSystemFmt => [
            "Cameras in the system: {}", "Камер в системе: {}", "Камер у системі: {}",
            "Kameras im System: {}", "Caméras dans le système : {}", "Cámaras en el sistema: {}", "系统中的摄像头：{}",
        ],
        // ----- VST tab -----
        Key::VstHeading => [
            "VST3 plug-ins & effect chain", "Плагины VST3 и цепочка эффектов", "Плагіни VST3 і ланцюжок ефектів",
            "VST3-Plug-ins & Effektkette", "Plug-ins VST3 et chaîne d'effets", "Plugins VST3 y cadena de efectos", "VST3 插件与效果链",
        ],
        Key::VstBanner => [
            "The scanner looks for .vst3 in %CommonProgramFiles%\\VST3, %CommonProgramFiles(x86)%\\VST3 and %ProgramFiles%\\VSTPlugins. Plug-in metadata (name/vendor/UID) is filled in by the APO on first real load.",
            "Сканер ищет .vst3 в %CommonProgramFiles%\\VST3, %CommonProgramFiles(x86)%\\VST3 и %ProgramFiles%\\VSTPlugins. Метаданные плагина (имя/вендор/UID) подтягиваются APO при первой реальной загрузке.",
            "Сканер шукає .vst3 у %CommonProgramFiles%\\VST3, %CommonProgramFiles(x86)%\\VST3 та %ProgramFiles%\\VSTPlugins. Метадані плагіна (ім'я/вендор/UID) підтягуються APO під час першого реального завантаження.",
            "Der Scanner sucht .vst3 in %CommonProgramFiles%\\VST3, %CommonProgramFiles(x86)%\\VST3 und %ProgramFiles%\\VSTPlugins. Plug-in-Metadaten (Name/Hersteller/UID) ergänzt das APO beim ersten echten Laden.",
            "Le scanner cherche les .vst3 dans %CommonProgramFiles%\\VST3, %CommonProgramFiles(x86)%\\VST3 et %ProgramFiles%\\VSTPlugins. Les métadonnées du plug-in (nom/éditeur/UID) sont complétées par l'APO au premier chargement réel.",
            "El escáner busca .vst3 en %CommonProgramFiles%\\VST3, %CommonProgramFiles(x86)%\\VST3 y %ProgramFiles%\\VSTPlugins. Los metadatos del plugin (nombre/fabricante/UID) los completa el APO en la primera carga real.",
            "扫描器在 %CommonProgramFiles%\\VST3、%CommonProgramFiles(x86)%\\VST3 和 %ProgramFiles%\\VSTPlugins 中查找 .vst3。插件元数据（名称/厂商/UID）由 APO 在首次实际加载时填充。",
        ],
        Key::Rescan => [
            "Rescan", "Пересканировать", "Перескануати", "Neu scannen", "Réanalyser", "Volver a buscar", "重新扫描",
        ],
        Key::ChainFmt => [
            "Chain: {} plug-in(s)", "Цепочка: {} плаг.", "Ланцюжок: {} плаг.",
            "Kette: {} Plug-in(s)", "Chaîne : {} plug-in(s)", "Cadena: {} plugin(s)", "效果链：{} 个插件",
        ],
        Key::SaveChain => [
            "Save chain", "Сохранить цепочку", "Зберегти ланцюжок", "Kette speichern", "Enregistrer la chaîne", "Guardar cadena", "保存效果链",
        ],
        Key::SaveChainHint => [
            "writes chain.txt", "записывает chain.txt", "записує chain.txt", "schreibt chain.txt", "écrit chain.txt", "escribe chain.txt", "写入 chain.txt",
        ],
        Key::ClearChain => [
            "Clear chain", "Очистить цепочку", "Очистити ланцюжок", "Kette leeren", "Vider la chaîne", "Vaciar cadena", "清空效果链",
        ],
        Key::DiscoveredPlugins => [
            "Discovered plug-ins", "Найденные плагины", "Знайдені плагіни", "Gefundene Plug-ins", "Plug-ins découverts", "Plugins detectados", "已发现的插件",
        ],
        Key::ActiveChain => [
            "Active chain (top → bottom)", "Активная цепочка (сверху → вниз)", "Активний ланцюжок (зверху → вниз)",
            "Aktive Kette (oben → unten)", "Chaîne active (haut → bas)", "Cadena activa (arriba → abajo)", "当前效果链（从上到下）",
        ],
        Key::PressRescan => [
            "(press Rescan)", "(нажмите «Пересканировать»)", "(натисніть «Перескануати»)",
            "(Neu scannen drücken)", "(cliquez sur Réanalyser)", "(pulse Volver a buscar)", "（请点击重新扫描）",
        ],
        Key::ChainEmpty => [
            "(chain is empty)", "(цепочка пуста)", "(ланцюжок порожній)", "(Kette ist leer)", "(chaîne vide)", "(cadena vacía)", "（效果链为空）",
        ],
        Key::AddToChain => [
            "Add to chain", "Добавить в цепочку", "Додати до ланцюжка", "Zur Kette hinzufügen", "Ajouter à la chaîne", "Añadir a la cadena", "添加到效果链",
        ],
        Key::Remove => [
            "Remove", "Удалить", "Видалити", "Entfernen", "Supprimer", "Quitar", "移除",
        ],
        Key::VstRestartWarning => [
            "After “Save chain”, toggle the device off/on in Windows sound settings so audiodg.exe re-reads chain.txt and the APO loads the plug-ins.",
            "После «Сохранить цепочку» выключите/включите устройство в настройках звука Windows, чтобы audiodg.exe перечитал chain.txt и APO загрузил плагины.",
            "Після «Зберегти ланцюжок» вимкніть/увімкніть пристрій у налаштуваннях звуку Windows, щоб audiodg.exe перечитав chain.txt і APO завантажив плагіни.",
            "Schalten Sie nach „Kette speichern“ das Gerät in den Windows-Soundeinstellungen aus/ein, damit audiodg.exe chain.txt neu liest und das APO die Plug-ins lädt.",
            "Après « Enregistrer la chaîne », désactivez/réactivez le périphérique dans les paramètres audio de Windows pour qu'audiodg.exe relise chain.txt et que l'APO charge les plug-ins.",
            "Tras «Guardar cadena», desactive/active el dispositivo en la configuración de sonido de Windows para que audiodg.exe relea chain.txt y el APO cargue los plugins.",
            "点击「保存效果链」后，请在 Windows 声音设置中关闭/重新打开设备，让 audiodg.exe 重新读取 chain.txt 并由 APO 加载插件。",
        ],
        // ----- Setup tab -----
        Key::SetupHeading => [
            "Setup", "Настройка", "Налаштування", "Einrichtung", "Configuration", "Configuración", "设置",
        ],
        Key::SvcHeader => [
            "Background service (mic lock without the GUI open)",
            "Фоновая служба (защита микрофона без открытого окна)",
            "Фонова служба (захист мікрофона без відкритого вікна)",
            "Hintergrunddienst (Mikrofonschutz ohne geöffnete App)",
            "Service en arrière-plan (protection micro sans fenêtre ouverte)",
            "Servicio en segundo plano (bloqueo de micro sin ventana abierta)",
            "后台服务（无需打开窗口即可锁定麦克风）",
        ],
        Key::SvcDesc => [
            "Install the service and the mic lock keeps working in the background — it survives closing the GUI and rebooting. The closest you can get to “Windows can't change the volume” without a kernel driver.",
            "Установите службу — и защита микрофона будет работать в фоне: переживёт закрытие окна и перезагрузку. Самый близкий путь к «Windows не сможет менять громкость» без kernel-драйвера.",
            "Установіть службу — і захист мікрофона працюватиме у фоні: переживе закриття вікна та перезавантаження. Найближчий шлях до «Windows не зможе змінювати гучність» без kernel-драйвера.",
            "Installieren Sie den Dienst und der Mikrofonschutz arbeitet im Hintergrund — er überlebt das Schließen der App und einen Neustart. So nah kommt man ohne Kerneltreiber an „Windows kann die Lautstärke nicht ändern“ heran.",
            "Installez le service et la protection du micro continue en arrière-plan — elle survit à la fermeture de l'application et au redémarrage. Ce qui se rapproche le plus de « Windows ne peut pas changer le volume » sans pilote noyau.",
            "Instale el servicio y el bloqueo del micrófono seguirá funcionando en segundo plano: sobrevive al cierre de la aplicación y al reinicio. Lo más cercano a «Windows no puede cambiar el volumen» sin un controlador de kernel.",
            "安装服务后，麦克风锁定将在后台持续工作——关闭窗口或重启后依然有效。这是在没有内核驱动的情况下最接近「Windows 无法更改音量」的方案。",
        ],
        Key::StatusLabel => [
            "Status:", "Статус:", "Статус:", "Status:", "Statut :", "Estado:", "状态：",
        ],
        Key::InstallStart => [
            "Install as Windows Service & start", "Установить как службу Windows и запустить", "Установити як службу Windows і запустити",
            "Als Windows-Dienst installieren & starten", "Installer comme service Windows et démarrer", "Instalar como servicio de Windows e iniciar", "安装为 Windows 服务并启动",
        ],
        Key::StartService => [
            "Start service", "Запустить службу", "Запустити службу", "Dienst starten", "Démarrer le service", "Iniciar servicio", "启动服务",
        ],
        Key::StopService => [
            "Stop service", "Остановить службу", "Зупинити службу", "Dienst anhalten", "Arrêter le service", "Detener servicio", "停止服务",
        ],
        Key::UninstallService => [
            "Uninstall service", "Удалить службу", "Видалити службу", "Dienst deinstallieren", "Désinstaller le service", "Desinstalar servicio", "卸载服务",
        ],
        Key::DllHeader => [
            "Embedded DLLs (APO + camera consumer)", "Встроенные DLL (APO + потребитель камеры)", "Вбудовані DLL (APO + споживач камери)",
            "Eingebettete DLLs (APO + Kamera-Consumer)", "DLL embarquées (APO + consommateur caméra)", "DLL integradas (APO + consumidor de cámara)", "内嵌 DLL（APO + 摄像头消费者）",
        ],
        Key::DllDesc => [
            "These DLLs are embedded in the .exe and were extracted + registered automatically on first launch. The buttons are for manual reinstall or diagnostics.",
            "Эти DLL встроены в .exe и автоматически распакованы и зарегистрированы при первом запуске. Кнопки — для ручной переустановки или диагностики.",
            "Ці DLL вбудовані в .exe і автоматично розпаковані та зареєстровані під час першого запуску. Кнопки — для ручного перевстановлення чи діагностики.",
            "Diese DLLs sind in die .exe eingebettet und wurden beim ersten Start automatisch entpackt und registriert. Die Schaltflächen dienen der manuellen Neuinstallation oder Diagnose.",
            "Ces DLL sont intégrées à l'.exe et ont été extraites + enregistrées automatiquement au premier lancement. Les boutons servent à la réinstallation manuelle ou au diagnostic.",
            "Estas DLL están integradas en el .exe y se extrajeron y registraron automáticamente en el primer inicio. Los botones son para reinstalación manual o diagnóstico.",
            "这些 DLL 内嵌在 .exe 中，首次启动时已自动解压并注册。按钮用于手动重装或诊断。",
        ],
        Key::ReRegister => [
            "Re-register", "Перерегистрировать", "Перереєструвати", "Neu registrieren", "Réenregistrer", "Volver a registrar", "重新注册",
        ],
        Key::Unregister => [
            "Unregister", "Отменить регистрацию", "Скасувати реєстрацію", "Registrierung aufheben", "Annuler l'enregistrement", "Anular registro", "取消注册",
        ],
        Key::ProbeHeader => [
            "AudioPolicyConfig probe (per-app routing)", "Проверка AudioPolicyConfig (маршрутизация по приложениям)", "Перевірка AudioPolicyConfig (маршрутизація за застосунками)",
            "AudioPolicyConfig-Test (App-Routing)", "Test AudioPolicyConfig (routage par application)", "Prueba de AudioPolicyConfig (enrutamiento por aplicación)", "AudioPolicyConfig 探测（按应用路由）",
        ],
        Key::ProbeDesc => [
            "Tests the undocumented IAudioPolicyConfigFactory: opens the COM object via the Win10 and Win11 CLSIDs and calls SetPersistedDefaultAudioEndpoint for this process. If it succeeds, per-app routing is feasible on this machine.",
            "Тест недокументированного IAudioPolicyConfigFactory: открывает COM-объект через CLSID Win10 и Win11 и вызывает SetPersistedDefaultAudioEndpoint для этого процесса. Если получилось — маршрутизация по приложениям на этой машине реализуема.",
            "Тест недокументованого IAudioPolicyConfigFactory: відкриває COM-об'єкт через CLSID Win10 та Win11 і викликає SetPersistedDefaultAudioEndpoint для цього процесу. Якщо вдалося — маршрутизація за застосунками на цій машині реалізовна.",
            "Testet die undokumentierte IAudioPolicyConfigFactory: öffnet das COM-Objekt über die Win10- und Win11-CLSIDs und ruft SetPersistedDefaultAudioEndpoint für diesen Prozess auf. Bei Erfolg ist App-Routing auf dieser Maschine machbar.",
            "Teste l'IAudioPolicyConfigFactory non documentée : ouvre l'objet COM via les CLSID Win10 et Win11 et appelle SetPersistedDefaultAudioEndpoint pour ce processus. En cas de succès, le routage par application est réalisable sur cette machine.",
            "Prueba la IAudioPolicyConfigFactory no documentada: abre el objeto COM mediante los CLSID de Win10 y Win11 y llama a SetPersistedDefaultAudioEndpoint para este proceso. Si funciona, el enrutamiento por aplicación es viable en esta máquina.",
            "测试未公开的 IAudioPolicyConfigFactory：通过 Win10 和 Win11 的 CLSID 打开 COM 对象，并为本进程调用 SetPersistedDefaultAudioEndpoint。如果成功，则此机器支持按应用路由。",
        ],
        Key::RunProbe => [
            "Run probe", "Запустить проверку", "Запустити перевірку", "Test ausführen", "Lancer le test", "Ejecutar prueba", "运行探测",
        ],
        Key::FeaturesHeader => [
            "Feature status (what actually works)", "Статус функций (что реально работает)", "Статус функцій (що реально працює)",
            "Funktionsstatus (was wirklich funktioniert)", "État des fonctions (ce qui marche vraiment)", "Estado de funciones (lo que realmente funciona)", "功能状态(实际可用情况)",
        ],
        Key::InstallDirFmt => [
            "Install dir: {}", "Папка установки: {}", "Тека встановлення: {}", "Installationsordner: {}", "Dossier d'installation : {}", "Carpeta de instalación: {}", "安装目录：{}",
        ],
        Key::ConfigDirFmt => [
            "Config dir : {}", "Папка конфигурации: {}", "Тека конфігурації: {}", "Konfigurationsordner: {}", "Dossier de configuration : {}", "Carpeta de configuración: {}", "配置目录：{}",
        ],
        Key::LogsDirFmt => [
            "Logs       : {}", "Журналы: {}", "Журнали: {}", "Protokolle: {}", "Journaux : {}", "Registros: {}", "日志目录：{}",
        ],
        // ----- Settings / language -----
        Key::LanguageLabel => [
            "Language", "Язык", "Мова", "Sprache", "Langue", "Idioma", "语言",
        ],
        Key::ThemeLabel => [
            "Theme", "Тема", "Тема", "Design", "Thème", "Tema", "主题",
        ],
        Key::ThemeSystem => [
            "System", "Как в Windows", "Як у Windows", "System", "Système", "Sistema", "跟随系统",
        ],
        Key::ThemeLight => [
            "Light", "Светлая", "Світла", "Hell", "Clair", "Claro", "浅色",
        ],
        Key::ThemeDark => [
            "Dark", "Тёмная", "Темна", "Dunkel", "Sombre", "Oscuro", "深色",
        ],
        Key::AppearanceHeader => [
            "Appearance", "Внешний вид", "Зовнішній вигляд", "Darstellung", "Apparence", "Apariencia", "外观",
        ],
    }
}

/// Replace each `{}` in a translated pattern with the next argument.
/// Tiny positional formatter so translations can reorder text around
/// values without `format!`'s compile-time literal requirement.
pub fn fmt(pattern: &str, args: &[&dyn std::fmt::Display]) -> String {
    let mut out = String::with_capacity(pattern.len() + 16);
    let mut rest = pattern;
    let mut i = 0;
    while let Some(pos) = rest.find("{}") {
        out.push_str(&rest[..pos]);
        if let Some(a) = args.get(i) {
            out.push_str(&a.to_string());
        }
        i += 1;
        rest = &rest[pos + 2..];
    }
    out.push_str(rest);
    out
}

/// Append Windows system fonts to egui so Cyrillic and CJK render
/// correctly: Segoe UI as the main proportional face, Microsoft YaHei
/// as the CJK fallback. Falls back silently to egui's built-ins when a
/// font file is missing (older/cut-down Windows).
pub fn install_system_fonts(ctx: &eframe::egui::Context) {
    use eframe::egui::{FontData, FontDefinitions, FontFamily};

    let mut fonts = FontDefinitions::default();
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".to_string());

    let mut add = |fonts: &mut FontDefinitions, name: &str, file: &str, prepend: bool| {
        let path = std::path::Path::new(&windir).join("Fonts").join(file);
        if let Ok(bytes) = std::fs::read(&path) {
            fonts
                .font_data
                .insert(name.to_string(), FontData::from_owned(bytes));
            let family = fonts
                .families
                .entry(FontFamily::Proportional)
                .or_default();
            if prepend {
                family.insert(0, name.to_string());
            } else {
                family.push(name.to_string());
            }
        }
    };

    // Primary UI face (full Latin + Cyrillic coverage, native look).
    add(&mut fonts, "segoe-ui", "segoeui.ttf", true);
    // CJK fallback for zh-CN (TTC: egui/ab_glyph reads the first face).
    add(&mut fonts, "msyh", "msyh.ttc", false);
    // Emoji/symbols used by tab icons.
    add(&mut fonts, "seguiemj", "seguiemj.ttf", false);

    ctx.set_fonts(fonts);
}
