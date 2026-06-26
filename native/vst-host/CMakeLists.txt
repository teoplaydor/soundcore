# soundcore_vst_host: JUCE-based VST3 host built as a STATIC library that
# the APO DLL links against. JUCE is fetched at configure time.

set(TARGET_NAME soundcore_vst_host)

include(FetchContent)

# JUCE 8.x — pinned for reproducibility. Switch the GIT_TAG to a newer
# release when you bump JUCE.
set(JUCE_VERSION 8.0.4 CACHE STRING "JUCE release tag")
FetchContent_Declare(JUCE
    GIT_REPOSITORY https://github.com/juce-framework/JUCE.git
    GIT_TAG        ${JUCE_VERSION}
    GIT_SHALLOW    TRUE)

# We only need a subset of JUCE — disable the GUI bits we don't want to
# pull in here. The APO is a non-GUI DLL; plugin editor windows are
# rendered out-of-process by the UI app.
set(JUCE_BUILD_EXAMPLES   OFF CACHE BOOL "" FORCE)
set(JUCE_BUILD_EXTRAS     OFF CACHE BOOL "" FORCE)

FetchContent_MakeAvailable(JUCE)

add_library(${TARGET_NAME} STATIC
    src/Host.cpp
    src/Index.cpp
    src/Chain.cpp
    include/soundcore_vst_host.h
    src/HostInternals.h
)

target_include_directories(${TARGET_NAME}
    PUBLIC
        include
    PRIVATE
        src
)

target_compile_definitions(${TARGET_NAME}
    PRIVATE
        JUCE_USE_CURL=0
        JUCE_WEB_BROWSER=0
        JUCE_PLUGINHOST_VST3=1
        JUCE_PLUGINHOST_VST=0
        JUCE_PLUGINHOST_AU=0
        JUCE_PLUGINHOST_LADSPA=0
        JUCE_MODAL_LOOPS_PERMITTED=0
        JUCE_DISABLE_NATIVE_FILECHOOSERS=1
        # We don't want to ship a JUCE splash; we use a commercial licence.
        JUCE_DISPLAY_SPLASH_SCREEN=0
        JUCE_USE_DARK_SPLASH_SCREEN=0
        JUCE_REPORT_APP_USAGE=0
        # Force release-CRT mode in JUCE regardless of the CMake config —
        # Rust on MSVC links to /MD (release CRT) even in `cargo build`
        # debug profile, so JUCE's debug-CRT calls (`_CrtDbgReport`,
        # `_CrtDumpMemoryLeaks`) would be unresolved at link time.
        JUCE_DEBUG=0
        NDEBUG=1
)

target_link_libraries(${TARGET_NAME}
    PRIVATE
        juce::juce_core
        juce::juce_events
        juce::juce_data_structures
        juce::juce_audio_basics
        juce::juce_audio_formats
        juce::juce_audio_processors
        juce::juce_audio_devices
        juce::juce_audio_utils
        juce::juce_dsp
)

# Recommended JUCE compile flags helper. Skip LTO/LTCG — the LTCG
# anonymous .obj format is incompatible with Rust's MSVC linker
# integration (anonymous-object members get dropped at link time).
target_link_libraries(${TARGET_NAME} PRIVATE
    juce::juce_recommended_warning_flags
    juce::juce_recommended_config_flags)

set_target_properties(${TARGET_NAME} PROPERTIES
    POSITION_INDEPENDENT_CODE ON
)
