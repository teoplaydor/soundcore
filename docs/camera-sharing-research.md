# Совместный доступ нескольких приложений к одной камере в Windows без виртуальной камеры

> Исследование для SoundCore. Цель: дать **нескольким** приложениям (Zoom, Chrome,
> OBS, Telegram, легаси-DirectShow-приложения) использовать **одну** физическую
> камеру **одновременно**, на Windows 10 и Windows 11, **без создания виртуальной
> камеры**, **без kernel-драйвера с WHQL/EV-подписью** и **без инъекций в чужие
> процессы**.
>
> Данные собраны многоагентным исследованием (7 направлений, факт-чек каждого
> ключевого утверждения по первоисточнику). Дата: июнь 2026.

---

## TL;DR (честный вывод)

1. **Архитектурно Windows уже мультиплексирует камеру.** Начиная с Windows 10 1607
   весь захват идёт через службу **Windows Camera Frame Server**
   («Enables multiple clients to access video frames from camera devices»). Один
   процесс‑служба открывает железо, остальные — её клиенты. Физически раздать кадры
   N приложениям ОС умеет давно.

2. **Мешает не техника, а политика арбитража.** Первое приложение, открывающее
   камеру, по умолчанию берёт её в режиме **ExclusiveControl**, и Frame Server
   отказывает остальным. Снять это ограничение «снаружи», за чужое приложение,
   штатно нельзя — приложение само должно было открыться в shared‑режиме.

3. **Полное решение задачи «много произвольных приложений, без виртуалки» существует
   только на Windows 11 24H2+** — это встроенный режим ОС **Multi‑App Camera**
   (переключатель «Разрешить нескольким приложениям использовать камеру»). Он снимает
   лимит «одно приложение за раз» на уровне самой ОС: Zoom и OBS видят настоящую
   камеру и оба получают кадры, ничего «опт‑ин» от них не требуется. От нас — только
   определить/подсказать включение, т.к. **API для программного включения нет**.

4. **На Windows 10 (и на Windows 11 до 24H2) задача в полной постановке
   неразрешима** без одного из двух: **виртуальной камеры** (которую просили
   избегать) **или DMFT** — пользовательского расширения драйвера камеры, которое
   ставится INF‑пакетом и требует **attestation‑подписи (нужен EV‑сертификат и
   Partner Center)**. Никакого «реестрового тумблера», заставляющего Frame Server
   обслуживать несколько произвольных клиентов на Win10, у Microsoft нет — это
   подтверждено ответами модераторов Microsoft Q&A.

5. **Следствие для продукта.** Существующая в репозитории архитектура
   (DirectShow Source Filter + shared‑memory ring) — это и есть «виртуальная
   камера». Исследование показывает: для произвольных приложений на Win10 её,
   по сути, **нечем заменить**. Рекомендуемая стратегия — **гибрид с деградацией**
   (см. раздел «Рекомендация»).

---

## 1. Frame Server — что уже умеет ОС

- Служба **`FrameServer`** («Windows Camera Frame Server»), `svchost.exe -k Camera`,
  `FrameServer.dll`, LocalSystem, запуск Manual, зависит от RPC. Официальное
  описание: *«Enables multiple clients to access video frames from camera devices»*.
  Есть в Windows 10 начиная с 1607. На Windows 11 добавлена вторая служба
  `FrameServerMonitor` (следит за состоянием первой).
  [services/frameserver](https://batcmd.com/windows/10/services/frameserver/) ·
  [services/frameservermonitor](https://batcmd.com/windows/11/services/frameservermonitor/)
- В 1607 захват перенесли внутрь процесса службы; приложения общаются с ней через
  `FSClient.dll`. Тогда же из стандартных API спрятали MJPEG/H.264 (расшифровать
  один раз и раздать) — это задело и DirectShow, и Media Foundation (Skype, OBS,
  Logitech C920/C930e). [alax.info/1686](https://alax.info/blog/1686)
- Низкоуровневые «сенсорные» API существуют с 1607: `MFCreateSensorGroup`,
  `IMFSensorDevice::SetSensorDeviceMode` (`MFSensorDeviceMode_Controller` =
  настройки менять можно / `MFSensorDeviceMode_Shared` = нельзя).
  [MFCreateSensorGroup](https://learn.microsoft.com/en-us/windows/win32/api/mfidl/nf-mfidl-mfcreatesensorgroup) ·
  [SetSensorDeviceMode](https://learn.microsoft.com/en-us/windows/win32/api/mfidl/nf-mfidl-imfsensordevice-setsensordevicemode)
- **Важно:** сама по себе sharing‑оринтированная архитектура не означает, что два
  произвольных приложения уживутся. Даже в 1607 две сессии захвата (два TopoEdit)
  падали. Раздача работает только если клиенты открываются в shared‑режиме.

---

## 2. Sharing modes — нативная раздача для «своих» приложений

- **WinRT `MediaCaptureSharingMode`** (с Windows 10 1607 / build 14393):
  `ExclusiveControl` (полный контроль, падает если камеру держит другой) и
  `SharedReadOnly` (получает кадры из уже используемого источника, **но не может
  менять его конфигурацию**). Рекомендуемый паттерн Microsoft — запросить
  ExclusiveControl, при неудаче откатиться на SharedReadOnly.
  [MediaCaptureSharingMode](https://learn.microsoft.com/en-us/uwp/api/windows.media.capture.mediacapturesharingmode?view=winrt-26100) ·
  [process-media-frames](https://learn.microsoft.com/en-us/windows/apps/develop/camera/process-media-frames-with-mediaframereader)
- В SharedReadOnly нельзя задать `PhotoMediaDescription/PreviewMediaDescription/
  RecordMediaDescription/VideoProfile`; можно `SourceGroup/VideoDeviceId/
  StreamingCaptureMode/...`. Смену формата владельцем читатель видит через
  `MediaFrameSource.FormatChanged` → `CurrentFormat`.
  [SharingMode prop](https://learn.microsoft.com/en-us/uwp/api/windows.media.capture.mediacaptureinitializationsettings.sharingmode?view=winrt-28000) ·
  [FormatChanged](https://learn.microsoft.com/en-us/uwp/api/windows.media.capture.frames.mediaframesource.formatchanged?view=winrt-26100)
- **Классический Win32 Media Foundation:** атрибут
  **`MF_DEVSOURCE_ATTRIBUTE_FRAMESERVER_SHARE_MODE`** при создании источника
  (`MFCreateDeviceSource`/`IMFActivate`) — Win32‑эквивалент SharedReadOnly. Но:
  **минимально поддерживаемый клиент — Windows 11 build 26100 (24H2)**, страница
  создана в июле 2024. **Для Windows 10 не документирован и не работает.**
  [MF_DEVSOURCE_ATTRIBUTE_FRAMESERVER_SHARE_MODE](https://learn.microsoft.com/en-us/windows/win32/medfound/mf-devsource-attribute-frameserver-share-mode)
- Есть ещё `MF_DEVICESTREAM_FRAMESERVER_SHARED` (Win10 1703, desktop) — но это
  документ‑заглушка без семантики; полагаться на него, чтобы заставить
  exclusive‑приложение делиться, нельзя.
  [MF_DEVICESTREAM_FRAMESERVER_SHARED](https://learn.microsoft.com/en-us/windows/win32/medfound/mf-devicestream-frameserver-shared)

**Ключевое ограничение sharing‑mode:** делиться должны *все* участники. Zoom, Chrome,
Teams открывают камеру в ExclusiveControl. Заставить их перейти в SharedReadOnly
«снаружи» нельзя. Поэтому SharedReadOnly решает «**мы** + кто‑то ещё», но не
«**они** друг с другом».

---

## 3. Windows 11 Multi‑App Camera — единственное полноценное нативное решение

- Функция «**Allow multiple apps to use the camera at the same time**» (разработана
  совместно с сообществом слабослышащих — для одновременного сурдопереводчика).
  - Insider Dev Channel **26120.2702** (декабрь 2024), демонстрация Camera + Teams.
    [robquickenden.blog](https://robquickenden.blog/2024/12/windows11-camera-in-multiapps/)
  - Release Preview **26100.3321 (KB5052093)**, объявлено 18 фев 2025.
    [blogs.windows.com](https://blogs.windows.com/windows-insider/2025/02/18/releasing-windows-11-build-26100-3321-to-the-release-preview-channel/)
  - Широкий выпуск **KB5089573** (build 26200.8524+, май 2026), всем — июньский
    Patch Tuesday 2026.
    [windowslatest](https://www.windowslatest.com/2026/05/30/microsoft-is-killing-the-one-app-at-a-time-camera-limit-in-windows-11-with-new-multi-app-mode/)
- Включается **на камеру**: Settings → Bluetooth & devices → Cameras → [камера] →
  доп. настройки → «Let multiple apps use your camera». При включении яркость и пр.
  настраиваются только из системных настроек, не из приложений.
  [betanews](https://betanews.com/article/at-long-last-microsoft-makes-it-possible-to-use-your-webcam-with-multiple-apps-simultaneously-in-windows-11/)
- **Нет ни API, ни документированного ключа реестра**, чтобы программно узнать или
  переключить состояние тумблера (проверяли `HKCU\Software\Microsoft\Camera` —
  бесполезно). Рекомендация Microsoft — поведенческое определение: попробовать
  открыть камеру вторым клиентом, пока она занята.
  [Microsoft Q&A](https://learn.microsoft.com/en-us/answers/questions/5565474/how-can-i-programmatically-tell-if-the-setting-to)
- Покрывает не все типы камер: настройки камеры Windows 11 не управляют ИК‑камерами,
  DirectShow‑камерами и проприетарными стеками.
  [support.microsoft.com](https://support.microsoft.com/en-us/windows/manage-cameras-with-camera-settings-in-windows-11-97997ed5-bb98-47b6-a13d-964106997757)

**На Windows 10 аналога нет.** Ответ Microsoft Q&A (20H2): официальной функции
мультиприложенческого доступа найти не удалось, пользователь ушёл на виртуальную
камеру. [Q&A 171402](https://learn.microsoft.com/en-us/answers/questions/171402/sharing-webcam-within-multiple-applications)

---

## 4. Реестр / политика Frame Server

- **`EnableFrameServerMode`** (DWORD под
  `HKLM\SOFTWARE\Microsoft\Windows Media Foundation\Platform`, плюс `WOW6432Node`
  для 32‑бит) — недокументированный, но широко известный. `=0` **выключает**
  frame‑server (исторический воркэраунд от зависаний веб‑камер после 1607). Это
  обратное тому, что нам нужно, и ломает MF‑клиентов.
  [thewindowsclub](https://www.thewindowsclub.com/windows-camera-frame-server-service-terminated-unexpectedly) ·
  [appuals](https://appuals.com/webcam-not-working-after-windows-10-anniversay-update/) ·
  [alax.info/1693](https://alax.info/blog/1693)
- **Реестрового тумблера, который заставил бы Frame Server обслуживать несколько
  произвольных клиентов, не существует** — ни на Win10, ни на Win11 (модераторы
  Microsoft на прямой вопрос предлагали только SharedReadOnly или DirectShow).
  [Q&A 218589](https://learn.microsoft.com/en-us/answers/questions/218589/how-to-get-the-frame-from-webcam-that-other-applic?page=1)

---

## 5. DMFT — путь «расширение драйвера без kernel‑драйвера»

- **Device MFT** — пользовательская DLL‑расширение драйвера камеры, которую конвейер
  захвата вставляет первым трансформом после железа. Видит кадры всех потоков,
  отдаёт сколько угодно выходных, через неё идут все управляющие вызовы (IKsControl).
  Загружается в 64‑битном процессе службы (а не пер‑приложение).
  [DMFT design](https://learn.microsoft.com/en-us/windows-hardware/drivers/stream/dmft-design)
- Цепочка DMFT: до 1703 — 1; 1703+ — до 2; Win11 22H2+ — до 4
  (`CameraDeviceMftCLSIDChain`, REG_MULTI_SZ).
- Для UVC‑камер (обычные USB‑вебкамеры) с 19H1 **свой драйвер запрещён**: «все
  UVC‑камеры обязаны использовать встроенный USB Video Class driver, а расширения —
  только в виде Device MFT». Ставится кастомным INF на основе inbox `USBVideo.INF`
  (`EnablePlatformDmft`, `CameraDeviceMftCLSIDChain`).
  [UVC guide](https://learn.microsoft.com/en-us/windows-hardware/drivers/stream/uvc-camera-implementation-guide)
- **Подпись:** драйвер‑пакет (INF+DLL в CAB) подписывается **EV‑сертификатом**,
  отправляется через Partner Center (attestation), Microsoft переподписывает.
  То есть DMFT **не** требует WHQL, но **требует EV‑сертификата и аккаунта Partner
  Center** — это ровно та «возня с подписью», которую просили избегать.
  [attestation signing](https://learn.microsoft.com/en-us/windows-hardware/drivers/dashboard/code-signing-attestation)
- DMFT даёт **обработку каждого кадра для всех приложений** (как Windows Studio
  Effects), но **сам по себе не включает мультиприложенческий доступ** и, помимо
  подписи, ограничен «один вендор DMFT на устройство». Studio Effects вообще работает
  только на встроенных камерах, не на внешних USB.
  [Studio Effects на USB](https://learn.microsoft.com/en-us/answers/questions/2313056/windows-studio-effects-on-usb-or-external-camera)

---

## 6. Контраст: виртуальные камеры (то, чего просили избегать)

- **`MFCreateVirtualCamera`** (Win11 22000+, `mfsensorgroup.dll`): чистый
  user‑mode software‑источник, хостится Frame Server, **без kernel‑драйвера**,
  виден **и MF, и DirectShow** приложениям. Но: ОС **принудительно дописывает
  «Windows Virtual Camera»** к имени (нельзя выдать за обычную камеру); AllUsers —
  только из‑под админа; **только Windows 11**; на Win10 не работает (нет служб).
  И — мультиприложенческая раздача самой виртуалки **не гарантирована** (VCamSample
  #13: Zoom и OBS не смогли одновременно).
  [MFCreateVirtualCamera](https://learn.microsoft.com/en-us/windows/win32/api/mfvirtualcamera/nf-mfvirtualcamera-mfcreatevirtualcamera) ·
  [alax.info/2245](https://alax.info/blog/2245) ·
  [VCamSample #13](https://github.com/smourier/VCamSample/issues/13) ·
  [VCamSample #11](https://github.com/smourier/VCamSample/issues/11)
- **DirectShow Source Filter** (Win10‑путь, как сейчас в SoundCore): виден **только**
  DirectShow‑приложениям той же разрядности, **не виден** MF/UWP‑приложениям.
  Фрагильно: OBS #8057 — Qt6/MF‑приложения не видят виртуалку OBS вовсе; akvirtualcamera
  прямо пишет «не работает в UWP и MF».
  [alax.info/1722](https://alax.info/blog/1722) ·
  [OBS #8057](https://github.com/obsproject/obs-studio/issues/8057) ·
  [akvirtualcamera support](https://github.com/webcamoid/akvirtualcamera/wiki/Support-status)
- **Все коммерческие конкуренты делают именно виртуальную камеру**, не делят
  физическое устройство: OBS (DirectShow + shared memory), ManyCam, SplitCam
  («SplitCam Video Driver», до 64 клиентов), e2eSoft VCam/CamSplitter (WDM‑драйвер),
  XSplit VCam, NVIDIA Broadcast («creates a virtual Windows camera»), Logitech
  Capture, Snap Camera (закрыта). **Ни один не мультиплексирует камеру нативно без
  предъявления нового (виртуального) устройства** — потому что иначе нельзя.
  [obs how-it-works](https://medium.com/deelvin-machine-learning/how-does-obs-virtual-camera-plugin-work-on-windows-e92ab8986c4e) ·
  [e2esoft camsplitter](https://www.e2esoft.com/camsplitter/) ·
  [nvidia broadcast faq](https://www.nvidia.com/en-us/geforce/broadcasting/broadcast-app/faq/) ·
  [splitcam](https://splitcam.com/) ·
  [softcam](https://github.com/tshino/softcam)

---

## 7. Сводная матрица

| Подход | Win10 | Win11 (<24H2) | Win11 24H2+ | Видят DShow-приложения | Видят MF/UWP-приложения | Без виртуалки | Подпись | Риск AV | Вердикт |
|---|---|---|---|---|---|---|---|---|---|
| **Win11 Multi‑App Camera (тумблер ОС)** | ✗ нет | ✗ нет | ✓ да | ✓ (настоящая камера) | ✓ | ✓ да | нет | нет | **Рекомендуется там, где есть** — но включает пользователь, API нет |
| **SharedReadOnly (WinRT) / FRAMESERVER_SHARE_MODE (Win32 MF)** | частично¹ | частично¹ | ✓ (Win32-атрибут с 26100) | только если так открылись | только если так открылись | ✓ да | нет | нет | Виабельно для **наших** клиентов, не для чужих |
| **Реестр EnableFrameServerMode** | ✗ (выключает, ломает) | ✗ | ✗ | — | — | — | нет | нет | **Не подходит** |
| **DMFT (расширение драйвера)** | ✓² | ✓² | ✓² | ✓ (обработка для всех) | ✓ | ✓ да | **EV + Partner Center (attestation)** | низкий | Обработка кадров для всех; **не** даёт мультидоступ; нужна EV‑подпись |
| **MFCreateVirtualCamera** | ✗ | ✓ | ✓ | ✓ | ✓ | ✗ виртуалка | админ для AllUsers | низкий | Контраст; Win11‑only; имя помечено; мультидоступ не гарантирован |
| **DirectShow Source Filter (текущий SoundCore)** | ✓ | ✓ | ✓ | ✓ | ✗ | ✗ виртуалка | нет | средний³ | Текущая реализация; единственный «без EV» путь на Win10 для DShow‑приложений |
| **Kernel/AVStream драйвер** | ✓ | ✓ | ✓ | ✓ | ✓ | зависит | **WHQL/EV** | низкий | Исключено условиями продукта |

¹ Только среди приложений, которые сами открылись в shared‑режиме (наши). Win32‑атрибут — только Win11 24H2+.
² SharedReadOnly среди клиентов всё равно нужен; DMFT решает «обработка для всех», а не «несколько эксклюзивных владельцев».
³ DirectShow‑фильтр + shared memory — паттерн OBS/ManyCam, коммерчески приемлем, но требует аккуратных ACL (см. найденные баги).

---

## 8. Рекомендация для SoundCore (гибрид с деградацией)

Условие «без виртуалки, на любой Windows, для любых приложений» в полной постановке
**неудовлетворимо** на Win10 и Win11 <24H2. Реалистичная стратегия — определять
возможности ОС и выбирать лучший доступный режим:

1. **Определение возможностей при старте** (раздел Camera в UI показывает явный
   статус режима):
   - Win11 build ≥ 26100 → проверить Win32‑атрибут `FRAMESERVER_SHARE_MODE` и
     поведенчески — включён ли Multi‑App Camera (пробный второй open).
   - Иначе → пометить как «нативный мультидоступ недоступен».

2. **Win11 24H2+: нативный путь, без виртуалки.** Если ОС поддерживает Multi‑App
   Camera — наша задача свести к нулю собственный код раздачи: открывать камеру в
   shared‑режиме (`MF_DEVSOURCE_ATTRIBUTE_FRAMESERVER_SHARE_MODE=1` для Win32 MF, или
   `SharedReadOnly` для WinRT) и **подсказать пользователю включить тумблер**
   (deep‑link в Settings: `ms-settings:camera`), раз API включения нет. Тогда Zoom,
   Teams, OBS и SoundCore работают с настоящей камерой одновременно, **без
   виртуального устройства**.

3. **Win10 / Win11 <24H2: честная деградация.** Нативно раздать одну камеру
   произвольным приложениям нельзя. Два варианта, оба показать пользователю явно:
   - **(a) Текущая виртуальная камера** (DirectShow Source Filter + shared‑memory
     ring) — единственный «без EV‑подписи» путь, покрывает DShow‑приложения
     (Zoom, Chrome, Skype, Telegram, OBS, Edge, VLC). **Сначала исправить найденные
     баги ACL/событий/реюза секции** (см. отчёт по багам — без этого consumer не
     подключится в обычном непривилегированном сценарии). Не покрывает чистые
     MF/UWP‑приложения.
   - **(b) DMFT‑путь** — если в перспективе нужен мультидоступ/обработка для **всех**
     приложений на Win10 без виртуального устройства: реализуемо как Device MFT, но
     требует EV‑сертификата и Partner Center (attestation). Это компромисс с
     заявленным «без возни с подписью», но **без kernel‑драйвера и без WHQL**.

4. **Маркетинговая честность.** Не обещать «без виртуальной камеры на любой Windows».
   Корректная формулировка: «Нативный одновременный доступ на Windows 11 (24H2+);
   на более старых системах — совместимый режим через устройство SoundCore Camera».

**Что делать в первую очередь:** (1) починить камерные баги shared‑memory (ACL,
manual‑reset event, реюз секции) — без них даже текущая виртуалка не подключается;
(2) добавить в core‑service определение Multi‑App/shared‑режима и Win32‑shared‑open
для Win11 24H2+; (3) в UI Camera показывать режим и deep‑link на системную настройку.
